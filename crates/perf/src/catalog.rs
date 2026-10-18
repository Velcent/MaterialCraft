//! [`Catalog`]: the built-in scenarios plus the user's scenario files. A file holds a JSON list of scenarios (or
//! `{"scenarios": [..]}`); a scenario with a built-in's id replaces it. Files are read from `--scenarios <file>`,
//! `$MC_PERF_SCENARIOS` (paths separated by `:`) and, when it exists, the docs' `perf/scenarios.json`.

use crate::builtin;
use crate::scenario::Scenario;
use materialcraft::json::{self, Value};
use std::path::{Path, PathBuf};

pub const SCENARIOS_ENV: &str = "MC_PERF_SCENARIOS";

/// Every known scenario, in table order (the user's new ones last).
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    pub scenarios: Vec<Scenario>,
    /// Where each scenario came from (`built-in` or a file), by index.
    pub origins: Vec<String>,
}

impl Catalog {
    pub fn builtin() -> Catalog {
        let scenarios = builtin::scenarios();
        let origins = vec!["built-in".to_string(); scenarios.len()];
        Catalog { scenarios, origins }
    }

    /// The built-ins, then `files`, then the files named by the environment and the docs' scenario file.
    pub fn load(files: &[PathBuf]) -> Result<Catalog, String> {
        let mut c = Catalog::builtin();
        for f in files.iter().cloned().chain(default_files()) {
            c.add_file(&f)?;
        }
        Ok(c)
    }

    /// Adds (or replaces, by id) the scenarios in `path`.
    pub fn add_file(&mut self, path: &Path) -> Result<(), String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        for s in parse_file(&text).map_err(|e| format!("{}: {e}", path.display()))? {
            self.add(s, path.display().to_string());
        }
        Ok(())
    }

    pub fn add(&mut self, s: Scenario, origin: String) {
        match self.scenarios.iter().position(|x| x.id == s.id) {
            Some(i) => {
                if let (Some(slot), Some(o)) = (self.scenarios.get_mut(i), self.origins.get_mut(i)) {
                    *slot = s;
                    *o = origin;
                }
            }
            None => {
                self.scenarios.push(s);
                self.origins.push(origin);
            }
        }
    }

    pub fn get(&self, id: &str) -> Option<&Scenario> {
        self.scenarios.iter().find(|s| s.id == id)
    }

    /// `all`, or the scenarios named in `ids` (comma-separated), in the order given.
    pub fn select(&self, ids: &str) -> Result<Vec<Scenario>, String> {
        if ids == "all" {
            return Ok(self.scenarios.clone());
        }
        ids.split(',').filter(|s| !s.is_empty()).map(|id| self.get(id).cloned().ok_or_else(|| format!("no scenario {id:?} (see mcperf list)"))).collect()
    }
}

/// The scenarios of one file's text.
pub fn parse_file(text: &str) -> Result<Vec<Scenario>, String> {
    let v = json::parse(text).map_err(|e| e.to_string())?;
    let list = match &v {
        Value::Array(items) => items,
        _ => v.get("scenarios").and_then(Value::as_array).ok_or("a scenario file is a list of scenarios or {\"scenarios\": [..]}")?,
    };
    list.iter().map(Scenario::from_json).collect()
}

fn default_files() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::env::var(SCENARIOS_ENV).map(|v| v.split(':').filter(|p| !p.is_empty()).map(PathBuf::from).collect()).unwrap_or_default();
    if let Some(docs) = materialcraft_parity::provenance::docs_dir() {
        let f = docs.join("perf").join("scenarios.json");
        if f.is_file() {
            files.push(f);
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_scenarios_add_and_replace() {
        let text = r#"[{"id": "stroke-2k", "source": {"mesh": "/m.obj", "resolution": 512}, "action": {"stroke": {"size": 0.2}}},
                       {"id": "mine", "source": {"document": "/d.mcpaint"}, "action": "open"}]"#;
        let mut c = Catalog::builtin();
        let n = c.scenarios.len();
        for s in parse_file(text).unwrap() {
            c.add(s, "file".into());
        }
        assert_eq!(c.scenarios.len(), n + 1);
        assert_eq!(c.get(builtin::STROKE).unwrap().source.path(), Path::new("/m.obj"));
        assert_eq!(c.select("mine,stroke-2k").unwrap().len(), 2);
        assert!(c.select("nope").is_err());
        assert_eq!(parse_file(&format!("{{\"scenarios\": {text}}}")).unwrap().len(), 2);
    }
}

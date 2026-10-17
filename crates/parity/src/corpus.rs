//! [`ProjectList`]: which `.spp` files a run uses: `--projects` (a `.spp`, a glob on file names like
//! `dir/*.spp`, or a text file with one path per line) and `--corpus` folders (every `.spp` below them).
//! Without either, the folder in `MC_SUBSTANCE_CORPUS`. Only `.spp` names are ever collected.

use std::path::{Path, PathBuf};

pub const CORPUS_ENV: &str = "MC_SUBSTANCE_CORPUS";
const MAX_DEPTH: usize = 12;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct ProjectList {
    pub projects: Vec<String>,
    pub corpora: Vec<PathBuf>,
}

impl ProjectList {
    /// The `.spp` files, sorted, without duplicates.
    pub fn resolve(&self) -> Result<Vec<PathBuf>, String> {
        let mut corpora = self.corpora.clone();
        if self.projects.is_empty() && corpora.is_empty() {
            let env = std::env::var_os(CORPUS_ENV)
                .ok_or_else(|| format!("no projects: pass --projects or --corpus, or set {CORPUS_ENV}"))?;
            corpora.push(PathBuf::from(env));
        }
        let mut out = Vec::new();
        for p in &self.projects {
            out.extend(expand(p)?);
        }
        for dir in &corpora {
            if !dir.is_dir() {
                return Err(format!("{} is not a folder", dir.display()));
            }
            collect(dir, MAX_DEPTH, &mut out);
        }
        out.sort();
        out.dedup();
        Ok(out)
    }
}

fn is_spp(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("spp"))
}

/// One `--projects` value.
fn expand(arg: &str) -> Result<Vec<PathBuf>, String> {
    let path = Path::new(arg);
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if name.contains(['*', '?']) {
        let dir = path
            .parent()
            .filter(|d| !d.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        return Ok(entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| is_spp(p) && p.file_name().is_some_and(|n| glob_match(&name, &n.to_string_lossy())))
            .collect());
    }
    if is_spp(path) {
        return Ok(vec![path.to_path_buf()]);
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("{arg}: {e}"))?;
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(PathBuf::from)
        .filter(|p| is_spp(p))
        .collect())
}

fn collect(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if kind.is_dir() && depth > 0 {
            collect(&path, depth - 1, out);
        } else if kind.is_file() && is_spp(&path) {
            out.push(path);
        }
    }
}

/// `*` (any run) and `?` (one character) on a file name.
pub fn glob_match(pattern: &str, name: &str) -> bool {
    let (p, n): (Vec<char>, Vec<char>) = (pattern.chars().collect(), name.chars().collect());
    let (mut pi, mut ni, mut star, mut mark) = (0, 0, None, 0);
    while ni < n.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == n[ni]) {
            pi += 1;
            ni += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ni;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ni = mark;
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|c| *c == '*')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn globs() {
        assert!(glob_match("*.spp", "VASE.spp"));
        assert!(glob_match("A?ULET*.spp", "AMULET 3.spp"));
        assert!(!glob_match("*.spp", "VASE.sbsar"));
        assert!(glob_match("*", ""));
    }

    #[test]
    fn resolves_files_globs_lists_and_folders() {
        let root = std::env::temp_dir().join(format!("mcparity-corpus-{}", std::process::id()));
        std::fs::create_dir_all(root.join("a/b")).unwrap();
        for f in ["a/one.spp", "a/b/two.SPP", "a/skip.txt"] {
            std::fs::write(root.join(f), "").unwrap();
        }
        let list_file = root.join("list.txt");
        std::fs::write(&list_file, format!("# mine\n{}\n", root.join("a/one.spp").display())).unwrap();
        let glob = root.join("a/*.spp").display().to_string();
        let list = ProjectList {
            projects: vec![glob, list_file.display().to_string()],
            corpora: vec![root.join("a")],
        };
        let found = list.resolve().unwrap();
        assert_eq!(found, [root.join("a/b/two.SPP"), root.join("a/one.spp")]);
        std::fs::remove_dir_all(&root).unwrap();
    }
}

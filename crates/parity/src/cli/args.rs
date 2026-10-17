//! [`Args`]: the parsed command line. Unknown flags are errors, so typos do not silently change a run.

use crate::corpus::ProjectList;
use crate::run::CacheCases;
use std::path::PathBuf;

/// Compared size when `--max-size` is not given.
pub const DEFAULT_MAX_SIZE: usize = 1024;

#[derive(Debug, Clone, PartialEq)]
pub struct Args {
    pub command: String,
    /// Positional words after the command.
    pub words: Vec<String>,
    pub projects: ProjectList,
    pub limit: Option<usize>,
    pub max_size: usize,
    /// Projects measured at the same time (`--jobs`).
    pub jobs: usize,
    pub out: Option<PathBuf>,
    pub history: Option<PathBuf>,
    pub no_history: bool,
    pub open: bool,
    /// `--cache-cases final|isolated|both`.
    pub cache_cases: CacheCases,
}

impl Args {
    pub fn parse(raw: Vec<String>) -> Result<Args, String> {
        let mut it = raw.into_iter();
        let command = it.next().ok_or_else(|| super::USAGE.to_string())?;
        let mut args = Args {
            command,
            words: Vec::new(),
            projects: ProjectList::default(),
            limit: None,
            max_size: DEFAULT_MAX_SIZE,
            jobs: materialcraft_substance::tools::parallel::CorpusPool::default_jobs(),
            out: None,
            history: None,
            no_history: false,
            open: false,
            cache_cases: CacheCases::default(),
        };
        while let Some(a) = it.next() {
            let mut value = || it.next().ok_or_else(|| format!("{a} needs a value"));
            match a.as_str() {
                "--projects" => args.projects.projects.push(value()?),
                "--corpus" => args.projects.corpora.push(PathBuf::from(value()?)),
                "--limit" => args.limit = Some(number(&a, &value()?)?),
                "--max-size" => args.max_size = number(&a, &value()?)?.clamp(16, 8192),
                "--jobs" => args.jobs = number(&a, &value()?)?.clamp(1, 64),
                "--out" => args.out = Some(PathBuf::from(value()?)),
                "--history" => args.history = Some(PathBuf::from(value()?)),
                "--no-history" => args.no_history = true,
                "--open" => args.open = true,
                "--cache-cases" => {
                    let v = value()?;
                    args.cache_cases = CacheCases::from_id(&v)
                        .ok_or(format!("--cache-cases: '{v}' is not final, isolated or both"))?;
                }
                f if f.starts_with("--") => return Err(format!("unknown option {f}")),
                _ => args.words.push(a),
            }
        }
        Ok(args)
    }
}

fn number(flag: &str, v: &str) -> Result<usize, String> {
    v.parse().map_err(|_| format!("{flag}: '{v}' is not a number"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<Args, String> {
        Args::parse(s.split_whitespace().map(String::from).collect())
    }

    #[test]
    fn parses_run_options() {
        let a = parse("run baked.ao --corpus /c --projects x.spp --limit 3 --max-size 99999 --jobs 6 --open").unwrap();
        assert_eq!(
            (a.command.as_str(), a.words.as_slice()),
            ("run", ["baked.ao".to_string()].as_slice())
        );
        assert_eq!(a.projects.corpora, [PathBuf::from("/c")]);
        assert_eq!(a.projects.projects, ["x.spp"]);
        assert_eq!((a.limit, a.max_size, a.jobs, a.open), (Some(3), 8192, 6, true));
        assert!(parse("run all --bogus").is_err());
        assert!(parse("run all --limit").is_err());
        assert!(parse("run all --limit x").is_err());
        assert!(parse("").is_err());
        assert_eq!(a.cache_cases, CacheCases::Both);
        assert_eq!(
            parse("run all --cache-cases isolated").unwrap().cache_cases,
            CacheCases::Isolated
        );
        assert!(parse("run all --cache-cases some").is_err());
    }
}

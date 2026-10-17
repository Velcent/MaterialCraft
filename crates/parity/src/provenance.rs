//! Where a run's code came from (git commit) and where the internal docs live, asked from `git` in the
//! current folder. Both are optional: without git the run still works.

use std::path::PathBuf;
use std::process::Command;

/// Name of the internal docs folder, a sibling of the main checkout.
const DOCS_DIR: &str = "MaterialCraft-docs";

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// `abc1234`, with `+dirty` when tracked files have changes; `unknown` outside a checkout.
pub fn commit() -> String {
    let Some(hash) = git(&["rev-parse", "--short", "HEAD"]) else {
        return "unknown".into();
    };
    match git(&["status", "--porcelain", "--untracked-files=no"]) {
        Some(s) if !s.is_empty() => format!("{hash}+dirty"),
        _ => hash,
    }
}

/// The docs folder next to the main checkout (also from a linked worktree), if it exists.
pub fn docs_dir() -> Option<PathBuf> {
    let common = PathBuf::from(git(&["rev-parse", "--path-format=absolute", "--git-common-dir"])?);
    let docs = common.parent()?.parent()?.join(DOCS_DIR);
    docs.is_dir().then_some(docs)
}

//! `mcstarter`: builds MaterialCraft's starter asset library into a folder.
//!
//! ```text
//! mcstarter <folder> [--size <texture pixels>] [--jobs <threads>]
//! ```
//!
//! The folder must not exist, be empty, or hold an earlier starter library (which is replaced). The same build
//! writes the same bytes.

use materialcraft_starter::{LIBRARY_VERSION, Options, build};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "usage: mcstarter <folder> [--size <texture pixels>] [--jobs <threads>]";

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("mcstarter: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    let mut options = Options::default();
    let mut dir: Option<PathBuf> = None;
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        let mut number = |what: &str| -> Result<usize, String> {
            it.next().and_then(|v| v.parse().ok()).filter(|n| *n > 0).ok_or_else(|| format!("{what} needs a positive whole number\n{USAGE}"))
        };
        match a.as_str() {
            "--size" => options.texture_size = number("--size")?,
            "--jobs" => options.jobs = number("--jobs")?,
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            _ if a.starts_with('-') => return Err(format!("unknown option {a}\n{USAGE}")),
            _ if dir.is_none() => dir = Some(PathBuf::from(a)),
            _ => return Err(format!("one folder only\n{USAGE}")),
        }
    }
    let dir = dir.ok_or(USAGE)?;
    // The CPU path is the reference: the same bytes on every machine, with or without a GPU.
    materialcraft::gpu::set_enabled(false);
    let built = build(&dir, &options).map_err(|e| e.to_string())?;
    println!("starter library {LIBRARY_VERSION}: {} assets in {}", built.assets.len(), built.dir.display());
    Ok(())
}

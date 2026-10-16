//! `mcsub`: inspect Substance Painter projects.
//!
//!     mcsub inspect <file.spp> [--verify]   one project, all sections
//!     mcsub tree <file.spp>                 every group and dataset in the file
//!     mcsub corpus <folder> [--verify]      one table row per .spp below the folder

use materialcraft_substance::container::hdf5::EntryKind;
use materialcraft_substance::inspect::{Corpus, CorpusTable, InspectOptions, ProjectReport, TextReport};
use materialcraft_substance::project::SppProject;
use std::path::PathBuf;
use std::process::ExitCode;

enum Command {
    Inspect(PathBuf, InspectOptions),
    Tree(PathBuf),
    Corpus(PathBuf, InspectOptions),
}

impl Command {
    fn parse(args: &[String]) -> Result<Command, String> {
        let options = InspectOptions {
            verify_pixels: args.iter().any(|a| a == "--verify"),
        };
        let positional: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
        match positional.as_slice() {
            [cmd, path] if cmd.as_str() == "inspect" => Ok(Command::Inspect(PathBuf::from(path), options)),
            [cmd, path] if cmd.as_str() == "tree" => Ok(Command::Tree(PathBuf::from(path))),
            [cmd, path] if cmd.as_str() == "corpus" => Ok(Command::Corpus(PathBuf::from(path), options)),
            _ => Err(
                "usage: mcsub inspect <file.spp> [--verify] | mcsub tree <file.spp> | mcsub corpus <folder> [--verify]"
                    .into(),
            ),
        }
    }

    fn run(self) -> Result<(), String> {
        match self {
            Command::Inspect(path, options) => {
                let report = ProjectReport::build(&path, options).map_err(|e| e.to_string())?;
                print!("{}", TextReport(&report));
            }
            Command::Tree(path) => {
                let project = SppProject::open(&path).map_err(|e| e.to_string())?;
                for e in project.layout().entries() {
                    match e.kind {
                        EntryKind::Group => println!("{}/", e.path),
                        EntryKind::Dataset { len } => println!("{:<90} {len:>12}", e.path),
                    }
                }
            }
            Command::Corpus(root, options) => {
                let corpus = Corpus::discover(&root);
                let rows = corpus.run(options, |i, n, p| eprintln!("[{}/{n}] {}", i + 1, p.display()));
                print!(
                    "{}",
                    CorpusTable {
                        rows: &rows,
                        root: &root
                    }
                );
            }
        }
        Ok(())
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match Command::parse(&args).and_then(Command::run) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

//! `mcsub`: inspect and import Substance Painter projects.
//!
//!     mcsub inspect <file.spp> [--verify]          one project, all sections
//!     mcsub tree <file.spp>                        every group and dataset in the file
//!     mcsub doc <file.spp> [dataset] [--full]      document.bin (or another object file) as indented text
//!     mcsub model <file.spp>                       Painter's layer stacks as a tree, with the model report
//!     mcsub model --all <folder>                   build the model of every .spp below the folder
//!     mcsub corpus <folder> [--verify]             one table row per .spp below the folder
//!     mcsub convert <in.spp> <out.mcpaint>         import and save as .mcpaint
//!     mcsub convert --all <folder> <out dir> [--discard] [--jobs N]   convert every .spp below the folder
//!     mcsub report <in>                            import and print the import report
//!     mcsub maps <in> <out dir>                    final maps per texture set as PNGs
//!     mcsub compare <in> <export dir> [--diff <dir>] [--height-normal <factor>]
//!                                                  compare with Painter's exported textures
//!     mcsub verify-layers <file.spp> [--fit]       imported layer stacks against Painter's texture cache
//!     mcsub verify-layers --all <folder> [--fit] [--jobs N]   the same for every .spp below the folder
//!
//! The `--all` commands work on several projects at once: `--jobs N` at most (default a quarter of the cores),
//! and only as many as fit in 60 % of the free memory (a project needs about twice its file size). Their
//! output is the same as one project after another.
//! `<in>` is a `.spp` (imported on the fly) or an already converted `.mcpaint`. `--height-normal` multiplies
//! the height → normal scale (default 1 = Painter's; 0 leaves height out of the normal map).
//! `convert`, `maps` and `compare` print progress on stderr as `progress <0..1> <message>`.

use materialcraft::paint::{Document, format};
use materialcraft_substance::container::hdf5::EntryKind;
use materialcraft_substance::inspect::document_dump::LIST_PREVIEW;
use materialcraft_substance::inspect::{Corpus, CorpusTable, DocumentDump, InspectOptions, ProjectReport, TextReport};
use materialcraft_substance::project::{SppProject, paths};
use materialcraft_substance::tools::model_text::ModelText;
use materialcraft_substance::tools::parallel::CorpusPool;
use materialcraft_substance::tools::report_text::ReportText;
use materialcraft_substance::tools::verify_layers::{VerifyOptions, verify_path};
use materialcraft_substance::tools::{compare, convert, maps, model_corpus};
use materialcraft_substance::translate::import_spp;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "usage: mcsub inspect|tree|doc|model|corpus|convert|report|maps|compare|verify-layers … (see the source header for details)";

/// Parsed command line: positional words, `--flag`s and `--diff <dir>`.
struct Args {
    words: Vec<String>,
    flags: Vec<String>,
    diff: Option<PathBuf>,
    height_normal: Option<f32>,
    jobs: Option<usize>,
}

impl Args {
    fn parse(raw: Vec<String>) -> Args {
        let mut args = Args {
            words: Vec::new(),
            flags: Vec::new(),
            diff: None,
            height_normal: None,
            jobs: None,
        };
        let mut it = raw.into_iter();
        while let Some(a) = it.next() {
            match a.as_str() {
                "--diff" => args.diff = it.next().map(PathBuf::from),
                "--height-normal" => args.height_normal = it.next().and_then(|v| v.parse().ok()),
                "--jobs" => args.jobs = it.next().and_then(|v| v.parse().ok()),
                f if f.starts_with("--") => args.flags.push(a),
                _ => args.words.push(a),
            }
        }
        args
    }

    /// The pool for the `--all` commands.
    fn pool(&self) -> CorpusPool {
        CorpusPool::new(self.jobs.unwrap_or_else(CorpusPool::default_jobs).clamp(1, 64))
    }

    fn has(&self, flag: &str) -> bool {
        self.flags.iter().any(|f| f == flag)
    }
}

fn progress_line(fraction: f32, message: &str) {
    eprintln!("progress {fraction:.3} {message}");
}

fn is_mcpaint(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("mcpaint"))
}

/// Loads a `.mcpaint`; imports anything else as a `.spp`.
fn open_document(path: &Path) -> Result<Document, String> {
    if is_mcpaint(path) {
        format::load_path(path).map_err(|e| e.to_string())
    } else {
        import_spp(path, &mut progress_line).map_err(|e| e.to_string())
    }
}

fn run(args: Args) -> Result<(), String> {
    let options = InspectOptions {
        verify_pixels: args.has("--verify"),
    };
    let words: Vec<&str> = args.words.iter().map(String::as_str).collect();
    let err = |e: &dyn std::fmt::Display| e.to_string();
    match words.as_slice() {
        ["inspect", path] => {
            let report = ProjectReport::build(Path::new(path), options).map_err(|e| err(&e))?;
            print!("{}", TextReport(&report));
        }
        ["tree", path] => {
            let project = SppProject::open(Path::new(path)).map_err(|e| err(&e))?;
            for e in project.layout().entries() {
                match e.kind {
                    EntryKind::Group => println!("{}/", e.path),
                    EntryKind::Dataset { len } => println!("{:<90} {len:>12}", e.path),
                }
            }
        }
        ["doc", path, rest @ ..] if rest.len() <= 1 => {
            let project = SppProject::open(Path::new(path)).map_err(|e| err(&e))?;
            let data = project
                .read(rest.first().copied().unwrap_or(paths::DOCUMENT))
                .map_err(|e| err(&e))?;
            let preview = if args.has("--full") { usize::MAX } else { LIST_PREVIEW };
            print!("{}", DocumentDump::render_with(&data, preview).map_err(|e| err(&e))?);
        }
        ["model", root] if args.has("--all") => {
            print!(
                "{}",
                model_corpus::model_all(Path::new(root), &mut |p| eprintln!("{}", p.display()))
            );
        }
        ["model", path] => {
            let project = SppProject::open(Path::new(path)).map_err(|e| err(&e))?;
            let (document, report) = project.model().map_err(|e| err(&e))?;
            print!(
                "{}",
                ModelText {
                    document: &document,
                    report: &report
                }
            );
        }
        ["corpus", root] => {
            let corpus = Corpus::discover(Path::new(root));
            let rows = corpus.run(options, |i, n, p| eprintln!("[{}/{n}] {}", i + 1, p.display()));
            let root = Path::new(root);
            print!("{}", CorpusTable { rows: &rows, root });
        }
        ["convert", root, out] if args.has("--all") => {
            let table = convert::convert_all(
                Path::new(root),
                Path::new(out),
                args.has("--discard"),
                args.pool(),
                &|line| eprintln!("{line}"),
            );
            print!("{table}");
        }
        ["convert", input, out] => {
            let summary =
                convert::convert_one(Path::new(input), Path::new(out), &mut progress_line).map_err(|e| err(&e))?;
            println!("{summary}");
        }
        ["report", input] => {
            let doc = open_document(Path::new(input))?;
            if let Some(report) = &doc.import_report {
                print!("{}", ReportText(report));
            }
        }
        ["maps", input, dir] => {
            let doc = open_document(Path::new(input))?;
            let written = maps::write_maps(&doc, Path::new(dir), &mut |set| {
                progress_line(1.0, &format!("maps of {set}"))
            })
            .map_err(|e| err(&e))?;
            println!("{} maps written to {dir}", written.len());
        }
        ["compare", input, exports] => {
            let input = Path::new(input);
            let doc = open_document(input)?;
            let mut options = compare::CompareOptions::for_project(input, args.diff.clone());
            if is_mcpaint(input) {
                options.project_time = None;
            }
            if let Some(factor) = args.height_normal {
                options.height_normal_strength = factor;
            }
            let table = compare::compare_document(&doc, Path::new(exports), &options, &mut |m| progress_line(1.0, m))
                .map_err(|e| err(&e))?;
            print!("{table}");
        }
        ["verify-layers", path] => {
            let options = VerifyOptions { fit: args.has("--fit") };
            let result = verify_path(Path::new(path), options, args.pool(), &|p| eprintln!("{}", p.display()));
            print!("{result}");
        }
        _ => return Err(USAGE.into()),
    }
    Ok(())
}

fn main() -> ExitCode {
    match run(Args::parse(std::env::args().skip(1).collect())) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

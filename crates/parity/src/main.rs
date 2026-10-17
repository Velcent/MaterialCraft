//! `mcparity`: our implementations against Painter's results on your own projects. See [`materialcraft_parity::cli`].

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match materialcraft_parity::cli::run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("mcparity: {e}");
            ExitCode::FAILURE
        }
    }
}

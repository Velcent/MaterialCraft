//! Runs the `mcsub convert` executable that ships next to the editor (see ADR 0005): `.spp` in,
//! `.mcpaint` out. Progress comes back as `progress <0..1> <message>` lines on stderr, a one-line
//! summary on stdout and exit code 0 on success.

use super::job::CancelToken;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::time::Duration;

/// Stderr lines kept for the error message when the conversion fails.
const ERROR_LINES: usize = 6;

/// The importer executable.
pub struct Mcsub {
    exe: PathBuf,
}

impl Mcsub {
    /// Finds `mcsub` next to the running editor, with a clear message if it is missing.
    pub fn locate() -> Result<Mcsub, String> {
        let me = std::env::current_exe().map_err(|e| format!("Cannot locate the editor executable: {e}"))?;
        let exe = me.with_file_name(format!("mcsub{}", std::env::consts::EXE_SUFFIX));
        if exe.is_file() {
            Ok(Mcsub { exe })
        } else {
            Err(format!(
                "The Substance Painter importer 'mcsub' was not found next to the editor (expected {}). \
                 Build it with: cargo build --release -p materialcraft-substance",
                exe.display()
            ))
        }
    }

    pub fn path(&self) -> &Path {
        &self.exe
    }

    /// Converts `input` to `output`, reporting progress. Returns mcsub's summary line.
    pub fn convert(&self, input: &Path, output: &Path, cancel: &CancelToken, mut progress: impl FnMut(f32, &str)) -> Result<String, String> {
        let mut child = Command::new(&self.exe)
            .arg("convert")
            .arg(input)
            .arg(output)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Cannot run {}: {e}", self.exe.display()))?;
        let stderr = child.stderr.take().ok_or("mcsub has no stderr")?;
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let mut errors: Vec<String> = Vec::new();
        loop {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(line) => match parse_progress(&line) {
                    Some((f, msg)) => progress(f, msg),
                    None if !line.trim().is_empty() => {
                        errors.push(line);
                        if errors.len() > ERROR_LINES {
                            errors.remove(0);
                        }
                    }
                    None => {}
                },
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            if cancel.is_cancelled() {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Import cancelled.".into());
            }
        }
        let mut summary = String::new();
        if let Some(mut out) = child.stdout.take() {
            let _ = out.read_to_string(&mut summary);
        }
        let status = child.wait().map_err(|e| format!("mcsub did not finish: {e}"))?;
        if status.success() {
            Ok(summary.trim().to_string())
        } else if errors.is_empty() {
            Err(format!("mcsub failed ({status})."))
        } else {
            Err(format!("mcsub failed: {}", errors.join("\n")))
        }
    }
}

/// Parses `progress <fraction> <message>`.
fn parse_progress(line: &str) -> Option<(f32, &str)> {
    let rest = line.strip_prefix("progress ")?;
    let (num, msg) = rest.split_once(' ').unwrap_or((rest, ""));
    let f: f32 = num.parse().ok()?;
    Some((f.clamp(0.0, 1.0), msg.trim()))
}

#[cfg(test)]
mod tests {
    use super::parse_progress;

    #[test]
    fn progress_lines() {
        assert_eq!(parse_progress("progress 0.25 Reading layers"), Some((0.25, "Reading layers")));
        assert_eq!(parse_progress("progress 1"), Some((1.0, "")));
        assert_eq!(parse_progress("progress x y"), None);
        assert_eq!(parse_progress("error: bad file"), None);
    }
}

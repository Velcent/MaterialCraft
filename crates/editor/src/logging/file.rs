// Copied from PhotoCraft, apps/photocraft/src/logging.rs (`rotate`, `format_line`, `Sink` and
// their tests), revision 57437ea. Adapted: MaterialCraft's file names; `Sink` is `LogFile` here,
// because `diag::Sink` is a different thing. See ATTRIBUTION.md.
//
// Copyright (c) 2026 ArtCraft Team and the PhotoCraft contributors.
// Licensed under either of the Apache License, Version 2.0 or the MIT license, at your option.

//! The log file: rotation at start-up, line format, and a size-capped writer that keeps early
//! lines until the file exists (LG-11).

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

/// The current log file's name inside the log directory.
pub const LOG_FILE: &str = "materialcraft.log";
/// How many previous logs are kept (`materialcraft.1.log` … `materialcraft.<KEEP>.log`).
pub const KEEP: usize = 2;
/// The log file stops growing past this size (a runaway warning can't fill the disk).
pub const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
/// Lines kept in memory until the log file is attached.
pub const MAX_PENDING: usize = 512;

fn numbered(dir: &Path, n: usize) -> PathBuf {
    if n == 0 { dir.join(LOG_FILE) } else { dir.join(format!("materialcraft.{n}.log")) }
}

/// Shift the previous logs up one (`.log` → `.1.log` → … → `.<KEEP>.log`, the oldest dropped)
/// and create a fresh, empty log file in `dir` (created if missing).
pub fn rotate(dir: &Path) -> std::io::Result<(PathBuf, File)> {
    std::fs::create_dir_all(dir)?;
    for n in (1..=KEEP).rev() {
        match std::fs::rename(numbered(dir, n - 1), numbered(dir, n)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
    }
    let path = numbered(dir, 0);
    let file = File::create(&path)?;
    Ok((path, file))
}

/// One formatted log line: `<timestamp> <LEVEL> [<thread>] <target>: <message>\n`.
pub fn format_line(ts: &str, level: log::Level, thread: &str, target: &str, message: &str) -> String {
    let message = message.strip_suffix('\n').unwrap_or(message);
    format!("{ts} {level:<5} [{thread}] {target}: {message}\n")
}

/// Where formatted lines go besides standard error: kept in memory until a file is attached,
/// then appended to it until it reaches its size cap.
pub struct LogFile {
    file: Option<File>,
    pending: Vec<String>,
    dropped: usize,
    written: u64,
    max_bytes: u64,
    capped: bool,
}

impl LogFile {
    pub fn new(max_bytes: u64) -> LogFile {
        LogFile { file: None, pending: Vec::new(), dropped: 0, written: 0, max_bytes, capped: false }
    }

    /// Append `line` to the file, or keep it until [`LogFile::attach`].
    pub fn write(&mut self, line: &str) {
        if self.file.is_none() {
            if self.pending.len() < MAX_PENDING {
                self.pending.push(line.to_owned());
            } else {
                self.dropped = self.dropped.saturating_add(1);
            }
            return;
        }
        self.append(line);
    }

    /// Start writing to `file`, first the lines kept so far.
    pub fn attach(&mut self, file: File) {
        self.file = Some(file);
        for line in std::mem::take(&mut self.pending) {
            self.append(&line);
        }
        if self.dropped > 0 {
            let note = format!("{} earlier log lines were dropped before the log file was opened\n", self.dropped);
            self.dropped = 0;
            self.append(&note);
        }
    }

    fn append(&mut self, line: &str) {
        if self.capped {
            return;
        }
        let Some(file) = self.file.as_mut() else { return };
        let len = u64::try_from(line.len()).unwrap_or(u64::MAX);
        let text = if self.written.saturating_add(len) > self.max_bytes {
            self.capped = true;
            format!("log file reached {} bytes; later records go to standard error only\n", self.max_bytes)
        } else {
            self.written = self.written.saturating_add(len);
            line.to_owned()
        };
        // A full disk or a vanished file must not take the app down: drop the file sink.
        if file.write_all(text.as_bytes()).is_err() {
            self.file = None;
            self.capped = true;
        }
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    pub fn temp_dir(tag: &str) -> PathBuf {
        static N: AtomicUsize = AtomicUsize::new(0);
        let d = std::env::temp_dir().join(format!("materialcraft-logging-{tag}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    pub fn read(path: &Path) -> String {
        std::fs::read_to_string(path).expect("read log")
    }

    #[test]
    fn rotation_keeps_the_previous_logs_and_starts_an_empty_file() {
        let dir = temp_dir("rotate");
        for run in 1..=4 {
            let (path, _file) = rotate(&dir).expect("rotate");
            assert_eq!(path, dir.join(LOG_FILE));
            assert_eq!(std::fs::read_to_string(&path).expect("read"), "");
            std::fs::write(&path, format!("run {run}")).expect("write");
        }
        assert_eq!(std::fs::read_to_string(dir.join(LOG_FILE)).expect("current"), "run 4");
        assert_eq!(std::fs::read_to_string(dir.join("materialcraft.1.log")).expect("1"), "run 3");
        assert_eq!(std::fs::read_to_string(dir.join("materialcraft.2.log")).expect("2"), "run 2");
        assert!(!dir.join("materialcraft.3.log").exists(), "only {KEEP} old logs are kept");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rotation_into_an_unusable_directory_is_an_error_not_a_panic() {
        let dir = temp_dir("blocked");
        std::fs::create_dir_all(dir.parent().expect("parent")).expect("tmp");
        std::fs::write(&dir, "a file where the directory should be").expect("block");
        assert!(rotate(&dir).is_err());
        let _ = std::fs::remove_file(&dir);
    }

    #[test]
    fn a_line_carries_time_level_thread_target_and_message() {
        assert_eq!(
            format_line("2026-10-08T07:59:17.728Z", log::Level::Warn, "main", "materialcraft_editor::gui_paint", "GPU viewer unavailable"),
            "2026-10-08T07:59:17.728Z WARN  [main] materialcraft_editor::gui_paint: GPU viewer unavailable\n"
        );
        // A multi-line message (a panic with a backtrace) keeps its lines; the record still ends in one newline.
        assert!(format_line("t", log::Level::Error, "w", "x", "a\nb\n").ends_with("x: a\nb\n"));
    }

    #[test]
    fn lines_logged_before_the_file_exists_are_written_when_it_is_attached() {
        let dir = temp_dir("pending");
        let (path, file) = rotate(&dir).expect("rotate");
        let mut sink = LogFile::new(MAX_FILE_BYTES);
        sink.write("early 1\n");
        sink.write("early 2\n");
        sink.attach(file);
        sink.write("late\n");
        assert_eq!(read(&path), "early 1\nearly 2\nlate\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_pending_buffer_is_bounded_and_says_how_much_it_dropped() {
        let dir = temp_dir("bounded");
        let (path, file) = rotate(&dir).expect("rotate");
        let mut sink = LogFile::new(MAX_FILE_BYTES);
        for i in 0..MAX_PENDING + 5 {
            sink.write(&format!("line {i}\n"));
        }
        sink.attach(file);
        let text = read(&path);
        assert_eq!(text.lines().filter(|l| l.starts_with("line ")).count(), MAX_PENDING);
        assert!(text.contains("5 earlier log lines were dropped"), "{text}");
        assert!(text.contains("line 0\n") && !text.contains(&format!("line {MAX_PENDING}\n")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_file_stops_at_its_size_cap_with_one_note() {
        let dir = temp_dir("cap");
        let (path, file) = rotate(&dir).expect("rotate");
        let mut sink = LogFile::new(100);
        sink.attach(file);
        for i in 0..50 {
            sink.write(&format!("line {i:02} with some padding\n"));
        }
        let text = read(&path);
        assert!(text.lines().filter(|l| l.starts_with("line ")).count() < 5, "{text}");
        assert_eq!(text.matches("log file reached").count(), 1, "{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

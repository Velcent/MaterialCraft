//! The sink the editor's own code emits into: every entry goes to the log file and to the
//! collector, so the file holds the whole session, not only `log` records.

use std::io::Write;
use std::sync::{Arc, PoisonError};

use materialcraft::diag::{Collector, Entry, Level, Sink};

use super::logger::SharedFile;

/// A [`Sink`] that writes each entry as a line to the log file (and warnings and errors to
/// standard error) and passes it on to the collector.
pub struct TeeSink {
    file: SharedFile,
    collector: Arc<Collector>,
    stderr: bool,
}

impl TeeSink {
    pub fn new(file: SharedFile, collector: Arc<Collector>, stderr: bool) -> TeeSink {
        TeeSink { file, collector, stderr }
    }
}

impl Sink for TeeSink {
    fn emit(&self, entry: Entry) {
        let line = format!("{entry}\n");
        if self.stderr && entry.level >= Level::Warning {
            let _ = std::io::stderr().write_all(line.as_bytes());
        }
        self.file.lock().unwrap_or_else(PoisonError::into_inner).write(&line);
        self.collector.emit(entry);
    }
}

#[cfg(test)]
mod tests {
    use super::super::file::{LogFile, MAX_FILE_BYTES, rotate, tests::read, tests::temp_dir};
    use super::*;
    use materialcraft::diag::{Area, RingBuffer, SinkExt};
    use std::sync::Mutex;

    #[test]
    fn entries_reach_both_the_file_and_the_collector() {
        let dir = temp_dir("tee");
        let file = Arc::new(Mutex::new(LogFile::new(MAX_FILE_BYTES)));
        let collector = Arc::new(Collector::default());
        let tee = TeeSink::new(file.clone(), collector.clone(), false);
        tee.info(Area::Project, "Opened demo");
        let (path, f) = rotate(&dir).expect("rotate");
        file.lock().expect("lock").attach(f);
        tee.error(Area::Import, "broken file");
        let text = read(&path);
        assert!(text.contains("info    [project] Opened demo\n"), "{text}");
        assert!(text.contains("error   [import] broken file\n"), "{text}");
        let mut history = RingBuffer::default();
        assert_eq!(collector.drain_into(&mut history), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

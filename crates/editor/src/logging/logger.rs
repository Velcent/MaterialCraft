// Copied from PhotoCraft, apps/photocraft/src/logging.rs (`AppLogger`, `install` and the logger
// tests), revision 57437ea. Adapted: the log file is shared with the diagnostics tee, and every
// record is also handed to a `diag::Collector` for the log panel. See ATTRIBUTION.md.
//
// Copyright (c) 2026 ArtCraft Team and the PhotoCraft contributors.
// Licensed under either of the Apache License, Version 2.0 or the MIT license, at your option.

//! The `log` facade's backend: records from every crate (eframe, glow, winit, the file dialog
//! portal) go to standard error, the log file and the session's [`diag::Collector`].

use std::io::Write;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::SystemTime;

use materialcraft::diag::{self, Area, Collector, Entry, Level, Sink};

use super::file::{LogFile, format_line};
use super::filter::Filter;

/// The log file, shared by the logger and the diagnostics tee.
pub type SharedFile = Arc<Mutex<LogFile>>;

/// The installed logger: a [`Filter`], standard error, the log file and the diagnostics collector.
pub struct AppLogger {
    filter: Filter,
    stderr: bool,
    file: SharedFile,
    collector: Arc<Collector>,
}

impl AppLogger {
    pub fn new(filter: Filter, stderr: bool, file: SharedFile, collector: Arc<Collector>) -> AppLogger {
        AppLogger { filter, stderr, file, collector }
    }
}

impl log::Log for AppLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= self.filter.level_for(metadata.target())
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let now = SystemTime::now();
        let message = record.args().to_string();
        let thread = std::thread::current();
        let line = format_line(&diag::timestamp(now), record.level(), thread.name().unwrap_or("?"), record.target(), &message);
        if self.stderr {
            let _ = std::io::stderr().write_all(line.as_bytes());
        }
        self.file.lock().unwrap_or_else(PoisonError::into_inner).write(&line);
        self.collector.emit(entry(record.level(), record.target(), &message).at(now));
    }

    fn flush(&self) {}
}

/// The diagnostics entry for a `log` record. Messages from other crates carry their target, so
/// the log panel can tell `egui_glow` from our own lines.
pub fn entry(level: log::Level, target: &str, message: &str) -> Entry {
    let level = match level {
        log::Level::Error => Level::Error,
        log::Level::Warn => Level::Warning,
        log::Level::Info => Level::Info,
        log::Level::Debug | log::Level::Trace => Level::Debug,
    };
    let message = message.strip_suffix('\n').unwrap_or(message);
    let text = if target.starts_with("materialcraft") { message.to_owned() } else { format!("{target}: {message}") };
    Entry::new(level, area(target), text)
}

/// GPU crates report under [`Area::Gpu`]; everything else is general.
pub fn area(target: &str) -> Area {
    const GPU: [&str; 4] = ["glow", "egui_glow", "wgpu", "naga"];
    if GPU.iter().any(|p| target.starts_with(p)) { Area::Gpu } else { Area::General }
}

/// Install a logger (filter from `RUST_LOG`, else the default); `None` when another logger was
/// installed first (a second editor instance in tests).
pub fn install(file: SharedFile, collector: Arc<Collector>) -> Option<&'static AppLogger> {
    static LOGGER: OnceLock<AppLogger> = OnceLock::new();
    let filter = Filter::from_env();
    let max = filter.max();
    let logger = LOGGER.get_or_init(|| AppLogger::new(filter, true, file, collector));
    log::set_logger(logger).ok()?;
    log::set_max_level(max);
    Some(logger)
}

#[cfg(test)]
mod tests {
    use super::super::file::{LOG_FILE, MAX_FILE_BYTES, rotate, tests::read, tests::temp_dir};
    use super::super::filter::DEFAULT_FILTER;
    use super::*;
    use log::Log;
    use materialcraft::diag::RingBuffer;

    fn logger() -> (AppLogger, SharedFile, Arc<Collector>) {
        let file = Arc::new(Mutex::new(LogFile::new(MAX_FILE_BYTES)));
        let collector = Arc::new(Collector::default());
        (AppLogger::new(Filter::parse(DEFAULT_FILTER), false, file.clone(), collector.clone()), file, collector)
    }

    fn record(logger: &AppLogger, level: log::Level, target: &'static str, msg: &'static str) {
        logger.log(&log::Record::builder().level(level).target(target).args(format_args!("{msg}")).build());
    }

    #[test]
    fn the_logger_filters_records_and_writes_them_to_the_attached_file() {
        let dir = temp_dir("logger");
        let (logger, file, _) = logger();
        record(&logger, log::Level::Info, "materialcraft_editor::logging", "settings directory");
        record(&logger, log::Level::Info, "winit::platform", "too chatty");
        let (path, f) = rotate(&dir).expect("rotate");
        file.lock().expect("lock").attach(f);
        assert_eq!(path, dir.join(LOG_FILE));
        record(&logger, log::Level::Warn, "egui_glow::painter", "a real warning");
        record(&logger, log::Level::Debug, "materialcraft", "below info");
        let text = read(&path);
        assert!(text.contains(" INFO  [") && text.contains("materialcraft_editor::logging: settings directory\n"), "{text}");
        assert!(text.contains("egui_glow::painter: a real warning\n"), "{text}");
        assert!(!text.contains("too chatty") && !text.contains("below info"), "{text}");
        assert!(logger.enabled(&log::Metadata::builder().level(log::Level::Info).target("materialcraft_substance").build()));
        assert!(!logger.enabled(&log::Metadata::builder().level(log::Level::Info).target("winit").build()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn records_reach_the_collector_with_level_area_and_source() {
        let (logger, _, collector) = logger();
        record(&logger, log::Level::Warn, "egui_glow::painter", "shader warning");
        record(&logger, log::Level::Error, "materialcraft_editor", "own error");
        record(&logger, log::Level::Info, "winit", "filtered out");
        let mut history = RingBuffer::default();
        assert_eq!(collector.drain_into(&mut history), 2);
        let entries: Vec<&Entry> = history.iter().map(|(_, e)| e).collect();
        assert_eq!((entries[0].level, entries[0].area, entries[0].text.as_str()), (Level::Warning, Area::Gpu, "egui_glow::painter: shader warning"));
        assert_eq!((entries[1].level, entries[1].area, entries[1].text.as_str()), (Level::Error, Area::General, "own error"));
        assert!(entries.iter().all(|e| e.target.is_none()));
    }

    #[test]
    fn levels_and_areas_map_as_designed() {
        assert_eq!(entry(log::Level::Trace, "materialcraft", "x").level, Level::Debug);
        assert_eq!(entry(log::Level::Debug, "materialcraft", "x").level, Level::Debug);
        for gpu in ["glow", "egui_glow::painter", "wgpu_core", "naga::front"] {
            assert_eq!(area(gpu), Area::Gpu, "{gpu}");
        }
        assert_eq!(area("eframe"), Area::General);
    }
}

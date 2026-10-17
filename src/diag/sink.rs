//! Where producers send entries. Passed explicitly; there is no global logger (ADR 0006 VQ1).

use super::{Area, Entry, Level, Target};

/// Receives diagnostic entries. Implementations must be cheap and must not block for long:
/// producers call this from worker threads in the middle of their work.
pub trait Sink: Send + Sync {
    fn emit(&self, entry: Entry);
}

/// Shorthands for every [`Sink`], including `&dyn Sink`.
pub trait SinkExt: Sink {
    fn debug(&self, area: Area, text: impl Into<String>) {
        self.emit(Entry::new(Level::Debug, area, text));
    }
    fn info(&self, area: Area, text: impl Into<String>) {
        self.emit(Entry::new(Level::Info, area, text));
    }
    fn warn(&self, area: Area, text: impl Into<String>) {
        self.emit(Entry::new(Level::Warning, area, text));
    }
    fn error(&self, area: Area, text: impl Into<String>) {
        self.emit(Entry::new(Level::Error, area, text));
    }
    /// An entry about `target`.
    fn about(&self, level: Level, area: Area, target: Target, text: impl Into<String>) {
        self.emit(Entry::new(level, area, text).with_target(target));
    }
}

impl<S: Sink + ?Sized> SinkExt for S {}

/// A sink that discards everything, for callers that don't want diagnostics.
#[derive(Debug, Clone, Copy, Default)]
pub struct NullSink;

impl Sink for NullSink {
    fn emit(&self, _entry: Entry) {}
}

impl<S: Sink + ?Sized> Sink for &S {
    fn emit(&self, entry: Entry) {
        (**self).emit(entry);
    }
}

impl<S: Sink + ?Sized> Sink for std::sync::Arc<S> {
    fn emit(&self, entry: Entry) {
        (**self).emit(entry);
    }
}

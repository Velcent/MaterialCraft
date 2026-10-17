//! [`SharedSink`]: lets the workers that measure projects side by side (on the importer's `CorpusPool`)
//! report into one [`CaseSink`].

use super::case::{CaseImages, CaseResult, CaseSink};
use std::sync::{Mutex, PoisonError};

/// One sink shared by the workers; each call holds the lock for that one case or note.
pub struct SharedSink<'a> {
    inner: Mutex<&'a mut (dyn CaseSink + Send)>,
}

impl<'a> SharedSink<'a> {
    pub fn new(sink: &'a mut (dyn CaseSink + Send)) -> SharedSink<'a> {
        SharedSink {
            inner: Mutex::new(sink),
        }
    }

    /// A handle a worker passes on as its own `&mut dyn CaseSink`.
    pub fn handle(&self) -> SinkHandle<'_, 'a> {
        SinkHandle { shared: self }
    }
}

pub struct SinkHandle<'s, 'a> {
    shared: &'s SharedSink<'a>,
}

impl CaseSink for SinkHandle<'_, '_> {
    fn case(&mut self, result: &CaseResult, images: Option<&CaseImages>) {
        self.shared
            .inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .case(result, images);
    }

    fn note(&mut self, message: &str) {
        self.shared
            .inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .note(message);
    }
}

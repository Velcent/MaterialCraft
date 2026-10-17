//! [`Job`]: one background thread that streams results to the UI thread, with an explicit cancel handle.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};

/// Shared cancel flag handed to background work (no globals).
#[derive(Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// A running background thread. Dropping the job cancels it.
pub struct Job<T> {
    rx: Receiver<T>,
    cancel: CancelToken,
    finished: bool,
}

impl<T: Send + 'static> Job<T> {
    /// Runs `work` on a new thread; it sends results through the sender and should stop soon after
    /// the token is cancelled.
    pub fn spawn(work: impl FnOnce(&CancelToken, &Sender<T>) + Send + 'static) -> Job<T> {
        let (tx, rx) = channel();
        let cancel = CancelToken::default();
        let token = cancel.clone();
        std::thread::spawn(move || work(&token, &tx));
        Job { rx, cancel, finished: false }
    }

    /// Results that arrived since the last call.
    pub fn poll(&mut self) -> Vec<T> {
        let mut out = Vec::new();
        loop {
            match self.rx.try_recv() {
                Ok(v) => out.push(v),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.finished = true;
                    break;
                }
            }
        }
        out
    }

    /// Asks the thread to stop; it ends at its next check of the token.
    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    /// True once the thread has ended and every result was polled.
    pub fn is_finished(&self) -> bool {
        self.finished
    }
}

impl<T> Drop for Job<T> {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

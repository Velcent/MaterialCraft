//! Running projects side by side: [`map_in_order`] hands out items to a fixed number of worker threads and
//! returns their results in input order; [`SharedSink`] lets those workers report into one [`CaseSink`].

use super::case::{CaseImages, CaseResult, CaseSink};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};

/// Projects measured at once when `--jobs` is not given: a quarter of the cores, since each project's bake
/// already runs on several threads, and at least one.
pub fn default_jobs() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get() / 4)
        .unwrap_or(1)
        .max(1)
}

/// Runs `work` on every item with at most `jobs` threads; results come back in the order of `items`.
pub fn map_in_order<T: Sync, R: Send>(items: &[T], jobs: usize, work: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let done: Mutex<Vec<(usize, R)>> = Mutex::new(Vec::with_capacity(items.len()));
    std::thread::scope(|sc| {
        for _ in 0..jobs.clamp(1, items.len().max(1)) {
            sc.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(item) = items.get(index) else { break };
                    let result = work(item);
                    done.lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .push((index, result));
                }
            });
        }
    });
    let mut done = done.into_inner().unwrap_or_else(PoisonError::into_inner);
    done.sort_by_key(|(i, _)| *i);
    done.into_iter().map(|(_, r)| r).collect()
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn results_keep_input_order_with_many_workers() {
        let items: Vec<u64> = (0..200).collect();
        let out = map_in_order(&items, 8, |&n| {
            // uneven work so workers finish out of order
            std::thread::sleep(std::time::Duration::from_micros((200 - n) * 20));
            n * 2
        });
        assert_eq!(out, items.iter().map(|n| n * 2).collect::<Vec<_>>());
    }

    #[test]
    fn handles_empty_input_and_zero_jobs() {
        let none: Vec<u8> = Vec::new();
        assert!(map_in_order(&none, 4, |&n| n).is_empty());
        assert_eq!(map_in_order(&[1u8, 2, 3], 0, |&n| n + 1), vec![2, 3, 4]);
    }
}

//! [`CorpusPool`]: runs one job per project on a few threads at once, within a memory budget, and returns the
//! results in project order. Importing a project holds about twice its file size in memory (8.7 GB for a
//! 4.3 GB project), so a thread count alone could run the machine out of memory when several large projects
//! meet; each job reserves its estimate first and waits until it fits. A job larger than the whole budget
//! runs alone.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, PoisonError};

/// Memory one project's job is assumed to need beyond twice its file size.
const BASE_BYTES: u64 = 256 << 20;
/// Budget when the available memory cannot be read.
const FALLBACK_BUDGET: u64 = 8 << 30;

#[derive(Debug, Clone, Copy)]
pub struct CorpusPool {
    /// Projects worked on at the same time, at most.
    pub jobs: usize,
    /// Sum of the jobs' memory estimates allowed at once, in bytes.
    pub budget: u64,
}

impl CorpusPool {
    /// `jobs` threads; the budget is 60 % of the memory available right now.
    pub fn new(jobs: usize) -> CorpusPool {
        CorpusPool {
            jobs: jobs.max(1),
            budget: available_memory().map_or(FALLBACK_BUDGET, |b| b / 10 * 6),
        }
    }

    /// A quarter of the cores, at least one: each project's own work already runs on several threads.
    pub fn default_jobs() -> usize {
        std::thread::available_parallelism()
            .map(|n| n.get() / 4)
            .unwrap_or(1)
            .max(1)
    }

    /// Estimated memory for working on the project at `path`: twice its size plus a base.
    pub fn estimate(path: &Path) -> u64 {
        let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        size.saturating_mul(2).saturating_add(BASE_BYTES)
    }

    /// Runs `work` on every item; results come back in the order of `items`. `weight` is an item's memory
    /// estimate (see [`CorpusPool::estimate`]).
    pub fn map<T: Sync, R: Send>(
        &self,
        items: &[T],
        weight: impl Fn(&T) -> u64 + Sync,
        work: impl Fn(&T) -> R + Sync,
    ) -> Vec<R> {
        let next = AtomicUsize::new(0);
        let memory = Budget::new(self.budget);
        let done: Mutex<Vec<(usize, R)>> = Mutex::new(Vec::with_capacity(items.len()));
        std::thread::scope(|sc| {
            for _ in 0..self.jobs.min(items.len().max(1)) {
                sc.spawn(|| {
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(item) = items.get(index) else { break };
                        let reserved = memory.reserve(weight(item));
                        let result = work(item);
                        memory.release(reserved);
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
}

/// Bytes reserved by running jobs; `reserve` blocks until a new job fits (or nothing else runs).
struct Budget {
    limit: u64,
    used: Mutex<u64>,
    freed: Condvar,
}

impl Budget {
    fn new(limit: u64) -> Budget {
        Budget {
            limit,
            used: Mutex::new(0),
            freed: Condvar::new(),
        }
    }

    /// Waits until `bytes` fit; returns what was reserved (never more than the whole budget).
    fn reserve(&self, bytes: u64) -> u64 {
        let want = bytes.min(self.limit);
        let mut used = self.used.lock().unwrap_or_else(PoisonError::into_inner);
        while *used > 0 && used.saturating_add(want) > self.limit {
            used = self.freed.wait(used).unwrap_or_else(PoisonError::into_inner);
        }
        *used = used.saturating_add(want);
        want
    }

    fn release(&self, bytes: u64) {
        let mut used = self.used.lock().unwrap_or_else(PoisonError::into_inner);
        *used = used.saturating_sub(bytes);
        self.freed.notify_all();
    }
}

/// `MemAvailable` from `/proc/meminfo` (Linux); `None` elsewhere.
fn available_memory() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    let line = text.lines().find(|l| l.starts_with("MemAvailable:"))?;
    let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kib.saturating_mul(1024))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU64;
    use std::time::Duration;

    #[test]
    fn results_keep_input_order() {
        let pool = CorpusPool {
            jobs: 8,
            budget: u64::MAX,
        };
        let items: Vec<u64> = (0..100).collect();
        let out = pool.map(
            &items,
            |_| 1,
            |&n| {
                std::thread::sleep(Duration::from_micros((100 - n) * 30));
                n * 3
            },
        );
        assert_eq!(out, items.iter().map(|n| n * 3).collect::<Vec<_>>());
    }

    #[test]
    fn running_jobs_stay_within_the_budget() {
        let pool = CorpusPool { jobs: 6, budget: 10 };
        let (running, peak) = (AtomicU64::new(0), AtomicU64::new(0));
        let items: Vec<u64> = vec![4, 4, 4, 3, 3, 3, 2, 2, 25, 1, 1, 1];
        pool.map(
            &items,
            |&w| w,
            |&w| {
                let reserved = w.min(10);
                let now = running.fetch_add(reserved, Ordering::SeqCst) + reserved;
                peak.fetch_max(now, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(5));
                running.fetch_sub(reserved, Ordering::SeqCst);
            },
        );
        assert!(
            peak.load(Ordering::SeqCst) <= 10,
            "peak {}",
            peak.load(Ordering::SeqCst)
        );
    }

    #[test]
    fn empty_input_and_one_job() {
        let pool = CorpusPool { jobs: 1, budget: 1 };
        assert!(pool.map(&Vec::<u8>::new(), |_| 1, |&n| n).is_empty());
        assert_eq!(pool.map(&[1u8, 2], |_| 5, |&n| n), vec![1, 2]);
    }
}

//! Small data-parallel helper for the paint module, built on scoped std threads.
//!
//! Unlike `crate::tex::par_map` it never stops early for a stale editor job, so callers always get
//! complete results.

/// Calls `f(index, item)` for every item, spread over the available cores.
pub(crate) fn for_each_mut<T: Send, F: Fn(usize, &mut T) + Sync>(items: &mut [T], f: F) {
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).max(1);
    if items.len() <= 1 || threads == 1 {
        for (i, it) in items.iter_mut().enumerate() {
            f(i, it);
        }
        return;
    }
    let per = items.len().div_ceil(threads);
    let f = &f;
    std::thread::scope(|s| {
        for (ci, chunk) in items.chunks_mut(per).enumerate() {
            s.spawn(move || {
                for (i, it) in chunk.iter_mut().enumerate() {
                    f(ci * per + i, it);
                }
            });
        }
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn visits_every_item_once_with_its_index() {
        let mut v = vec![0usize; 1000];
        super::for_each_mut(&mut v, |i, x| *x += i + 1);
        assert!(v.iter().enumerate().all(|(i, &x)| x == i + 1));
    }
}

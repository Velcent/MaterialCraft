//! Parallel encoding and decoding of images, a few at a time to bound memory.

/// How many images are processed at once.
pub fn batch_size() -> usize {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).clamp(1, 8)
}

/// `f` applied to every item, one thread per item, results in order. A panicking `f` gives
/// `Err` for its item.
pub fn par_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> Result<R, String> + Sync) -> Vec<Result<R, String>> {
    if items.len() <= 1 {
        return items.iter().map(&f).collect();
    }
    let f = &f;
    std::thread::scope(|s| {
        let handles: Vec<_> = items.iter().map(|it| s.spawn(move || f(it))).collect();
        handles.into_iter().map(|h| h.join().unwrap_or_else(|_| Err("internal error while coding the image".into()))).collect()
    })
}

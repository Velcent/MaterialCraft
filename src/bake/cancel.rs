//! [`CancelToken`]: lets another thread stop a running bake.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A shared flag: clone it, hand one copy to the bake and call [`CancelToken::cancel`] on another. The baker checks
/// it between stages and every few thousand texels, then returns [`super::BakeError::Cancelled`].
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> CancelToken {
        CancelToken::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clones_share_the_flag() {
        let a = CancelToken::new();
        let b = a.clone();
        assert!(!b.is_cancelled());
        a.cancel();
        assert!(b.is_cancelled());
    }
}

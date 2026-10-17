//! [`Cancel`]: the caller's "stop now" question, asked between the stages of an import, before each texture
//! set and before each image decode. Once it answers yes the import ends with [`ImportError::Cancelled`].

use super::error::ImportError;

/// A borrowed cancel hook. `Cancel::NEVER` never stops.
#[derive(Clone, Copy)]
pub struct Cancel<'a> {
    asked: Option<&'a dyn Fn() -> bool>,
}

impl<'a> Cancel<'a> {
    /// An import that runs to the end.
    pub const NEVER: Cancel<'static> = Cancel { asked: None };

    /// Stops the import once `asked` returns true.
    pub fn new(asked: &'a dyn Fn() -> bool) -> Self {
        Cancel { asked: Some(asked) }
    }

    /// True once the caller wants the import stopped.
    pub fn requested(&self) -> bool {
        self.asked.is_some_and(|f| f())
    }

    /// `Err(Cancelled)` once the caller wants the import stopped.
    pub fn check(&self) -> Result<(), ImportError> {
        if self.requested() {
            Err(ImportError::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn checks_ask_the_hook_each_time() {
        assert!(Cancel::NEVER.check().is_ok());
        let stop = Cell::new(false);
        let asked = || stop.get();
        let cancel = Cancel::new(&asked);
        assert!(cancel.check().is_ok());
        stop.set(true);
        assert!(matches!(cancel.check(), Err(ImportError::Cancelled)));
    }
}

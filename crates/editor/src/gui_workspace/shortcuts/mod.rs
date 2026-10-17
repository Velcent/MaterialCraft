//! The rebindable shortcut table and its page (WS-12, PT-59).

mod capture;
pub mod chord;
pub mod page;
mod table;

pub use page::{PageEdit, ShortcutsPage};
pub use table::{ActionSpec, Kind, ShortcutTable};

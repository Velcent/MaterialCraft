//! The generic object tree the model is built from: [`RawTree`] (the walk, in memory) of [`RawObject`]s.

mod builder;
mod tree;

pub use builder::RawTree;
pub use tree::{RawObject, RawValue};

//! Session diagnostics: what happened during this run, for the log panel and bug reports.
//!
//! Producers (importer, baker, renderer, asset loading, document checks) get a [`Sink`] passed in
//! explicitly and emit [`Entry`]s into it; there is no global logger, so the core stays std-only
//! and tests can hand a producer its own sink. The editor owns a [`Collector`] (the sink every
//! thread writes to) and once per frame moves what arrived into a [`RingBuffer`], the bounded
//! history the log panel shows and queries with a [`Filter`].
//!
//! ```text
//! producer threads --emit--> Collector (bounded inbox) --drain_into, once a frame--> RingBuffer --Filter--> panel
//! ```
//!
//! Both buffers are bounded and drop their *oldest* entries when full, so a burst never grows
//! memory and the newest messages always survive; drops are counted and reported as an entry.
//! Design: ADR 0006 VQ1 and VQ14 in MaterialCraft-docs, requirements LG-01 to LG-12.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod area;
mod buffer;
mod collect;
mod entry;
mod filter;
mod level;
mod sink;
mod target;
mod time;

pub use area::{Area, Tab};
pub use buffer::{Counts, EntryId, RingBuffer};
pub use collect::Collector;
pub use entry::Entry;
pub use filter::{Filter, LevelMask};
pub use level::Level;
pub use sink::{NullSink, Sink, SinkExt};
pub use target::Target;
pub use time::timestamp;

//! [`BakerEvent`]: what the bakers panel asks for.

use materialcraft::bake::{MapKind, MapSelection};
use materialcraft::history::CoalesceKey;
use materialcraft::paint::bake_config::{BakePart, CommonBake, MapFlags, MapSettings};

#[derive(Clone, PartialEq, Debug)]
pub enum BakerEvent {
    /// New common settings of `set` (every set while linked); `key` joins a slider drag into one step.
    Common {
        set: String,
        common: Box<CommonBake>,
        key: Option<CoalesceKey>,
    },
    /// New settings of one map of `set` (every set while that map is linked).
    Settings {
        set: String,
        settings: MapSettings,
        key: Option<CoalesceKey>,
    },
    Flags {
        set: String,
        kind: MapKind,
        flags: MapFlags,
    },
    /// Show `kind` of `set` in the viewport, or nothing.
    Show {
        set: String,
        kind: Option<MapKind>,
    },
    /// Link or unlink a part; linking gives every set `set`'s values.
    Link {
        set: String,
        part: BakePart,
        on: bool,
    },
    CopyPart {
        from: String,
        part: BakePart,
        to: Vec<String>,
    },
    CopyAll {
        from: String,
        to: Vec<String>,
    },
    CheckAll {
        set: String,
        on: bool,
    },
    InvertChecks {
        set: String,
    },
    CopyChecks {
        from: String,
        to: Vec<String>,
    },
    /// A slider drag or typed edit ended.
    EndGesture,
    /// Bake `sets` (ids) in order: their included maps, or (`maps`) only these.
    Bake {
        sets: Vec<String>,
        maps: Option<MapSelection>,
    },
    /// Stop the running bake.
    Cancel,
}

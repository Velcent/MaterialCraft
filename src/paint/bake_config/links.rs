//! [`BakeLinks`]: which parts of the bake configuration are linked across texture sets. While a part is linked, a
//! change of it in one set is made in every set ([`super::ops`]).

use crate::bake::{MapKind, MapSelection};

/// One linkable part of the configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BakePart {
    /// The common settings (size, rays, dilation, high-poly source).
    Common,
    /// One map's baker settings.
    Map(MapKind),
}

/// The document's link flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BakeLinks {
    pub common: bool,
    pub maps: MapSelection,
}

impl Default for BakeLinks {
    /// Nothing linked.
    fn default() -> BakeLinks {
        BakeLinks { common: false, maps: MapSelection::none() }
    }
}

impl BakeLinks {
    pub fn is_linked(&self, part: BakePart) -> bool {
        match part {
            BakePart::Common => self.common,
            BakePart::Map(kind) => self.maps.contains(kind),
        }
    }

    /// The same links with `part` on or off.
    pub fn with(self, part: BakePart, on: bool) -> BakeLinks {
        match (part, on) {
            (BakePart::Common, on) => BakeLinks { common: on, ..self },
            (BakePart::Map(kind), true) => BakeLinks { maps: self.maps.with(kind), ..self },
            (BakePart::Map(kind), false) => BakeLinks { maps: self.maps.without(kind), ..self },
        }
    }
}

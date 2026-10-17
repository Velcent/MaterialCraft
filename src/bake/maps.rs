//! The kinds of map the baker makes, a selection of them, and the result of one bake.

use crate::tex::Tex;
use std::collections::BTreeMap;
use std::fmt;

/// One bakeable mesh map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MapKind {
    /// Tangent-space normal map (OpenGL: green points up in the texture).
    Normal,
    /// World-space normal of the high mesh, `n * 0.5 + 0.5`.
    WorldNormal,
    /// Position of the high surface, normalised into 0..1 (see [`super::PositionSettings`]).
    Position,
    /// Ambient occlusion: 1 open, 0 occluded.
    AmbientOcclusion,
    /// Mesh curvature: 0.5 flat, brighter convex, darker concave.
    Curvature,
    /// Thickness: how far rays travel inside the mesh, 1 thick, 0 thin.
    Thickness,
    /// A flat colour per high-mesh part.
    Id,
}

impl MapKind {
    /// Every kind, in baking order.
    pub const ALL: [MapKind; 7] =
        [MapKind::Normal, MapKind::WorldNormal, MapKind::Position, MapKind::AmbientOcclusion, MapKind::Curvature, MapKind::Thickness, MapKind::Id];

    /// The stable name used in file names and by callers of [`super::bake`] (`ambient_occlusion`, …).
    pub fn name(self) -> &'static str {
        match self {
            MapKind::Normal => "normal",
            MapKind::WorldNormal => "world_normal",
            MapKind::Position => "position",
            MapKind::AmbientOcclusion => "ambient_occlusion",
            MapKind::Curvature => "curvature",
            MapKind::Thickness => "thickness",
            MapKind::Id => "id",
        }
    }

    /// The kind with this [`MapKind::name`].
    pub fn from_name(name: &str) -> Option<MapKind> {
        MapKind::ALL.into_iter().find(|k| k.name() == name)
    }

    fn bit(self) -> u8 {
        1 << (self as u8)
    }
}

impl fmt::Display for MapKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Which maps one bake makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MapSelection(u8);

impl MapSelection {
    pub fn all() -> MapSelection {
        MapSelection::of(&MapKind::ALL)
    }

    pub fn none() -> MapSelection {
        MapSelection(0)
    }

    pub fn of(kinds: &[MapKind]) -> MapSelection {
        MapSelection(kinds.iter().fold(0, |bits, k| bits | k.bit()))
    }

    pub fn contains(self, kind: MapKind) -> bool {
        self.0 & kind.bit() != 0
    }

    pub fn with(self, kind: MapKind) -> MapSelection {
        MapSelection(self.0 | kind.bit())
    }

    pub fn without(self, kind: MapKind) -> MapSelection {
        MapSelection(self.0 & !kind.bit())
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The selected kinds in baking order.
    pub fn kinds(self) -> impl Iterator<Item = MapKind> {
        MapKind::ALL.into_iter().filter(move |k| self.contains(*k))
    }
}

impl Default for MapSelection {
    fn default() -> MapSelection {
        MapSelection::all()
    }
}

/// The maps of one bake, rows top first, plus where the low surface found the high one.
#[derive(Debug, Clone, Default)]
pub struct BakedMaps {
    pub maps: BTreeMap<MapKind, Tex>,
    /// Per texel of the output: 1 where a ray found the high mesh, 0 where the low surface was used instead or the
    /// texel lies outside the set (before dilation).
    pub hit_mask: Vec<f32>,
}

impl BakedMaps {
    pub fn get(&self, kind: MapKind) -> Option<&Tex> {
        self.maps.get(&kind)
    }

    /// The maps by [`MapKind::name`], as the original single-call baker returned them.
    pub fn into_named(self) -> BTreeMap<String, Tex> {
        self.maps.into_iter().map(|(k, t)| (k.name().to_string(), t)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_and_names() {
        let s = MapSelection::of(&[MapKind::Normal, MapKind::Id]);
        assert!(s.contains(MapKind::Normal) && s.contains(MapKind::Id) && !s.contains(MapKind::Position));
        assert_eq!(s.kinds().collect::<Vec<_>>(), [MapKind::Normal, MapKind::Id]);
        assert_eq!(s.without(MapKind::Normal).with(MapKind::Curvature).kinds().count(), 2);
        assert!(MapSelection::none().is_empty());
        assert_eq!(MapSelection::all().kinds().count(), 7);
        for k in MapKind::ALL {
            assert_eq!(MapKind::from_name(k.name()), Some(k));
        }
    }
}

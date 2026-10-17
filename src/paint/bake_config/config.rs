//! [`SetBakeConfig`]: how one texture set is baked: the common part ([`CommonBake`]) and one [`MapEntry`] per
//! bakeable map with its flags and its baker's settings ([`MapSettings`]).
//!
//! The settings types are the baker's own ([`crate::bake`]); [`SetBakeConfig::settings`] turns the configuration into
//! the [`BakeSettings`] one bake runs with and [`SetBakeConfig::from_settings`] goes the other way, losslessly.

use crate::bake::{AoSettings, BakeSettings, CurvatureSettings, MapKind, MapSelection, PositionSettings, TangentSource, ThicknessSettings};

/// Where the high-poly detail comes from.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HighPoly {
    /// Bake from the document's own mesh (every part of it).
    pub low_as_high: bool,
    /// High-poly mesh files: absolute paths or file names, looked up below the document's folder.
    pub files: Vec<String>,
    /// Moves the files' points into the document's space; `None` = used as they are.
    pub placement: Option<HighPlacement>,
}

impl HighPoly {
    /// The document's own mesh.
    pub fn low_as_high() -> HighPoly {
        HighPoly { low_as_high: true, files: Vec::new(), placement: None }
    }
}

/// `(p - center) / radius`: how an imported document's mesh was moved into its own space, applied to the
/// high-poly files that were modelled next to the original mesh.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HighPlacement {
    pub center: [f32; 3],
    pub radius: f32,
}

impl HighPlacement {
    pub fn apply(&self, p: [f32; 3]) -> [f32; 3] {
        let r = if self.radius.abs() > f32::EPSILON && self.radius.is_finite() { self.radius } else { 1.0 };
        [(p[0] - self.center[0]) / r, (p[1] - self.center[1]) / r, (p[2] - self.center[2]) / r]
    }
}

/// The settings every map of a set shares (size, rays, sampling, dilation, high-poly source).
#[derive(Debug, Clone, PartialEq)]
pub struct CommonBake {
    /// Output size in texels (square).
    pub size: u32,
    /// Frontal ray distance, relative to the high mesh's bounding-box diagonal.
    pub max_dist: f32,
    /// Rear ray distance (relative); `None` = the frontal one.
    pub rear_dist: Option<f32>,
    /// Dilation width in texels.
    pub padding: u32,
    /// Samples per texel side.
    pub samples: u32,
    pub average_normals: bool,
    pub ignore_backfaces: bool,
    pub high: HighPoly,
}

/// One baker's own settings. The variant says which map it belongs to.
#[derive(Debug, Clone, PartialEq)]
pub enum MapSettings {
    Normal {
        tangents: TangentSource,
    },
    WorldNormal,
    Position(PositionSettings),
    /// `rays` secondary rays looking `distance` (relative) for occluders.
    AmbientOcclusion {
        rays: u32,
        distance: f32,
        ao: AoSettings,
    },
    Curvature(CurvatureSettings),
    Thickness(ThicknessSettings),
    Id,
}

impl MapSettings {
    pub fn kind(&self) -> MapKind {
        match self {
            MapSettings::Normal { .. } => MapKind::Normal,
            MapSettings::WorldNormal => MapKind::WorldNormal,
            MapSettings::Position(_) => MapKind::Position,
            MapSettings::AmbientOcclusion { .. } => MapKind::AmbientOcclusion,
            MapSettings::Curvature(_) => MapKind::Curvature,
            MapSettings::Thickness(_) => MapKind::Thickness,
            MapSettings::Id => MapKind::Id,
        }
    }

    /// The settings of `kind` in `s`.
    fn of(kind: MapKind, s: &BakeSettings) -> MapSettings {
        match kind {
            MapKind::Normal => MapSettings::Normal { tangents: s.tangents },
            MapKind::WorldNormal => MapSettings::WorldNormal,
            MapKind::Position => MapSettings::Position(s.position),
            MapKind::AmbientOcclusion => MapSettings::AmbientOcclusion { rays: s.ao_rays, distance: s.ao_dist, ao: s.ao },
            MapKind::Curvature => MapSettings::Curvature(s.curvature),
            MapKind::Thickness => MapSettings::Thickness(s.thickness),
            MapKind::Id => MapSettings::Id,
        }
    }

    /// Writes these settings into `s`.
    fn apply(&self, s: &mut BakeSettings) {
        match self {
            MapSettings::Normal { tangents } => s.tangents = *tangents,
            MapSettings::Position(p) => s.position = *p,
            MapSettings::AmbientOcclusion { rays, distance, ao } => {
                s.ao_rays = *rays;
                s.ao_dist = *distance;
                s.ao = *ao;
            }
            MapSettings::Curvature(c) => s.curvature = *c,
            MapSettings::Thickness(t) => s.thickness = *t,
            MapSettings::WorldNormal | MapSettings::Id => {}
        }
    }
}

/// The on/off states of one map in one set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapFlags {
    /// Baked when the set is baked.
    pub include: bool,
    /// Shown in the viewport (at most one map per set).
    pub show: bool,
    /// A change of its settings queues a rebake of this map.
    pub auto_rebake: bool,
}

impl Default for MapFlags {
    fn default() -> MapFlags {
        MapFlags { include: true, show: false, auto_rebake: false }
    }
}

/// One map of a set: its flags and its baker's settings.
#[derive(Debug, Clone, PartialEq)]
pub struct MapEntry {
    pub flags: MapFlags,
    settings: MapSettings,
}

impl MapEntry {
    pub fn settings(&self) -> &MapSettings {
        &self.settings
    }

    pub fn kind(&self) -> MapKind {
        self.settings.kind()
    }
}

/// One [`MapEntry`] per [`MapKind`]; an entry's settings always belong to its kind.
#[derive(Debug, Clone, PartialEq)]
pub struct MapEntries {
    normal: MapEntry,
    world_normal: MapEntry,
    position: MapEntry,
    ambient_occlusion: MapEntry,
    curvature: MapEntry,
    thickness: MapEntry,
    id: MapEntry,
}

impl MapEntries {
    fn from_settings(s: &BakeSettings) -> MapEntries {
        let entry = |kind: MapKind| MapEntry { flags: MapFlags { include: s.maps.contains(kind), ..MapFlags::default() }, settings: MapSettings::of(kind, s) };
        MapEntries {
            normal: entry(MapKind::Normal),
            world_normal: entry(MapKind::WorldNormal),
            position: entry(MapKind::Position),
            ambient_occlusion: entry(MapKind::AmbientOcclusion),
            curvature: entry(MapKind::Curvature),
            thickness: entry(MapKind::Thickness),
            id: entry(MapKind::Id),
        }
    }

    pub fn get(&self, kind: MapKind) -> &MapEntry {
        match kind {
            MapKind::Normal => &self.normal,
            MapKind::WorldNormal => &self.world_normal,
            MapKind::Position => &self.position,
            MapKind::AmbientOcclusion => &self.ambient_occlusion,
            MapKind::Curvature => &self.curvature,
            MapKind::Thickness => &self.thickness,
            MapKind::Id => &self.id,
        }
    }

    fn get_mut(&mut self, kind: MapKind) -> &mut MapEntry {
        match kind {
            MapKind::Normal => &mut self.normal,
            MapKind::WorldNormal => &mut self.world_normal,
            MapKind::Position => &mut self.position,
            MapKind::AmbientOcclusion => &mut self.ambient_occlusion,
            MapKind::Curvature => &mut self.curvature,
            MapKind::Thickness => &mut self.thickness,
            MapKind::Id => &mut self.id,
        }
    }

    /// The flags of `kind`, to change them.
    pub fn flags_mut(&mut self, kind: MapKind) -> &mut MapFlags {
        &mut self.get_mut(kind).flags
    }

    /// Replaces the settings of the map they belong to.
    pub fn set_settings(&mut self, settings: MapSettings) {
        let kind = settings.kind();
        self.get_mut(kind).settings = settings;
    }

    /// Every entry, in baking order.
    pub fn iter(&self) -> impl Iterator<Item = &MapEntry> {
        MapKind::ALL.into_iter().map(|k| self.get(k))
    }
}

/// How one texture set is baked. See the module docs.
#[derive(Debug, Clone, PartialEq)]
pub struct SetBakeConfig {
    pub common: CommonBake,
    pub maps: MapEntries,
}

impl Default for SetBakeConfig {
    /// The baker's defaults, every map included, from the document's own mesh.
    fn default() -> SetBakeConfig {
        let mut config = SetBakeConfig::from_settings(&BakeSettings::default());
        config.common.high = HighPoly::low_as_high();
        config
    }
}

impl SetBakeConfig {
    /// The defaults with an output size of `size` texels.
    pub fn with_size(size: u32) -> SetBakeConfig {
        let mut config = SetBakeConfig::default();
        config.common.size = size;
        config
    }

    /// The configuration that bakes exactly like `s` (maps in `s.maps` included), with no high-poly source set.
    pub fn from_settings(s: &BakeSettings) -> SetBakeConfig {
        SetBakeConfig {
            common: CommonBake {
                size: u32::try_from(s.size).unwrap_or(u32::MAX),
                max_dist: s.max_dist,
                rear_dist: s.rear_dist,
                padding: u32::try_from(s.padding).unwrap_or(u32::MAX),
                samples: s.samples,
                average_normals: s.average_normals,
                ignore_backfaces: s.ignore_backfaces,
                high: HighPoly::default(),
            },
            maps: MapEntries::from_settings(s),
        }
    }

    /// The maps included in a bake of the whole set.
    pub fn included(&self) -> MapSelection {
        self.maps.iter().filter(|e| e.flags.include).fold(MapSelection::none(), |s, e| s.with(e.kind()))
    }

    /// The settings a bake of the included maps runs with.
    pub fn settings(&self) -> BakeSettings {
        self.settings_for(self.included())
    }

    /// The settings a bake of `maps` runs with (any maps, included or not: "bake only this map").
    pub fn settings_for(&self, maps: MapSelection) -> BakeSettings {
        let c = &self.common;
        let mut s = BakeSettings {
            size: c.size as usize,
            max_dist: c.max_dist,
            rear_dist: c.rear_dist,
            padding: c.padding as usize,
            samples: c.samples,
            average_normals: c.average_normals,
            ignore_backfaces: c.ignore_backfaces,
            maps,
            ..BakeSettings::default()
        };
        self.maps.iter().for_each(|e| e.settings.apply(&mut s));
        s
    }

    /// The map shown in the viewport, if any.
    pub fn shown(&self) -> Option<MapKind> {
        self.maps.iter().find(|e| e.flags.show).map(MapEntry::kind)
    }
}

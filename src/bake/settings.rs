//! What one bake runs with: the common part (size, ray distances, sampling, padding) and one block per baker.
//!
//! Distances are relative to the bake's scale: the diagonal of the high mesh's bounding box. Defaults are the values
//! the parity measurements against the user's own bakes were calibrated with (MaterialCraft-docs/parity/README.md).

use super::maps::MapSelection;

/// Settings of one bake (one texture set).
#[derive(Debug, Clone, PartialEq)]
pub struct BakeSettings {
    /// Output size in texels (square).
    pub size: usize,
    /// Frontal ray distance: how far outside the low surface the high one may lie (relative).
    pub max_dist: f32,
    /// Rear ray distance: how far inside the low surface the high one may lie (relative); `None` = `max_dist`.
    pub rear_dist: Option<f32>,
    /// Secondary rays per texel for ambient occlusion.
    pub ao_rays: u32,
    /// How far ambient occlusion looks for occluders (relative).
    pub ao_dist: f32,
    /// Dilation width in texels around the islands.
    pub padding: usize,
    /// Which maps to make.
    pub maps: MapSelection,
    /// Samples per texel side (1 = one ray per texel; 2 = 2 x 2 averaged). Secondary rays are shared among them.
    pub samples: u32,
    /// Cast rays along normals averaged across hard edges, so split vertices do not leave gaps.
    pub average_normals: bool,
    /// Rays pass through high triangles seen from behind.
    pub ignore_backfaces: bool,
    /// Which tangent frame the tangent-space normal map is written in.
    pub tangents: TangentSource,
    pub ao: AoSettings,
    pub curvature: CurvatureSettings,
    pub thickness: ThicknessSettings,
    pub position: PositionSettings,
}

impl Default for BakeSettings {
    fn default() -> Self {
        BakeSettings {
            size: 1024,
            max_dist: 0.1,
            rear_dist: None,
            ao_rays: 64,
            ao_dist: 0.5,
            padding: 8,
            maps: MapSelection::all(),
            samples: 1,
            average_normals: true,
            ignore_backfaces: true,
            tangents: TangentSource::default(),
            ao: AoSettings::default(),
            curvature: CurvatureSettings::default(),
            thickness: ThicknessSettings::default(),
            position: PositionSettings::default(),
        }
    }
}

impl BakeSettings {
    /// The rear distance actually used.
    pub fn rear(&self) -> f32 {
        self.rear_dist.unwrap_or(self.max_dist)
    }
}

/// The tangent frame of the tangent-space normal map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TangentSource {
    /// The mesh's own per-vertex tangents when it has them (imported with the mesh), else generated ones.
    #[default]
    Mesh,
    /// Always tangents generated from the UV set ([`crate::mesh::MeshPart::generate_tangents`]).
    Generated,
}

/// How secondary rays spread over the hemisphere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RayDistribution {
    /// Every direction equally likely.
    Uniform,
    /// More rays near the normal (cosine-weighted).
    #[default]
    Cosine,
}

/// How much an occluder counts depending on its distance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Attenuation {
    /// Every hit within the distance counts fully.
    None,
    /// Hits count less the farther they are, smoothly falling to 0 at the distance.
    #[default]
    Smooth,
    /// Hits count `1 - t / distance`.
    Linear,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AoSettings {
    pub attenuation: Attenuation,
    pub distribution: RayDistribution,
    /// Opening angle of the ray cone in degrees (180 = the whole hemisphere).
    pub spread: f32,
    /// Occluders nearer than this (relative) are ignored.
    pub min_dist: f32,
    /// Rays pass through high triangles seen from behind (only front faces occlude).
    pub ignore_backfaces: bool,
}

impl Default for AoSettings {
    fn default() -> Self {
        AoSettings { attenuation: Attenuation::Smooth, distribution: RayDistribution::Cosine, spread: 180.0, min_dist: 1e-5, ignore_backfaces: false }
    }
}

/// How curvature values are brought into 0..1 around 0.5.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CurvatureRange {
    /// The strongest curvature of the map (a high percentile, so single spikes don't flatten it) maps to 0 or 1.
    Auto,
    /// This curvature (1 / relative distance) maps to 0 or 1.
    Fixed(f32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CurvatureSettings {
    /// Distance from the surface point at which the neighbourhood is sampled, relative to half the bake's scale (the bounding sphere's radius).
    pub radius: f32,
    /// Samples around each point.
    pub rays: u32,
    pub range: CurvatureRange,
}

impl Default for CurvatureSettings {
    fn default() -> Self {
        CurvatureSettings { radius: 0.001, rays: 32, range: CurvatureRange::Auto }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThicknessSettings {
    /// Rays cast into the mesh per texel.
    pub rays: u32,
    /// How far the rays look (relative to half the bake's scale); misses count as this far.
    pub max_dist: f32,
    pub distribution: RayDistribution,
    /// Stretch the result so the thickest texel of the map is white.
    pub maximise_range: bool,
}

impl Default for ThicknessSettings {
    fn default() -> Self {
        ThicknessSettings { rays: 64, max_dist: 0.1, distribution: RayDistribution::Cosine, maximise_range: true }
    }
}

/// How positions are normalised into 0..1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PositionMode {
    /// One scale for all axes, centred on the bounding box: the longest side spans 0..1, the others less.
    #[default]
    Uniform,
    /// Each axis stretched over its own extent of the bounding box.
    PerAxis,
}

/// Which bounding box the position map is normalised to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PositionBounds {
    /// The whole low mesh (every texture set), so positions agree across sets.
    #[default]
    Scene,
    /// Only the baked texture set's parts.
    Set,
    /// The high mesh.
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PositionSettings {
    pub mode: PositionMode,
    pub bounds: PositionBounds,
}

//! The data of the effect kinds that are the same in content and mask stacks.

use super::EffectDomain;
use crate::json::Value;
use crate::paint::BakedKind;

/// A generator or filter: which implementation (an id resolved through the [`super::EffectRegistry`]) and
/// its parameter values as strings, in order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Procedural {
    pub id: String,
    pub params: Vec<(String, String)>,
}

impl Procedural {
    pub fn new(id: impl Into<String>) -> Procedural {
        Procedural { id: id.into(), params: Vec::new() }
    }

    /// The value of parameter `name`, if set.
    pub fn param(&self, name: &str) -> Option<&str> {
        self.params.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }
}

/// Levels per RGB component (grey values use the first): input range, gamma, output range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Levels {
    pub input_min: [f32; 3],
    pub input_max: [f32; 3],
    pub gamma: [f32; 3],
    pub output_min: [f32; 3],
    pub output_max: [f32; 3],
    /// Clamp the result to the output range.
    pub clamp: bool,
}

impl Default for Levels {
    /// The identity: 0..1 in, gamma 1, 0..1 out.
    fn default() -> Levels {
        Levels { input_min: [0.0; 3], input_max: [1.0; 3], gamma: [1.0; 3], output_min: [0.0; 3], output_max: [1.0; 3], clamp: true }
    }
}

/// Selects texels by colour in a baked map.
#[derive(Debug, Clone, PartialEq)]
pub struct ColorSelection {
    /// The baked map the colours are picked from (usually [`BakedKind::Id`]).
    pub map: BakedKind,
    /// Picked colours, in the map's stored encoding.
    pub colors: Vec<[f32; 3]>,
    /// 0..1.
    pub tolerance: f32,
    /// 0..1.
    pub hardness: f32,
}

/// How a polygon-fill pick grows from the clicked triangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickMode {
    Triangle,
    Polygon,
    /// The whole connected piece of mesh.
    Mesh,
    UvIsland,
}

impl PickMode {
    pub const ALL: [PickMode; 4] = [PickMode::Triangle, PickMode::Polygon, PickMode::Mesh, PickMode::UvIsland];

    pub fn id(self) -> &'static str {
        match self {
            PickMode::Triangle => "triangle",
            PickMode::Polygon => "polygon",
            PickMode::Mesh => "mesh",
            PickMode::UvIsland => "uv_island",
        }
    }

    pub fn from_id(id: &str) -> Option<PickMode> {
        PickMode::ALL.into_iter().find(|m| m.id() == id)
    }
}

/// Picked triangles of one mesh part, by part name (so picks survive a mesh re-import) and triangle index.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Pick {
    pub part: String,
    pub triangles: Vec<u32>,
}

/// What a polygon fill covers: picks re-rasterised at the texture set's resolution.
#[derive(Debug, Clone, PartialEq)]
pub struct Selection {
    pub mode: PickMode,
    pub picks: Vec<Pick>,
}

impl Default for Selection {
    fn default() -> Selection {
        Selection { mode: PickMode::Triangle, picks: Vec::new() }
    }
}

/// Fills the selection with values (per channel in content, one grey value in masks).
#[derive(Debug, Clone, PartialEq)]
pub struct PolygonFill<D: EffectDomain> {
    pub selection: Selection,
    pub values: D::Values,
}

/// A named point of a stack. `id` is unique within the texture set; references name it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Anchor {
    pub id: String,
}

/// Reads the content of the anchor with id `anchor`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Reference {
    pub anchor: String,
}

/// An effect of a kind this version does not know, kept as written (the whole JSON object) so that saving
/// puts it back unchanged. Images it may refer to are not kept.
#[derive(Debug, Clone, PartialEq)]
pub struct UnknownEffect {
    pub kind: String,
    pub data: Value,
}

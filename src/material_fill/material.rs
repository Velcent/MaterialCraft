//! [`Material`]: a material reference made ready to render: its graph, parsed, with the fill's parameter
//! overrides applied, and the [`MaterialKey`] its renders are cached under.

use super::reference::{MaterialResolver, ResolveError};
use crate::graph::Graph;
use std::collections::BTreeMap;
use std::fmt;

/// What a cached render depends on: the graph's text, the parameter overrides and the render size. Two
/// materials with equal keys render to the same textures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MaterialKey {
    /// FNV-1a 64 of the graph text and the overrides.
    pub content: u64,
    /// Width and height of the square render.
    pub size: usize,
}

/// Why a material cannot be rendered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaterialError {
    Resolve(ResolveError),
    /// The graph text does not parse or validate.
    Graph(String),
    /// A parameter override names no exposed parameter or is not a number.
    Param {
        name: String,
        reason: String,
    },
    /// Evaluation failed.
    Render(String),
}

impl fmt::Display for MaterialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MaterialError::Resolve(e) => e.fmt(f),
            MaterialError::Graph(e) => write!(f, "material graph: {e}"),
            MaterialError::Param { name, reason } => write!(f, "material parameter {name:?}: {reason}"),
            MaterialError::Render(e) => write!(f, "material render: {e}"),
        }
    }
}

/// A resolved material with its overrides applied.
#[derive(Debug, Clone)]
pub struct Material {
    graph: Graph,
    content: u64,
}

impl Material {
    /// Resolves `reference` and applies `params` (exposed parameter label → number as text, as the fill
    /// stores them).
    pub fn load(resolver: &dyn MaterialResolver, reference: &str, params: &[(String, String)]) -> Result<Material, MaterialError> {
        let text = resolver.resolve(reference).map_err(MaterialError::Resolve)?;
        Material::from_text(&text, params)
    }

    /// The material of graph text `text` with `params` applied.
    pub fn from_text(text: &str, params: &[(String, String)]) -> Result<Material, MaterialError> {
        let mut graph = Graph::parse(text).map_err(|e| MaterialError::Graph(e.to_string()))?;
        let mut values = BTreeMap::new();
        for (name, value) in params {
            let v: f32 = value.trim().parse().map_err(|_| MaterialError::Param { name: name.clone(), reason: format!("{value:?} is not a number") })?;
            if !v.is_finite() {
                return Err(MaterialError::Param { name: name.clone(), reason: "not a finite number".into() });
            }
            if !graph.exposed.iter().any(|e| &e.label == name) {
                return Err(MaterialError::Param { name: name.clone(), reason: "the graph exposes no such parameter".into() });
            }
            values.insert(name.clone(), v);
        }
        graph.apply_params(&values).map_err(|reason| MaterialError::Param { name: String::new(), reason })?;
        let mut hash = Fnv::new();
        hash.write(text.as_bytes());
        for (name, v) in &values {
            hash.write(&[0xff]);
            hash.write(name.as_bytes());
            hash.write(&v.to_bits().to_le_bytes());
        }
        Ok(Material { graph, content: hash.0 })
    }

    pub fn graph(&self) -> &Graph {
        &self.graph
    }

    /// The cache key of a render at `size` × `size`.
    pub fn key(&self, size: usize) -> MaterialKey {
        MaterialKey { content: self.content, size }
    }
}

/// FNV-1a, 64 bit: stable across runs and platforms (unlike `std`'s hasher, which may change).
struct Fnv(u64);

impl Fnv {
    fn new() -> Fnv {
        Fnv(0xcbf2_9ce4_8422_2325)
    }

    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 = (self.0 ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3);
        }
    }
}

//! Triangle meshes imported from other tools: several parts, material slots and UV sets.
//! In Substance Painter a material slot is called a texture set.
//!
//! Conventions (as in glTF): right-handed, counter-clockwise triangles face outwards, UV origin top-left (v grows
//! downwards in the texture; importers of bottom-left sources flip v), tangent along +u with
//! `bitangent = cross(normal, tangent.xyz) * w` pointing along -v.

mod bounds;
mod bvh;
mod derivatives;
mod error;
pub mod gltf;
mod normals;
mod obj;
mod obj_read;
mod surface;
mod tangents;
#[cfg(test)]
mod test_util;
mod topology;
mod validate;
mod viewer;

pub use bounds::{Bounds, Fit, Sphere};
pub use bvh::{Bvh, Hit, Triangles};
pub use error::{Attribute, MeshError, MeshErrorKind};
pub use obj_read::{ObjError, ObjImport};
pub use surface::{MAX_SURFACE_SIZE, SurfaceError, SurfaceKey, SurfaceMap, SurfaceSample, TexelKind, mesh_fingerprint};
pub use topology::{Components, Edge, FaceGroups, Topology, UvEdge, UvEdgeKind, UvEdges};
pub use viewer::ViewportPart;

pub type Vec3 = [f32; 3];

/// A whole model. Parts that share a material slot are painted with the same texture set.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Mesh {
    /// Material slot names; [`MeshPart::material`] indexes into this list.
    pub materials: Vec<String>,
    pub parts: Vec<MeshPart>,
}

/// One indexed triangle list with a single material slot. All vertex arrays have the same length.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MeshPart {
    pub name: String,
    pub material: usize,
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    /// xyz tangent and w handedness (±1). Empty when the source had none.
    pub tangents: Vec<[f32; 4]>,
    /// One list per UV set; set 0 is the one textures are painted on.
    pub uv_sets: Vec<Vec<[f32; 2]>>,
    pub triangles: Vec<[u32; 3]>,
}

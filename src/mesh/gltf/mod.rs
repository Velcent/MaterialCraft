//! glTF 2.0 binary (`.glb`) writing and reading for [`Mesh`].
//!
//! Writing: one node per part (named after the part) holding a glTF mesh with one triangle
//! primitive (POSITION with min/max, NORMAL, TANGENT, TEXCOORD_n, `u32` indices), one glTF
//! material per material slot (named after the slot). Our conventions are glTF's, so values are
//! stored unchanged and a write → read round trip is exact. A part without vertices has no glTF
//! equivalent and becomes a node without a mesh, marked in `extras.materialcraft`; a part with
//! vertices but no triangles becomes a `POINTS` primitive.
//!
//! Reading: everything we write, plus general GLB files: node hierarchies (transforms are baked
//! into the vertices), several primitives per mesh (part `name.i`), triangle strips and fans,
//! interleaved and normalised-integer accessors. Rejected with [`GltfError::Unsupported`]:
//! external or data-URI buffers, sparse accessors, required extensions (Draco, meshopt, …),
//! point/line primitives other than our marker. Skins, morph targets, colours and cameras are
//! ignored. Primitives without a material get a slot named `default`.

mod accessor;
mod error;
mod glb;
mod read;
mod transform;
mod write;

#[cfg(test)]
mod tests;

pub use error::GltfError;

use super::Mesh;
use crate::json;

/// Key under a node's `extras` for MaterialCraft data.
const EXTRAS_KEY: &str = "materialcraft";
/// `asset.generator` of written files.
const GENERATOR: &str = concat!("MaterialCraft ", env!("CARGO_PKG_VERSION"));

impl Mesh {
    /// The mesh as a glTF 2.0 binary file. Fails if the mesh does not validate or is 4 GiB or
    /// larger. Same mesh, same bytes.
    pub fn to_glb(&self) -> Result<Vec<u8>, GltfError> {
        let (root, bin) = write::to_gltf(self)?;
        glb::Glb::assemble(&root.to_string(), &bin)
    }

    /// Reads a glTF 2.0 binary file (see the module docs for what is supported).
    pub fn from_glb(bytes: &[u8]) -> Result<Mesh, GltfError> {
        let glb = glb::Glb::parse(bytes)?;
        let text = std::str::from_utf8(glb.json).map_err(|_| GltfError::Json("JSON chunk is not UTF-8".into()))?;
        let root = json::parse(text).map_err(|e| GltfError::Json(e.to_string()))?;
        read::GltfReader::new(&root, glb.bin.unwrap_or(&[]))?.read()
    }
}

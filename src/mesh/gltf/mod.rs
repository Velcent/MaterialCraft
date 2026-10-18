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
//! point/line primitives other than our marker. Skins, morph targets and colours are ignored;
//! cameras are read separately ([`read_cameras`], [`Mesh::to_glb_with_cameras`]). Primitives
//! without a material get a slot named `default`.

mod accessor;
mod cameras;
mod error;
mod glb;
mod read;
mod transform;
mod write;

#[cfg(test)]
mod camera_tests;
#[cfg(test)]
mod tests;

pub use error::GltfError;

use super::Mesh;
use crate::camera::SceneCamera;
use crate::json;

/// Key under a node's `extras` for MaterialCraft data.
const EXTRAS_KEY: &str = "materialcraft";
/// `asset.generator` of written files.
const GENERATOR: &str = concat!("MaterialCraft ", env!("CARGO_PKG_VERSION"));

impl Mesh {
    /// The mesh as a glTF 2.0 binary file. Fails if the mesh does not validate or is 4 GiB or
    /// larger. Same mesh, same bytes.
    pub fn to_glb(&self) -> Result<Vec<u8>, GltfError> {
        self.to_glb_with_cameras(&[])
    }

    /// The mesh and `cameras` (in the mesh's coordinates) as a glTF 2.0 binary file.
    pub fn to_glb_with_cameras(&self, cameras: &[SceneCamera]) -> Result<Vec<u8>, GltfError> {
        let (mut root, bin) = write::to_gltf(self)?;
        cameras::write(&mut root, cameras);
        glb::Glb::assemble(&root.to_string(), &bin)
    }

    /// Reads a glTF 2.0 binary file (see the module docs for what is supported).
    pub fn from_glb(bytes: &[u8]) -> Result<Mesh, GltfError> {
        let glb = glb::Glb::parse(bytes)?;
        let root = json_of(&glb)?;
        read::GltfReader::new(&root, glb.bin.unwrap_or(&[]))?.read()
    }
}

/// The cameras of a glTF 2.0 binary file's default scene, in the mesh's coordinates (see `cameras`).
pub fn read_cameras(bytes: &[u8]) -> Result<Vec<SceneCamera>, GltfError> {
    cameras::read(&json_of(&glb::Glb::parse(bytes)?)?)
}

fn json_of(glb: &glb::Glb) -> Result<json::Value, GltfError> {
    let text = std::str::from_utf8(glb.json).map_err(|_| GltfError::Json("JSON chunk is not UTF-8".into()))?;
    json::parse(text).map_err(|e| GltfError::Json(e.to_string()))
}

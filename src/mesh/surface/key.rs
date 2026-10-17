//! What a surface map depends on, so callers can cache it: one map per key.

use super::super::Mesh;

/// Everything a [`super::SurfaceMap`] is computed from besides the mesh itself. Two maps with equal keys over the
/// same mesh revision are identical.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfaceKey {
    /// The texture set: parts whose `material` is this slot.
    pub material: usize,
    /// UV set the texture is painted on (0 for Painter imports).
    pub uv_set: usize,
    pub width: u32,
    pub height: u32,
    /// Gutter width in texels around each island (8 matches the bake padding).
    pub gutter: u32,
    /// Changes whenever the mesh changes: a document's revision counter, or [`mesh_fingerprint`].
    pub mesh_revision: u64,
}

impl SurfaceKey {
    /// A square map of `size` texels with the default 8-texel gutter, keyed by the mesh's content.
    pub fn square(mesh: &Mesh, material: usize, size: u32) -> SurfaceKey {
        SurfaceKey { material, uv_set: 0, width: size, height: size, gutter: 8, mesh_revision: mesh_fingerprint(mesh, material) }
    }
}

/// A content hash (64-bit FNV-1a) of the parts of texture set `material`: positions, normals, tangents, UVs and
/// triangles. Stable across runs, so it can also key caches on disk.
pub fn mesh_fingerprint(mesh: &Mesh, material: usize) -> u64 {
    let mut h = Fnv(0xcbf2_9ce4_8422_2325);
    for (i, part) in mesh.parts.iter().enumerate().filter(|(_, p)| p.material == material) {
        h.u64(i as u64);
        let floats = |h: &mut Fnv, len: usize, it: &mut dyn Iterator<Item = f32>| {
            h.u64(len as u64);
            it.for_each(|f| h.u32(f.to_bits()));
        };
        floats(&mut h, part.positions.len(), &mut part.positions.iter().flatten().copied());
        floats(&mut h, part.normals.len(), &mut part.normals.iter().flatten().copied());
        floats(&mut h, part.tangents.len(), &mut part.tangents.iter().flatten().copied());
        h.u64(part.uv_sets.len() as u64);
        for set in &part.uv_sets {
            floats(&mut h, set.len(), &mut set.iter().flatten().copied());
        }
        h.u64(part.triangles.len() as u64);
        part.triangles.iter().flatten().for_each(|&i| h.u32(i));
    }
    h.0
}

struct Fnv(u64);

impl Fnv {
    fn u32(&mut self, v: u32) {
        for b in v.to_le_bytes() {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
    }

    fn u64(&mut self, v: u64) {
        self.u32(v as u32);
        self.u32((v >> 32) as u32);
    }
}

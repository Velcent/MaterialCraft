//! The surface map (also called the texel atlas): a texture set's parts rasterised into its UV space, so every
//! texel knows which point of the mesh it shows.
//!
//! Per texel it stores only a triangle reference, barycentrics and coverage; position, interpolated vertex normal,
//! tangent, part, triangle and UV-island id are derived on query from a per-triangle table, which keeps a 4K map at
//! 16 bytes per texel. Texels around each island (the gutter) refer to the nearest island triangle extended beyond
//! its edge (see `gutter`). Build one per [`SurfaceKey`] and share it: projections, generators, filters, brushes,
//! polygon fill and the baker all read the same map.
//!
//! Texel `(x, y)` covers UV `x / width .. (x + 1) / width`, `y / height .. (y + 1) / height`, rows top first, as the
//! mesh's top-left UV origin (see the module docs of `mesh`).

mod gutter;
mod key;
mod raster;
mod source;
#[cfg(test)]
mod tests;

pub use key::{SurfaceKey, mesh_fingerprint};

use super::normals::unit_or;
use super::tangents::any_perpendicular;
use super::{Mesh, Vec3};
use crate::v3::{add, dot, mul, sub};
use raster::{NONE, Texel};
use source::SurfaceTriangle;
use std::fmt;

/// The largest map side accepted.
pub const MAX_SURFACE_SIZE: u32 = 16384;

/// Why a surface map could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurfaceError {
    /// No material slot and no part with this index.
    NoSuchMaterial { material: usize },
    /// A part of the set lacks the UV set (or it has the wrong length).
    MissingUvSet { part: usize, set: usize },
    /// Width or height is 0 or above [`MAX_SURFACE_SIZE`].
    BadSize { width: u32, height: u32 },
}

impl fmt::Display for SurfaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SurfaceError::NoSuchMaterial { material } => write!(f, "texture set {material} does not exist"),
            SurfaceError::MissingUvSet { part, set } => write!(f, "mesh part {part} has no usable UV set {set}"),
            SurfaceError::BadSize { width, height } => write!(f, "surface map size {width} x {height} is not between 1 and {MAX_SURFACE_SIZE}"),
        }
    }
}

impl std::error::Error for SurfaceError {}

/// Whether a texel lies on the set or in the gutter around it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TexelKind {
    /// At least part of the texel is covered by a triangle of the set.
    Covered,
    /// Outside every island, within the gutter: the surface extended from the nearest island.
    Gutter,
}

/// The surface point under one texel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceSample {
    /// Object-space position.
    pub position: Vec3,
    /// Interpolated vertex normal (unit length), not a normal map.
    pub normal: Vec3,
    /// Interpolated tangent orthogonalised against `normal`, with the handedness of the triangle in w.
    pub tangent: [f32; 4],
    /// Index into `Mesh::parts`.
    pub part: usize,
    /// Index into that part's `triangles`.
    pub triangle: usize,
    /// Weights of the triangle's corners 0, 1, 2 (sum 1; outside 0..1 in the gutter and at partly covered edges).
    pub barycentric: [f32; 3],
    /// UV island, numbered across the set: the first part's islands, then the next part's.
    pub island: u32,
    /// Covered share of the texel, 0..1 (0 in the gutter).
    pub coverage: f32,
    pub kind: TexelKind,
}

/// A texture set rasterised into its UV space. Immutable; rebuild it when its [`SurfaceKey`] changes.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceMap {
    key: SurfaceKey,
    texels: Vec<Texel>,
    triangles: Vec<SurfaceTriangle>,
    island_count: u32,
    /// The names of all of the mesh's parts, by part index (polygon-fill picks name their part).
    part_names: Vec<String>,
}

impl SurfaceMap {
    /// Rasterises the parts of `key.material` at `key.width` x `key.height` and fills the gutter.
    pub fn build(mesh: &Mesh, key: SurfaceKey) -> Result<SurfaceMap, SurfaceError> {
        let (w, h) = (key.width, key.height);
        if w == 0 || h == 0 || w > MAX_SURFACE_SIZE || h > MAX_SURFACE_SIZE {
            return Err(SurfaceError::BadSize { width: w, height: h });
        }
        let (triangles, island_count) = source::gather(mesh, key.material, key.uv_set)?;
        let (w, h) = (w as usize, h as usize);
        let mut texels = raster::rasterise(&triangles, w, h);
        gutter::fill(&mut texels, &triangles, w, h, key.gutter);
        let part_names = mesh.parts.iter().map(|p| p.name.clone()).collect();
        Ok(SurfaceMap { key, texels, triangles, island_count, part_names })
    }

    pub fn key(&self) -> &SurfaceKey {
        &self.key
    }

    pub fn width(&self) -> usize {
        self.key.width as usize
    }

    pub fn height(&self) -> usize {
        self.key.height as usize
    }

    /// The name of mesh part `part` (an index of [`SurfaceSample::part`]).
    pub fn part_name(&self, part: usize) -> Option<&str> {
        self.part_names.get(part).map(String::as_str)
    }

    /// The part and triangle of texel `i` (row-major) and its covered share, or `None` outside the set and its
    /// gutter: the lookup polygon fill rasterises picks with.
    pub fn triangle_index(&self, i: usize) -> Option<(usize, usize, f32)> {
        let texel = self.texels.get(i)?;
        let tri = self.triangles.get(texel.tri as usize)?;
        Some((tri.part as usize, tri.triangle as usize, texel.coverage))
    }

    /// How many UV islands the set has.
    pub fn island_count(&self) -> u32 {
        self.island_count
    }

    /// The surface point under texel `(x, y)`, or `None` outside the set and its gutter (or the image).
    pub fn sample(&self, x: usize, y: usize) -> Option<SurfaceSample> {
        if x >= self.width() {
            return None;
        }
        self.sample_index(y.checked_mul(self.width())?.checked_add(x)?)
    }

    /// [`SurfaceMap::sample`] by row-major texel index `y * width + x`.
    pub fn sample_index(&self, i: usize) -> Option<SurfaceSample> {
        let texel = self.texels.get(i)?;
        let tri = self.triangles.get(texel.tri as usize)?;
        let [b1, b2] = texel.bary;
        let b = [1.0 - b1 - b2, b1, b2];
        let lerp = |v: [Vec3; 3]| add(add(mul(v[0], b[0]), mul(v[1], b[1])), mul(v[2], b[2]));
        let normal = unit_or(lerp(tri.n), unit_or(face_normal(tri), [0.0, 0.0, 1.0]));
        let t = lerp(tri.t.map(|t| [t[0], t[1], t[2]]));
        let t = unit_or(sub(t, mul(normal, dot(t, normal))), any_perpendicular(normal));
        let w = if tri.t[0][3] < 0.0 { -1.0 } else { 1.0 };
        Some(SurfaceSample {
            position: lerp(tri.p),
            normal,
            tangent: [t[0], t[1], t[2], w],
            part: tri.part as usize,
            triangle: tri.triangle as usize,
            barycentric: b,
            island: tri.island,
            coverage: texel.coverage,
            kind: if texel.coverage > 0.0 { TexelKind::Covered } else { TexelKind::Gutter },
        })
    }

    /// Position, interpolated vertex normal (unit length) and covered share of texel `i` (row-major), or `None`
    /// outside the set and its gutter: the part of [`SurfaceMap::sample_index`] that per-texel loops over many
    /// texels (brush stamps) need, without the tangent frame.
    pub fn point_index(&self, i: usize) -> Option<(Vec3, Vec3, f32)> {
        let texel = self.texels.get(i)?;
        let tri = self.triangles.get(texel.tri as usize)?;
        let [b1, b2] = texel.bary;
        let b = [1.0 - b1 - b2, b1, b2];
        let lerp = |v: [Vec3; 3]| add(add(mul(v[0], b[0]), mul(v[1], b[1])), mul(v[2], b[2]));
        let normal = unit_or(lerp(tri.n), unit_or(face_normal(tri), [0.0, 0.0, 1.0]));
        Some((lerp(tri.p), normal, texel.coverage))
    }

    /// The point of texel `i`'s triangle nearest to its centre's surface point: the same as the position of
    /// [`SurfaceMap::point_index`] inside the triangle, but for edge and gutter texels (whose centres extend the
    /// triangle's plane beyond its edges) a point on the triangle itself. Visibility tests use it: an extended plane
    /// can dip behind the neighbouring surface at a concave crease.
    pub fn point_on_triangle_index(&self, i: usize) -> Option<Vec3> {
        let texel = self.texels.get(i)?;
        let tri = self.triangles.get(texel.tri as usize)?;
        let [b1, b2] = texel.bary;
        let b = [(1.0 - b1 - b2).max(0.0), b1.max(0.0), b2.max(0.0)];
        let sum = b[0] + b[1] + b[2];
        let b = if sum > 0.0 { b.map(|v| v / sum) } else { [1.0 / 3.0; 3] };
        Some(add(add(mul(tri.p[0], b[0]), mul(tri.p[1], b[1])), mul(tri.p[2], b[2])))
    }

    /// The covered share of every texel, row-major (0 in the gutter and outside).
    pub fn coverage(&self) -> Vec<f32> {
        self.texels.iter().map(|t| t.coverage).collect()
    }

    /// The UV island of every texel, row-major, `None` outside the set and its gutter. Island-aware filters use
    /// this to keep neighbourhoods inside one island.
    pub fn islands(&self) -> Vec<Option<u32>> {
        self.texels.iter().map(|t| if t.tri == NONE { None } else { self.triangles.get(t.tri as usize).map(|tri| tri.island) }).collect()
    }

    /// The UV island of texel `i` (row-major), `None` outside the set and its gutter (or the image): one entry of
    /// [`SurfaceMap::islands`], for filters that look at a tile's texels only.
    pub fn island_index(&self, i: usize) -> Option<u32> {
        let texel = self.texels.get(i)?;
        if texel.tri == NONE { None } else { self.triangles.get(texel.tri as usize).map(|tri| tri.island) }
    }
}

fn face_normal(t: &SurfaceTriangle) -> Vec3 {
    crate::v3::cross(sub(t.p[1], t.p[0]), sub(t.p[2], t.p[0]))
}

//! Screen ↔ mesh for one camera snapshot: the one abstraction brushes, projection painting, stencils,
//! eyedroppers, polygon fill and orbit-around-cursor share (ADR 0006 Q-PJ-07: projection owns it, the brush tools
//! own strokes and how projected paint is applied).
//!
//! - [`ViewProjector`]: pick (screen → surface point), to_screen (surface point → screen with depth), and
//!   visibility (occlusion and facing). [`CpuViewProjector`] answers on the CPU with the mesh's BVH, so headless
//!   tests and replays of stored strokes work without a GPU; a GPU id/depth buffer can implement the trait later.
//! - [`ViewScene`]: the per-mesh part of the CPU projector (one BVH per part, the viewport's fit), built once per
//!   mesh and paired with each frame's [`Camera`].
//! - [`ScreenImageTransform`]: the move/turn/scale/tile state of an image laid over the view, shared by stencils
//!   and the projection tool; [`ScreenProjector`] is the [`super::Projector`] that maps texels through the view
//!   onto such an image.
//! - [`compare_depth`]: checks a projector against a depth buffer drawn through the same camera (the GPU's).
//!
//! Spaces: surface positions and normals are in the mesh's own (object) space, as [`super::SurfacePoint`] and the
//! surface map use them; the camera looks at the *fitted* model the viewport draws (`Fit` maps object → world).
//! Screen positions are in the camera's viewport units, origin top-left, y down.

mod cpu;
mod depth_check;
mod scene;
mod screen;
#[cfg(test)]
mod tests;

pub use cpu::CpuViewProjector;
pub use depth_check::{DepthAgreement, DepthBuffer, PixelAgreement, compare_depth};
pub use scene::ViewScene;
pub use screen::{ScreenImageTransform, ScreenProjector};

use crate::camera::Camera;
use crate::mesh::Vec3;

/// What lies under a screen position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceHit {
    /// Mesh part index.
    pub part: usize,
    /// Triangle index within the part.
    pub triangle: u32,
    /// Weights of the triangle's three corners (sum 1).
    pub barycentric: [f32; 3],
    /// Texture coordinate in the part's painted UV set (zero when the part has none).
    pub uv: [f32; 2],
    /// Object-space position.
    pub position: Vec3,
    /// Unit object-space normal: the interpolated vertex normal, or the face normal when the part has none.
    pub normal: Vec3,
    /// Distance from the camera ray's origin (on the near plane) to the hit, in world units.
    pub distance: f32,
}

/// An object-space point on screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenPoint {
    /// Screen position (viewport units, origin top-left, y down).
    pub pos: [f32; 2],
    /// Window depth as the GPU's depth buffer stores it (0 near … 1 far).
    pub depth: f32,
    /// Distance in front of the camera along its viewing direction (world units).
    pub view_depth: f32,
}

/// Screen ↔ mesh for one camera snapshot.
pub trait ViewProjector: Send + Sync {
    /// The camera this projector looks through.
    fn camera(&self) -> &Camera;

    /// The nearest surface under `screen`, or `None` where the ray misses every shown part.
    fn pick(&self, screen: [f32; 2]) -> Option<SurfaceHit>;

    /// Where the object-space point `pos` lands on screen, or `None` when it is behind the camera.
    fn to_screen(&self, pos: Vec3) -> Option<ScreenPoint>;

    /// The camera ray through `screen` in object space (from the near plane, unit direction), for gizmo drags;
    /// `None` for a singular camera.
    fn ray(&self, screen: [f32; 2]) -> Option<super::gizmo::Ray>;

    /// True if another surface lies between the object-space point `pos` and the camera.
    fn occluded(&self, pos: Vec3) -> bool;

    /// Cosine between the object-space `normal` at `pos` and the direction towards the camera (−1 … 1).
    fn facing(&self, pos: Vec3, normal: Vec3) -> f32;

    /// How visible a surface point is, 0 … 1: 0 behind the camera, occluded or facing away; otherwise how
    /// directly it faces the camera.
    fn visibility(&self, pos: Vec3, normal: Vec3) -> f32 {
        if self.to_screen(pos).is_none() || self.occluded(pos) {
            return 0.0;
        }
        self.facing(pos, normal).max(0.0)
    }
}

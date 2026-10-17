//! The volume-based projectors, prepared for one pass: tri-planar, planar, spherical and cylindrical
//! (requirements/projections.md §3.1). Each starts from the surface point in the volume's local frame
//! ([`VolumeTransform`]: −0.5 … 0.5 across the volume), turns it into *projection coordinates* (0 … 1 across the
//! projection once, rows top first, like UV) and applies the image placement last.
//!
//! | Mode | Projection coordinates | Coverage |
//! |---|---|---|
//! | planar | `(x + ½, ½ − y)`: seen from +z, looking along −z | clip, depth falloff on z, facing falloff on the normal against +z |
//! | tri-planar | per plane, as seen from the side the normal faces (never mirrored): x `(½ ∓ z, ½ − y)`, y `(x + ½, ½ ± z)`, z `(½ ± x, ½ − y)`; weights by the local normal | clip |
//! | spherical | longitude from +z towards +x → u, latitude → v (top at +y) | clip |
//! | cylindrical | angle around local y from +z → u over the coverage angle, `½ − y` → v | clip, outside the angle, facing on the radial direction, caps |
//!
//! Footprints: the texel's object-space size ([`SurfacePoint::world_len`]) in projection units, times the
//! placement's scale; without it the texel's UV size stands in.

use super::falloff::{self, tri_planar_weights};
use super::modes::{FacingFalloff, Falloff};
use super::placement::UvPlacement;
use super::projector::{SurfacePoint, Tap, Taps};
use super::volume::{HALF, VolumeTransform};
use std::f32::consts::{PI, TAU};

/// Below this the normal's component along a cylinder's axis counts as a cap (45°).
const CAP_COS: f32 = std::f32::consts::FRAC_1_SQRT_2;

/// The texel's size in local units (UV size as a stand-in without an object-space size).
#[inline]
fn local_footprint(vol: &VolumeTransform, p: &SurfacePoint) -> f32 {
    if p.world_len > 0.0 && p.world_len.is_finite() { vol.local_len(p.world_len) } else { p.texel_len }
}

#[inline]
fn tap(place: &UvPlacement, coords: [f32; 2], weight: f32, footprint: f32) -> Tap {
    Tap { uv: place.apply(coords), weight, footprint: footprint * place.scale() }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreparedPlanar {
    pub vol: VolumeTransform,
    pub place: UvPlacement,
    pub depth: Falloff,
    pub facing: FacingFalloff,
}

impl PreparedPlanar {
    #[inline]
    pub fn project(&self, p: &SurfacePoint) -> Taps {
        let l = self.vol.local(p.pos);
        let n = self.vol.local_normal(p.normal);
        let cos = if n == [0.0; 3] { 1.0 } else { n[2] };
        let coverage = self.vol.clip_coverage(l) * falloff::depth(&self.depth, l[2], HALF) * falloff::facing(&self.facing, cos);
        let mut t = Taps::one([0.0; 2], 0.0);
        t.taps[0] = tap(&self.place, [l[0] + 0.5, 0.5 - l[1]], 1.0, local_footprint(&self.vol, p));
        t.coverage = coverage;
        t
    }
}

/// The projection coordinates of local point `l` on the planes facing x, y and z, each as seen from the side the
/// local normal `n` faces, so no plane shows its image mirrored.
#[inline]
pub fn tri_planar_planes(l: [f32; 3], n: [f32; 3]) -> [[f32; 2]; 3] {
    let sign = |c: f32| if c < 0.0 { -1.0 } else { 1.0 };
    [[0.5 - l[2] * sign(n[0]), 0.5 - l[1]], [l[0] + 0.5, 0.5 + l[2] * sign(n[1])], [0.5 + l[0] * sign(n[2]), 0.5 - l[1]]]
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreparedTriPlanar {
    pub vol: VolumeTransform,
    /// The placement without offset (tri-planar has none, ADR 0008).
    pub place: UvPlacement,
    pub hardness: f32,
}

impl PreparedTriPlanar {
    #[inline]
    pub fn project(&self, p: &SurfacePoint) -> Taps {
        let l = self.vol.local(p.pos);
        let n = self.vol.local_normal(p.normal);
        let w = tri_planar_weights(n, self.hardness);
        let coords = tri_planar_planes(l, n);
        let footprint = local_footprint(&self.vol, p);
        let mut out = Taps { coverage: self.vol.clip_coverage(l), ..Taps::default() };
        let mut k = 0usize;
        for (c, weight) in coords.into_iter().zip(w) {
            if weight > 0.0
                && let Some(slot) = out.taps.get_mut(k)
            {
                *slot = tap(&self.place, c, weight, footprint);
                k += 1;
            }
        }
        out.n = k as u8;
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreparedSpherical {
    pub vol: VolumeTransform,
    pub place: UvPlacement,
}

impl PreparedSpherical {
    #[inline]
    pub fn project(&self, p: &SurfacePoint) -> Taps {
        let l = self.vol.local(p.pos);
        let r = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt();
        let lon = l[0].atan2(l[2]);
        let lat = if r > 1e-9 { (l[1] / r).clamp(-1.0, 1.0).asin() } else { 0.0 };
        let footprint = local_footprint(&self.vol, p) / (PI * r.max(1e-3));
        let mut t = Taps::one([0.0; 2], 0.0);
        t.taps[0] = tap(&self.place, [0.5 + lon / TAU, 0.5 - lat / PI], 1.0, footprint);
        t.coverage = self.vol.clip_coverage(l);
        t
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreparedCylindrical {
    pub vol: VolumeTransform,
    pub place: UvPlacement,
    pub facing: FacingFalloff,
    /// Radians of the circumference the image covers (0 < … ≤ 2π).
    pub coverage_angle: f32,
    pub hide_caps: bool,
}

impl PreparedCylindrical {
    #[inline]
    pub fn project(&self, p: &SurfacePoint) -> Taps {
        let l = self.vol.local(p.pos);
        let n = self.vol.local_normal(p.normal);
        let angle = l[0].atan2(l[2]);
        let r = (l[0] * l[0] + l[2] * l[2]).sqrt();
        let inside_angle = angle.abs() <= 0.5 * self.coverage_angle + 1e-6;
        let radial_cos = match (n == [0.0; 3], r > 1e-9) {
            (false, true) => (n[0] * l[0] + n[2] * l[2]) / r,
            _ => 1.0,
        };
        let cap = self.hide_caps && n[1].abs() > CAP_COS;
        let mut coverage = self.vol.clip_coverage(l) * falloff::facing(&self.facing, radial_cos);
        if !inside_angle || cap {
            coverage = 0.0;
        }
        let local = local_footprint(&self.vol, p);
        let footprint = local.max(local / (self.coverage_angle * r.max(1e-3)));
        let mut t = Taps::one([0.0; 2], 0.0);
        t.taps[0] = tap(&self.place, [0.5 + angle / self.coverage_angle, 0.5 - l[1]], 1.0, footprint);
        t.coverage = coverage;
        t
    }
}

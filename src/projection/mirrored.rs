//! [`Mirrored`]: fill symmetry (PJ-22, PT-40) as a wrapper around any projector. The fill shows its projection
//! once more at every symmetric copy of the shared [`SymmetrySpec`] (a mirror plane or radial copies, the same
//! settings the brush uses): each surface point is projected as itself and as its copies, and the copy whose
//! projection covers the point most wins (ties keep the earlier copy, the direct one first). Copies that flip
//! (alternate radial copies) flip their image about its centre.

use super::data::Projection;
use super::frame::SceneFrame;
use super::projector::{NotComputable, Prepared, Projector, SurfacePoint, Taps};
use crate::symmetry::{CopyTransform, SymmetrySpec};

impl Projection {
    /// [`Projection::prepare`] wrapped in the projection's fill symmetry ([`Projection::effective_symmetry`]).
    pub fn prepare_mirrored(&self, frame: Option<&SceneFrame>, has_surface: bool) -> Result<Mirrored<Prepared>, NotComputable> {
        Ok(Mirrored::new(self.prepare(frame, has_surface)?, self.effective_symmetry()))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Mirrored<P> {
    inner: P,
    /// The direct copy first; only it when the spec makes no copies.
    copies: Vec<CopyTransform>,
}

impl<P: Projector> Mirrored<P> {
    pub fn new(inner: P, symmetry: SymmetrySpec) -> Mirrored<P> {
        Mirrored { inner, copies: symmetry.copies() }
    }

    pub fn inner(&self) -> &P {
        &self.inner
    }

    /// True if the wrapper adds copies.
    pub fn is_active(&self) -> bool {
        self.copies.len() > 1
    }
}

impl<P: Projector> Projector for Mirrored<P> {
    #[inline]
    fn project(&self, p: &SurfacePoint) -> Taps {
        let mut best = self.inner.project(p);
        for c in self.copies.iter().skip(1) {
            if best.coverage >= 1.0 {
                break;
            }
            let q = SurfacePoint { pos: c.point(p.pos), normal: c.normal(p.normal), ..*p };
            let mut t = self.inner.project(&q);
            if t.coverage > best.coverage {
                if c.flips() {
                    let n = usize::from(t.n).min(t.taps.len());
                    for tap in t.taps.iter_mut().take(n) {
                        tap.uv = c.frame(tap.uv, [0.5, 0.5]);
                    }
                }
                best = t;
            }
        }
        best
    }
}

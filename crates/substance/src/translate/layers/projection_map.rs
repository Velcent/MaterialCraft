//! The one mapping of Painter's fill projection settings (`DataActionFill`) to ours ([`Projection`]), with how
//! exact it is. Table and evidence: `MaterialCraft-docs/requirements/projections.md`, sections 2 and 3.5.
//!
//! | Painter | Ours | Status |
//! |---|---|---|
//! | `projection` 6 | texture space | exact (ADR 0006 Q-PJ-01: no placement was ever edited in 1,639 fills; a stale guess shows up as a cache disagreement) |
//! | `projection` 0, identity `uvTransformation0` | UV, identity (the same pixels as texture space on one tile) | exact |
//! | `projection` 0 with tiling/offset/rotation | UV with that placement (radians → degrees) | approximated: the rotation/offset convention is not verified |
//! | `projection` 1, 2, 3, 4, 7 | texture space for now; the volume modes are evaluated from step 24 on | approximated |
//! | other values | texture space | approximated (unknown mode) |
//! | `filtering` 2 / 0 | smooth / bilinear | 0 is a guess (nearest or bilinear): approximated where it matters |
//! | `uvSamplingWrap` 0, 1, 2, 3 | repeat none, u, v, both | 1 vs 2 unverified: approximated when 1 or 2 |
//!
//! Painter's raw fields stay in the kept `document.bin` (`foreign/substance`); size modes, the second UV
//! transform and the warp grid are not mapped yet.

use super::computability::Computability;
use crate::model::FillEffect;
use materialcraft::projection::{Filter, ImagePlacement, Projection, ProjectionMode, Repeat};

/// Our projection for a Painter fill and how exactly it stands in for Painter's.
pub struct MappedProjection {
    pub projection: Projection,
    /// Exact or approximated (never not computable: the fill is still shown).
    pub status: Computability,
    /// True if the result depends on the filter (approximated filters only matter then).
    filter_guess: bool,
}

impl MappedProjection {
    /// The status for an image of `image_size` on a set of `set_size`: a guessed filter only matters when the
    /// image is resampled or placed.
    pub fn status_for(&self, image_size: (u32, u32), set_size: (u32, u32)) -> Computability {
        let mut status = self.status.clone();
        let resampled = image_size != set_size || self.projection.mode != ProjectionMode::TextureSpace;
        if self.filter_guess && resampled {
            status.and(&Computability::approximated("Painter filtering 0 assumed bilinear"));
        }
        status
    }
}

/// Painter's mode numbers by our name, for messages (Inferred, see the module table).
fn painter_mode_name(mode: u64) -> &'static str {
    match mode {
        1 => "planar",
        2 => "tri-planar",
        3 => "spherical",
        4 => "cylindrical",
        7 => "warp",
        _ => "unknown",
    }
}

pub fn map_projection(fill: &FillEffect) -> MappedProjection {
    let mut status = Computability::Exact;
    let filter = match fill.filtering {
        2 => Filter::Smooth,
        0 => Filter::Bilinear,
        other => {
            status.and(&Computability::approximated(format!(
                "Painter filtering {other} assumed smooth"
            )));
            Filter::Smooth
        }
    };
    let repeat = match fill.placement.uv_wrap.unwrap_or(3) {
        0 => Repeat::None,
        3 => Repeat::Both,
        1 | 2 => {
            status.and(&Computability::approximated(
                "single-axis repetition (u/v order unverified)",
            ));
            if fill.placement.uv_wrap == Some(1) {
                Repeat::U
            } else {
                Repeat::V
            }
        }
        other => {
            status.and(&Computability::approximated(format!(
                "Painter repetition {other} assumed both"
            )));
            Repeat::Both
        }
    };
    let mode = match fill.projection {
        6 => ProjectionMode::TextureSpace,
        0 => ProjectionMode::Uv,
        other => {
            status.and(&Computability::approximated(format!(
                "projection {other} ({}) not evaluated yet; imported as texture space",
                painter_mode_name(other)
            )));
            ProjectionMode::TextureSpace
        }
    };
    if mode == ProjectionMode::TextureSpace && fill.uv_transform.is_some_and(|t| !t.is_identity()) {
        status.and(&Computability::approximated(
            "UV tiling/offset/rotation of a texture-space fill ignored",
        ));
    }
    let mut image = ImagePlacement::default();
    if mode == ProjectionMode::Uv
        && let Some(t) = fill.uv_transform.filter(|t| !t.is_identity())
    {
        image.tiling = t.scale;
        image.rotation = t.rotation.to_degrees();
        image.offset = t.translation;
        status.and(&Computability::approximated(
            "UV tiling/offset/rotation applied (convention not verified)",
        ));
    }
    MappedProjection {
        projection: Projection {
            mode,
            filter,
            repeat,
            image,
        },
        status,
        filter_guess: fill.filtering == 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EffectHeader, UvTransform};

    fn fill(projection: u64, filtering: u64, uv: Option<UvTransform>) -> FillEffect {
        FillEffect {
            header: EffectHeader::default(),
            sources: Vec::new(),
            projection,
            filtering,
            uv_transform: uv,
            placement: Default::default(),
        }
    }

    #[test]
    fn texture_space_and_identity_uv_are_exact() {
        for (mode, ours) in [(6, ProjectionMode::TextureSpace), (0, ProjectionMode::Uv)] {
            let m = map_projection(&fill(mode, 2, None));
            assert!(m.status.is_exact());
            assert_eq!(m.projection.mode, ours);
            assert!(m.projection.image.is_identity());
            assert!(m.status_for((8, 8), (4, 4)).is_exact());
        }
        let bilinear = map_projection(&fill(6, 0, None));
        assert_eq!(bilinear.projection.filter, Filter::Bilinear);
        assert!(bilinear.status_for((4, 4), (4, 4)).is_exact());
        assert!(!bilinear.status_for((8, 8), (4, 4)).is_exact());
    }

    #[test]
    fn placements_and_volume_modes_are_approximated() {
        let t = UvTransform {
            scale: [2.0, 3.0],
            translation: [0.1, 0.0],
            rotation: std::f32::consts::FRAC_PI_2,
        };
        let m = map_projection(&fill(0, 2, Some(t)));
        assert_eq!(m.projection.mode, ProjectionMode::Uv);
        assert_eq!(m.projection.image.tiling, [2.0, 3.0]);
        assert!((m.projection.image.rotation - 90.0).abs() < 1e-4);
        assert!(!m.status.is_exact());
        let planar = map_projection(&fill(1, 2, None));
        assert_eq!(planar.projection.mode, ProjectionMode::TextureSpace);
        assert!(planar.status.reasons()[0].contains("planar"));
        let mut wrapped = fill(6, 2, None);
        wrapped.placement.uv_wrap = Some(0);
        assert_eq!(map_projection(&wrapped).projection.repeat, Repeat::None);
    }
}

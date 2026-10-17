//! [`SymmetrySpec`]: which copies a stroke (or a fill) makes besides itself.

use super::copy::CopyTransform;
use crate::mesh::Vec3;

/// The most radial copies (PT-38: 2 … 64, the original included).
pub const MAX_RADIAL_COPIES: u32 = 64;

/// Which alternate radial copies are flipped in their own frame (for lettering, PT-38).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Flip {
    #[default]
    None,
    /// Every second copy is flipped along U (the stamp's horizontal axis).
    U,
    /// Every second copy is flipped along V (the stamp's vertical axis).
    V,
}

impl Flip {
    pub const ALL: [Flip; 3] = [Flip::None, Flip::U, Flip::V];

    /// The `[u, v]` flips of a copy with this setting.
    pub fn axes(self) -> [bool; 2] {
        [self == Flip::U, self == Flip::V]
    }

    pub fn label(self) -> &'static str {
        match self {
            Flip::None => "None",
            Flip::U => "Flip U",
            Flip::V => "Flip V",
        }
    }
}

/// Which copies a stroke paints besides itself. Positions are in the mesh's object space; axes are 0 = x, 1 = y,
/// 2 = z.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SymmetrySpec {
    #[default]
    None,
    /// A mirror plane across object axis `axis` at `offset` along it (PT-37).
    Mirror { axis: usize, offset: f32 },
    /// `count` copies (the original included) turned around object axis `axis` through `point`; alternate copies
    /// flipped as `flip` says (PT-38).
    Radial { axis: usize, point: Vec3, count: u32, flip: Flip },
}

impl SymmetrySpec {
    /// True if the spec makes copies (a valid mirror or radial setting).
    pub fn is_active(self) -> bool {
        self.copies().len() > 1
    }

    /// Every copy a stamp is applied with, the direct one first. Invalid settings (an axis past z, a non-finite
    /// offset or point, fewer than two radial copies) give the direct copy alone; radial counts above
    /// [`MAX_RADIAL_COPIES`] are clamped.
    pub fn copies(self) -> Vec<CopyTransform> {
        let mut out = vec![CopyTransform::IDENTITY];
        match self {
            SymmetrySpec::None => {}
            SymmetrySpec::Mirror { axis, offset } => {
                if let Some(m) = CopyTransform::mirror(axis, offset) {
                    out.push(m);
                }
            }
            SymmetrySpec::Radial { axis, point, count, flip } => {
                let n = count.min(MAX_RADIAL_COPIES);
                for k in 1..n {
                    let degrees = k as f64 * 360.0 / f64::from(n);
                    let flips = if k % 2 == 1 { flip.axes() } else { [false; 2] };
                    match CopyTransform::rotation(axis, point, degrees) {
                        Some(r) => out.push(CopyTransform { flip: flips, ..r }),
                        None => return vec![CopyTransform::IDENTITY],
                    }
                }
            }
        }
        out
    }
}

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

    /// The copies as a stroke record stores them: one step transform, how many times it repeats, and the flips
    /// of odd copies; `None` without copies. Invalid settings (an axis past z, a non-finite offset or point, fewer
    /// than two radial copies) make none; radial counts above [`MAX_RADIAL_COPIES`] are clamped.
    pub fn step(self) -> Option<(CopyTransform, u32, [bool; 2])> {
        match self {
            SymmetrySpec::None => None,
            SymmetrySpec::Mirror { axis, offset } => Some((CopyTransform::mirror(axis, offset)?, 1, [false; 2])),
            SymmetrySpec::Radial { axis, point, count, flip } => {
                let n = count.min(MAX_RADIAL_COPIES);
                if n < 2 {
                    return None;
                }
                Some((CopyTransform::rotation(axis, point, 360.0 / f64::from(n))?, n - 1, flip.axes()))
            }
        }
    }

    /// Every copy a stamp is applied with, the direct one first (see [`SymmetrySpec::step`] and [`repeat`]).
    pub fn copies(self) -> Vec<CopyTransform> {
        match self.step() {
            Some((step, n, flip)) => repeat(&step, n, flip),
            None => vec![CopyTransform::IDENTITY],
        }
    }
}

/// The direct copy and `copies` more, copy `i` being `step` applied `i` times; odd copies flip as `flip` says.
/// Copies past [`MAX_RADIAL_COPIES`] − 1 are dropped. Live strokes and replays of their records both build their
/// copies here, so they agree bit for bit.
pub fn repeat(step: &CopyTransform, copies: u32, flip: [bool; 2]) -> Vec<CopyTransform> {
    let n = copies.min(MAX_RADIAL_COPIES - 1);
    let mut out = vec![CopyTransform::IDENTITY];
    let mut current = CopyTransform { flip: [false; 2], ..*step };
    for i in 1..=n {
        let flips = if i % 2 == 1 { flip } else { [false; 2] };
        out.push(CopyTransform { flip: flips, ..current });
        current = step.after(&current);
    }
    out
}

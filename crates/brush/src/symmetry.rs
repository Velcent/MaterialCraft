//! Symmetry as the stamp stage takes it ([`SymmetrySpec`], [`CopyTransform`]s) and as a stroke record stores it
//! (a step transform, how often it repeats, and the flips of odd copies).
//!
//! A stroke's copies are always built from its record form ([`copies`]), live and on replay alike, so a replay
//! paints the same bits; the spec is what the settings edit.

use materialcraft::paint::stroke::Symmetry;
use materialcraft::symmetry::{CopyTransform, SymmetrySpec, repeat};

/// The record's form of `spec`: a mirror is one copy with a reflection matrix, radial symmetry `count − 1` copies
/// of one rotation step.
pub fn to_record(spec: SymmetrySpec) -> Symmetry {
    match spec.step() {
        Some((step, copies, flip)) => Symmetry { copies, transform: step.matrix(), flip },
        None => Symmetry::default(),
    }
}

/// The copies a recorded stroke is painted with, the direct one first: copy `i` is the record's transform applied
/// `i` times (any affine transform, so imported symmetry replays as recorded).
pub fn copies(s: &Symmetry) -> Vec<CopyTransform> {
    match CopyTransform::from_matrix(&s.transform) {
        Some(step) if s.copies > 0 => repeat(&step, s.copies, s.flip),
        _ => vec![CopyTransform::IDENTITY],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft::paint::stroke::IDENTITY;
    use materialcraft::symmetry::Flip;

    #[test]
    fn records_give_the_specs_copies_bit_for_bit() {
        for spec in [
            SymmetrySpec::None,
            SymmetrySpec::Mirror { axis: 0, offset: 0.25 },
            SymmetrySpec::Mirror { axis: 2, offset: -1.0 },
            SymmetrySpec::Radial { axis: 1, point: [0.3, 9.0, -0.2], count: 5, flip: Flip::U },
        ] {
            assert_eq!(copies(&to_record(spec)), spec.copies(), "{spec:?}");
        }
        assert_eq!(to_record(SymmetrySpec::Radial { axis: 1, point: [0.0; 3], count: 6, flip: Flip::V }).copies, 5);
        let none = Symmetry { copies: 0, transform: IDENTITY, flip: [false; 2] };
        assert_eq!(copies(&none).len(), 1);
        let broken = Symmetry { copies: 3, transform: [f32::NAN; 16], flip: [false; 2] };
        assert_eq!(copies(&broken).len(), 1);
    }
}

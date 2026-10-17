//! Symmetry as the stamp stage takes it ([`SymmetrySpec`]) and as a stroke record stores it (a copy count and a
//! transform matrix).

use materialcraft::paint::stroke::{IDENTITY, Symmetry};
use materialcraft::stamp::SymmetrySpec;

/// The record's form of `spec`: a mirror is one copy with a reflection matrix.
pub fn to_record(spec: SymmetrySpec) -> Symmetry {
    match spec {
        SymmetrySpec::Mirror { axis, offset } if axis < 3 => {
            let mut m = IDENTITY;
            if let Some(d) = m.get_mut(axis * 5) {
                *d = -1.0;
            }
            if let Some(t) = m.get_mut(12 + axis) {
                *t = 2.0 * offset;
            }
            Symmetry { copies: 1, transform: m }
        }
        _ => Symmetry::default(),
    }
}

/// The spec a record's symmetry stands for; transforms other than an axis mirror (imported radial symmetry) are not
/// replayed yet and give none.
pub fn from_record(s: &Symmetry) -> SymmetrySpec {
    if s.copies != 1 {
        return SymmetrySpec::None;
    }
    for axis in 0..3 {
        let candidate = to_record(SymmetrySpec::Mirror { axis, offset: s.transform.get(12 + axis).copied().unwrap_or(0.0) * 0.5 });
        if candidate.transform == s.transform {
            return SymmetrySpec::Mirror { axis, offset: s.transform.get(12 + axis).copied().unwrap_or(0.0) * 0.5 };
        }
    }
    SymmetrySpec::None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirrors_round_trip_through_the_record() {
        for spec in [SymmetrySpec::None, SymmetrySpec::Mirror { axis: 0, offset: 0.25 }, SymmetrySpec::Mirror { axis: 2, offset: -1.0 }] {
            assert_eq!(from_record(&to_record(spec)), spec);
        }
        let odd = Symmetry { copies: 3, transform: IDENTITY };
        assert_eq!(from_record(&odd), SymmetrySpec::None);
    }
}

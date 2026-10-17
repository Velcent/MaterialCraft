//! How the final normal map combines its three inputs, as Substance Painter does (Verified against
//! Painter exports, see `interop/substance-painter.md`, "Normals: height filter and combine"):
//!
//! `final = RNM(RNM(baked, height_normal), normal_channel)`
//!
//! with RNM = reoriented normal mapping ([`super::normal::reoriented`]): the height detail is first
//! laid onto the baked mesh normal, then the normal channel onto the result. Flat inputs drop out.
//!
//! All inputs are OpenGL (+Y up), the convention of every normal map in a document.

use super::normal::{self, Vec3};

/// Painter's combination of the baked mesh normal, the height-derived normal and the normal channel
/// (all decoded, OpenGL).
pub fn combine(baked: Vec3, height: Vec3, channel: Vec3) -> Vec3 {
    normal::reoriented(normal::reoriented(baked, height), channel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::normal::{FLAT, normalize};

    fn close(a: Vec3, b: Vec3) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-5)
    }

    #[test]
    fn flat_inputs_drop_out() {
        let n = normalize([0.3, -0.2, 0.9]);
        assert!(close(combine(n, FLAT, FLAT), n));
        assert!(close(combine(FLAT, n, FLAT), n));
        assert!(close(combine(FLAT, FLAT, n), n));
    }

    #[test]
    fn height_is_laid_on_baked_before_the_channel() {
        // RNM is not associative: the order is part of what Painter does.
        let (b, h, c) = (normalize([0.6, 0.1, 1.0]), normalize([0.0, 0.5, 1.0]), normalize([-0.4, 0.2, 1.0]));
        let painter = combine(b, h, c);
        let other = normal::reoriented(b, normal::reoriented(h, c));
        assert!(!close(painter, other));
        assert!(close(painter, normal::reoriented(normal::reoriented(b, h), c)));
    }
}

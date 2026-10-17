use super::guides::{Guide, guide, meeting};
use super::*;
use crate::mesh::test_util::quad;
use crate::mesh::{Bounds, Mesh};

#[test]
fn a_mirror_is_its_own_inverse_and_flips_normals() {
    let m = CopyTransform::mirror(0, 0.25).unwrap();
    let p = [0.75, -0.3, 0.1];
    assert_eq!(m.point(p), [-0.25, -0.3, 0.1]);
    assert_eq!(m.point(m.point(p)), p);
    assert_eq!(m.normal([1.0, 0.0, 0.0]), [-1.0, 0.0, 0.0]);
    assert!(CopyTransform::mirror(3, 0.0).is_none() && CopyTransform::mirror(1, f32::NAN).is_none());
}

#[test]
fn radial_copies_turn_around_the_axis_and_flip_alternately() {
    let spec = SymmetrySpec::Radial { axis: 2, point: [1.0, 0.0, 0.0], count: 4, flip: Flip::U };
    let copies = spec.copies();
    assert_eq!(copies.len(), 4);
    assert!(copies[0].is_identity());
    // a quarter turn around (1, 0, z): (2, 0) → (1, 1), exactly
    assert_eq!(copies[1].point([2.0, 0.0, 0.5]), [1.0, 1.0, 0.5]);
    assert_eq!(copies[2].point([2.0, 0.0, 0.5]), [0.0, 0.0, 0.5]);
    assert_eq!(copies.iter().map(|c| c.flip[0]).collect::<Vec<_>>(), vec![false, true, false, true]);
    assert_eq!(copies[1].frame([12.0, 5.0], [10.0, 0.0]), [8.0, 5.0]);
    // n copies of one step close the circle
    let six = SymmetrySpec::Radial { axis: 1, point: [0.0; 3], count: 6, flip: Flip::None }.copies();
    let mut p = [0.3, 0.2, 0.9];
    for _ in 0..6 {
        p = six[1].point(p);
    }
    assert!((p[0] - 0.3).abs() < 1e-5 && (p[2] - 0.9).abs() < 1e-5, "{p:?}");
    assert_eq!(SymmetrySpec::Radial { axis: 1, point: [0.0; 3], count: 500, flip: Flip::None }.copies().len(), MAX_RADIAL_COPIES as usize);
    assert!(!SymmetrySpec::Radial { axis: 1, point: [0.0; 3], count: 1, flip: Flip::None }.is_active());
    assert!(!SymmetrySpec::Mirror { axis: 7, offset: 0.0 }.is_active());
}

#[test]
fn matrices_hold_the_position_transform() {
    let m = CopyTransform::rotation(2, [1.0, 0.0, 0.0], 90.0).unwrap().matrix();
    let p = [2.0f32, 0.0, 0.0];
    let q = [0, 1, 2].map(|r| m[r] * p[0] + m[4 + r] * p[1] + m[8 + r] * p[2] + m[12 + r]);
    assert_eq!(q, [1.0, 1.0, 0.0]);
    assert_eq!(m[15], 1.0);
}

#[test]
fn guides_span_the_bounds_and_meet_the_mesh() {
    let b = Bounds { min: [-1.0, -1.0, 0.0], max: [1.0, 1.0, 0.0] };
    let Some(Guide::Plane(corners)) = guide(SymmetrySpec::Mirror { axis: 0, offset: 0.5 }, b) else { panic!() };
    assert!(corners.iter().all(|c| c[0] == 0.5));
    assert!(corners.iter().any(|c| c[1] < -1.0) && corners.iter().any(|c| c[1] > 1.0));
    let Some(Guide::Axis([a, z])) = guide(SymmetrySpec::Radial { axis: 2, point: [0.2, 0.1, 0.0], count: 3, flip: Flip::None }, b) else { panic!() };
    assert_eq!((a[0], a[1], z[0]), (0.2, 0.1, 0.2));
    assert!(guide(SymmetrySpec::None, b).is_none());

    let mesh = Mesh { materials: vec!["m".into()], parts: vec![quad()] };
    let cut = meeting(SymmetrySpec::Mirror { axis: 0, offset: 0.5 }, &mesh);
    assert!(!cut.lines.is_empty() && cut.lines.iter().flatten().all(|p| (p[0] - 0.5).abs() < 1e-6));
    let hit = meeting(SymmetrySpec::Radial { axis: 2, point: [0.2, 0.1, 5.0], count: 3, flip: Flip::None }, &mesh);
    assert!(!hit.points.is_empty() && hit.points.iter().all(|p| (p[0] - 0.2).abs() < 1e-6 && p[2] == 0.0));
}

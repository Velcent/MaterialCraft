//! The volume projectors (tri-planar, planar, spherical, cylindrical) and the fill symmetry wrapper.

use super::*;
use crate::symmetry::SymmetrySpec;

/// A scene two units wide around the origin (one scene unit = 2).
const FRAME: SceneFrame = SceneFrame { centre: [0.0; 3], size: [2.0, 2.0, 2.0] };

fn prepared(mode: ProjectionMode) -> Prepared {
    Projection { mode, ..Projection::default() }.prepare(Some(&FRAME), true).unwrap()
}

fn point(pos: [f32; 3], normal: [f32; 3]) -> SurfacePoint {
    SurfacePoint { pos, normal, texel_len: 1.0 / 64.0, world_len: 2.0 / 64.0, ..SurfacePoint::default() }
}

fn close(a: [f32; 2], b: [f32; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-5 && (a[1] - b[1]).abs() < 1e-5
}

#[test]
fn planar_looks_along_minus_z_with_the_image_upright() {
    let p = prepared(ProjectionMode::default_of("planar").unwrap());
    let t = p.project(&point([-1.0, 1.0, 0.0], [0.0, 0.0, 1.0]));
    assert_eq!((t.n, t.coverage), (1, 1.0));
    // the scene's top-left corner seen from +z is the image's top-left corner
    assert!(close(t.taps[0].uv, [0.0, 0.0]));
    assert!(close(p.project(&point([0.5, -0.5, 0.3], [0.0, 0.0, 1.0])).taps[0].uv, [0.75, 0.75]));
    // footprint: one texel of 2/64 object units is 1/64 of the projection
    assert!((t.taps[0].footprint - 1.0 / 64.0).abs() < 1e-6);
}

#[test]
fn planar_falloffs_and_clipping_limit_coverage() {
    let mut m = Planar::default();
    m.volume.scale = [1.0, 1.0, 0.1];
    m.depth_falloff = Falloff { on: true, hardness: 1.0 };
    m.facing_falloff = FacingFalloff { on: true, angle: 90.0, hardness: 1.0 };
    let p = prepared(ProjectionMode::Planar(m));
    assert_eq!(p.project(&point([0.0, 0.0, 0.05], [0.0, 0.0, 1.0])).coverage, 1.0);
    // beyond the volume's depth (0.1 × 2 / 2 = 0.1 on either side)
    assert_eq!(p.project(&point([0.0, 0.0, 0.15], [0.0, 0.0, 1.0])).coverage, 0.0);
    // facing away from the projector
    assert_eq!(p.project(&point([0.0, 0.0, 0.0], [0.0, 0.0, -1.0])).coverage, 0.0);
    let clipped = Planar { volume: Volume { scale: [0.5, 0.5, 1.0], clip: true, ..Volume::default() }, ..Planar::default() };
    let p = prepared(ProjectionMode::Planar(clipped));
    assert_eq!(p.project(&point([0.4, 0.0, 0.0], [0.0, 0.0, 1.0])).coverage, 1.0);
    assert_eq!(p.project(&point([0.6, 0.0, 0.0], [0.0, 0.0, 1.0])).coverage, 0.0);
}

#[test]
fn volume_offset_rotation_and_mirror_move_the_image() {
    let moved = Planar { volume: Volume { offset: [0.25, 0.0, 0.0], ..Volume::default() }, ..Planar::default() };
    // the centre moved by 0.25 scene units = 0.5 object units
    assert!(close(prepared(ProjectionMode::Planar(moved)).project(&point([0.5, 0.0, 0.0], [0.0, 0.0, 1.0])).taps[0].uv, [0.5, 0.5]));
    let turned = Planar { volume: Volume { rotation: [0.0, 0.0, 90.0], ..Volume::default() }, ..Planar::default() };
    // turned 90° about z: the object's +y is the volume's +x (right in the image)
    assert!(close(prepared(ProjectionMode::Planar(turned)).project(&point([0.0, 1.0, 0.0], [0.0, 0.0, 1.0])).taps[0].uv, [1.0, 0.5]));
    let mirrored = Planar { volume: Volume { scale: [-1.0, 1.0, 1.0], ..Volume::default() }, ..Planar::default() };
    assert!(close(prepared(ProjectionMode::Planar(mirrored)).project(&point([1.0, 0.0, 0.0], [0.0, 0.0, 1.0])).taps[0].uv, [0.0, 0.5]));
}

#[test]
fn tri_planar_blends_three_unmirrored_planes() {
    let hard = TriPlanar { blend_hardness: 1.0, ..TriPlanar::default() };
    let p = prepared(ProjectionMode::TriPlanar(hard));
    // a face looking at +z shows the z plane like planar does
    let t = p.project(&point([-1.0, 1.0, 0.0], [0.0, 0.0, 1.0]));
    assert_eq!(t.n, 1);
    assert!(close(t.taps[0].uv, [0.0, 0.0]));
    // the opposite face is not mirrored: right in its view (−x) is +u
    let back = p.project(&point([-1.0, 0.0, 0.0], [0.0, 0.0, -1.0]));
    assert!(close(back.taps[0].uv, [1.0, 0.5]));
    // a face looking at +x: right in its view is −z
    let side = p.project(&point([1.0, 0.0, -1.0], [1.0, 0.0, 0.0]));
    assert!(close(side.taps[0].uv, [1.0, 0.5]));
    let soft = prepared(ProjectionMode::TriPlanar(TriPlanar { blend_hardness: 0.0, ..TriPlanar::default() }));
    let t = soft.project(&point([0.0; 3], [0.6, 0.0, 0.8]));
    assert_eq!(t.n, 2);
    assert!((t.used().iter().map(|t| t.weight).sum::<f32>() - 1.0).abs() < 1e-5);
    // tri-planar always repeats and ignores the image offset
    let pr = Projection { mode: ProjectionMode::TriPlanar(hard), repeat: Repeat::None, ..Projection::default() };
    assert_eq!(pr.sampling_repeat(), Repeat::Both);
    let offset = Projection { image: ImagePlacement { offset: [0.3, 0.3], ..ImagePlacement::default() }, ..pr };
    let a = offset.prepare(Some(&FRAME), true).unwrap().project(&point([0.0; 3], [0.0, 0.0, 1.0]));
    assert!(close(a.taps[0].uv, [0.5, 0.5]));
}

#[test]
fn spherical_and_cylindrical_wrap_around_the_centre() {
    let s = prepared(ProjectionMode::default_of("spherical").unwrap());
    assert!(close(s.project(&point([0.0, 0.0, 1.0], [0.0, 0.0, 1.0])).taps[0].uv, [0.5, 0.5]));
    assert!(close(s.project(&point([1.0, 0.0, 0.0], [1.0, 0.0, 0.0])).taps[0].uv, [0.75, 0.5]));
    assert!(close(s.project(&point([0.0, 1.0, 0.0], [0.0, 1.0, 0.0])).taps[0].uv, [0.5, 0.0]));
    let c = prepared(ProjectionMode::default_of("cylindrical").unwrap());
    assert!(close(c.project(&point([-1.0, 0.5, 0.0], [-1.0, 0.0, 0.0])).taps[0].uv, [0.25, 0.25]));
    let half = Cylindrical { coverage_angle: 180.0, hide_caps: true, ..Cylindrical::default() };
    let c = prepared(ProjectionMode::Cylindrical(half));
    assert!(close(c.project(&point([1.0, 0.0, 0.0], [1.0, 0.0, 0.0])).taps[0].uv, [1.0, 0.5]));
    assert_eq!(c.project(&point([0.0, 0.0, -1.0], [0.0, 0.0, -1.0])).coverage, 0.0);
    // a cap (normal along the axis) is hidden
    assert_eq!(c.project(&point([0.0, 1.0, 0.5], [0.0, 1.0, 0.0])).coverage, 0.0);
}

#[test]
fn mirrored_fills_show_the_projection_at_the_copy() {
    let decal = Planar { volume: Volume { offset: [0.25, 0.0, 0.0], scale: [0.25, 0.25, 1.0], clip: true, ..Volume::default() }, ..Planar::default() };
    let projection = Projection { mode: ProjectionMode::Planar(decal), symmetry: SymmetrySpec::Mirror { axis: 0, offset: 0.0 }, ..Projection::default() };
    let p = projection.prepare_mirrored(Some(&FRAME), true).unwrap();
    let right = p.project(&point([0.5, 0.0, 0.0], [0.0, 0.0, 1.0]));
    let left = p.project(&point([-0.5, 0.0, 0.0], [0.0, 0.0, 1.0]));
    assert_eq!((right.coverage, left.coverage), (1.0, 1.0));
    assert!(close(right.taps[0].uv, left.taps[0].uv));
    assert_eq!(p.project(&point([0.0, 0.5, 0.0], [0.0, 0.0, 1.0])).coverage, 0.0);
    // texture space and UV take no symmetry
    let uv = Projection { symmetry: SymmetrySpec::Mirror { axis: 0, offset: 0.0 }, ..Projection::uv(ImagePlacement::default()) };
    assert!(!uv.prepare_mirrored(None, false).unwrap().is_active());
}

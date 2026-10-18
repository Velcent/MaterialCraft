use super::sampling::{bilinear, nearest};
use super::*;
use crate::tex::Tex;

fn ramp(w: usize, h: usize) -> Tex {
    let px = (0..w * h).map(|i| [(i % w) as f32, (i / w) as f32, 0.0, 1.0]).collect();
    Tex { w, h, px }
}

fn close(a: [f32; 2], b: [f32; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-5 && (a[1] - b[1]).abs() < 1e-5
}

#[test]
fn texture_space_maps_uv_to_itself() {
    let p = Projection::default().prepare(None, false).unwrap();
    let point = SurfacePoint { uv: [0.25, 0.75], texel_len: 1.0 / 64.0, ..SurfacePoint::default() };
    let taps = p.project(&point);
    assert_eq!(taps.used().len(), 1);
    assert_eq!(taps.coverage, 1.0);
    assert!(close(taps.taps[0].uv, [0.25, 0.75]));
    assert!(p.uv_only());
}

#[test]
fn uv_placement_tiles_rotates_and_offsets_about_the_centre() {
    let tiled = UvPlacement::new(&ImagePlacement { tiling: [2.0, 3.0], ..ImagePlacement::default() });
    assert!(close(tiled.apply([0.5, 0.5]), [0.5, 0.5]));
    assert!(close(tiled.apply([1.0, 1.0]), [1.5, 2.0]));
    assert_eq!(tiled.scale(), 3.0);
    let turned = UvPlacement::new(&ImagePlacement { rotation: 90.0, ..ImagePlacement::default() });
    // the image turns counter-clockwise: right of the UV centre shows what was above the image centre
    assert!(close(turned.apply([1.0, 0.5]), [0.5, 0.0]));
    let moved = UvPlacement::new(&ImagePlacement { offset: [0.25, 0.0], ..ImagePlacement::default() });
    assert!(close(moved.apply([0.75, 0.5]), [0.5, 0.5]));
    let mirrored = UvPlacement::new(&ImagePlacement { tiling: [-1.0, 1.0], ..ImagePlacement::default() });
    assert!(close(mirrored.apply([0.0, 0.5]), [1.0, 0.5]));
    assert!(close(UvPlacement::identity().apply([0.1, 0.9]), [0.1, 0.9]));
}

#[test]
fn volume_modes_report_instead_of_guessing() {
    let planar = Projection { mode: ProjectionMode::default_of("planar").unwrap(), ..Projection::default() };
    assert!(matches!(planar.prepare(None, false), Err(NotComputable::NeedsMesh { .. })));
    let frame = SceneFrame { centre: [0.0; 3], size: [2.0, 1.0, 0.5] };
    assert_eq!(frame.largest_extent(), 2.0);
    assert!(matches!(planar.prepare(Some(&frame), false), Err(NotComputable::NeedsMesh { .. })));
    assert!(matches!(planar.prepare(Some(&frame), true), Ok(Prepared::Planar(_))));
    let warp = Projection { mode: ProjectionMode::default_of("warp").unwrap(), ..Projection::default() };
    assert!(matches!(warp.prepare(Some(&frame), true), Ok(Prepared::Warp(_))));
    assert!(matches!(warp.prepare(Some(&frame), false), Err(NotComputable::NeedsMesh { .. })));
    let unknown = Projection { mode: ProjectionMode::Unknown(UnknownMode { mode: "hologram".into(), json: "{}".into() }), ..Projection::default() };
    assert_eq!(unknown.prepare(Some(&frame), true), Err(NotComputable::Unknown { mode: "hologram".into() }));
    for id in ["texture_space", "uv", "tri_planar", "planar", "spherical", "cylindrical", "warp"] {
        let m = ProjectionMode::default_of(id).unwrap();
        assert_eq!(m.id(), id);
        assert_eq!(m.needs_mesh(), m.volume().is_some());
    }
    assert!(ProjectionMode::default_of("hologram").is_none());
}

#[test]
fn enum_ids_round_trip() {
    for f in Filter::ALL {
        assert_eq!(Filter::from_id(f.id()), Some(f));
    }
    for r in Repeat::ALL {
        assert_eq!(Repeat::from_id(r.id()), Some(r));
    }
    for m in SizeMode::ALL {
        assert_eq!(SizeMode::from_id(m.id()), Some(m));
    }
    assert!(Projection::default().is_default());
    assert!(!Projection::uv(ImagePlacement::default()).is_default());
}

#[test]
fn sampling_wraps_clamps_and_clips_per_axis() {
    let t = ramp(4, 2);
    assert_eq!(nearest(&t, [0.0, 0.0], Repeat::Both), Some([0.0, 0.0, 0.0, 1.0]));
    assert_eq!(nearest(&t, [1.1, 0.0], Repeat::Both), Some([0.0, 0.0, 0.0, 1.0]));
    assert_eq!(nearest(&t, [1.1, 0.0], Repeat::V), None);
    assert_eq!(nearest(&t, [0.5, 1.5], Repeat::U), None);
    // texel centres read exactly
    assert_eq!(bilinear(&t, [2.5 / 4.0, 0.25], Repeat::Both), Some([2.0, 0.0, 0.0, 1.0]));
    // between texel 3 and texel 0: wraps when repeating, clamps when not
    assert_eq!(bilinear(&t, [1.0, 0.25], Repeat::Both).unwrap()[0], 1.5);
    assert_eq!(bilinear(&t, [1.0, 0.25], Repeat::None).unwrap()[0], 3.0);
    assert_eq!(bilinear(&t, [f32::NAN, 0.25], Repeat::Both), None);
    let empty = Tex { w: 0, h: 0, px: Vec::new() };
    assert_eq!(bilinear(&empty, [0.5, 0.5], Repeat::Both), None);
}

#[test]
fn levels_halve_down_to_one_texel_and_smooth_picks_by_footprint() {
    let t = ramp(5, 2);
    let chain = LevelChain::build(&t);
    let levels = ImageLevels::new(&t, &chain);
    assert_eq!(levels.count(), 4); // 5x2, 3x1, 2x1, 1x1
    assert_eq!(levels.level(1).size(), (3, 1));
    assert_eq!(levels.level(3).size(), (1, 1));
    // level 1 texel 0 averages x 0,1 and rows 0,1
    assert_eq!(levels.level(1).texel(0, 0), [0.5, 0.5, 0.0, 1.0]);
    let centre = [0.5, 0.5];
    let fine = levels.sample(centre, 1.0 / 5.0, Filter::Smooth, Repeat::Both).unwrap();
    assert_eq!(Some(fine), bilinear(&t, centre, Repeat::Both));
    let coarse = levels.sample(centre, 100.0, Filter::Smooth, Repeat::Both).unwrap();
    assert_eq!(coarse, levels.level(3).texel(0, 0));
    assert_eq!(levels.sample([0.1, 0.1], 100.0, Filter::Nearest, Repeat::Both), Some([0.0, 0.0, 0.0, 1.0]));
}

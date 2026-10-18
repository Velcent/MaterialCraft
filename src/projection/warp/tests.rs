//! The gate of step 43: the inverse lookup lands within 0.1 texel (of a 4096-texel image) of a brute-force search
//! on deformed grids; a regular grid projects as the planar projection does; depth and falloff.

use super::super::modes::{Falloff, Planar, ProjectionMode, Volume, Warp, WarpGrid, WarpPoint};
use super::super::{Projection, Projector, SceneFrame, SurfacePoint};
use super::grid::{U_NEXT, U_PREV, V_NEXT, V_PREV};
use super::*;
use std::sync::Arc;

const IMAGE: f64 = 4096.0;

/// A small deterministic generator in −1…1.
struct Noise(u64);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
    }
}

/// A regular grid with every anchor moved (x-y by up to `shift` of a cell, z by up to 0.3) and explicit handles
/// bent by up to `bend` of a cell, and uneven image coordinates.
fn deformed(rows: u32, columns: u32, seed: u64, shift: f32, bend: f32) -> WarpGrid {
    let mut n = Noise(seed);
    let (du, dv) = (2.0 / (columns - 1) as f32, 2.0 / (rows - 1) as f32);
    let mut points = WarpGrid::regular_points(rows, columns);
    for p in &mut points {
        p.pos[0] += n.next() * shift * du;
        p.pos[1] += n.next() * shift * dv;
        p.pos[2] += n.next() * 0.3;
        let hu = [du / 3.0 + n.next() * bend * du, n.next() * bend * dv, n.next() * 0.1];
        let hv = [n.next() * bend * du, -dv / 3.0 + n.next() * bend * dv, n.next() * 0.1];
        p.tangents[U_NEXT] = hu;
        p.tangents[U_PREV] = hu.map(|v| -v);
        p.tangents[V_NEXT] = hv;
        p.tangents[V_PREV] = hv.map(|v| -v);
        p.uv = [(p.uv[0] + 0.03 * n.next()).clamp(0.0, 1.0), (p.uv[1] + 0.03 * n.next()).clamp(0.0, 1.0)];
    }
    WarpGrid { rows, columns, depth: 0.5, points }
}

fn brute_force(s: &WarpSurface, xy: [f64; 2]) -> Option<[f64; 2]> {
    let (u, v) = s.mesh().param_at_xy(xy, 32);
    let p = s.mesh().eval(u, v);
    let miss = (p[0] - xy[0]).hypot(p[1] - xy[1]);
    (miss < 1e-7).then(|| s.image_at(u, v).0)
}

#[test]
fn the_inverse_lookup_matches_a_brute_force_search_within_a_tenth_of_a_texel() {
    let grids = [deformed(3, 3, 1, 0.2, 0.08), deformed(5, 3, 7, 0.15, 0.05), deformed(4, 6, 11, 0.2, 0.06), WarpGrid::default()];
    let mut worst = 0.0f64;
    let (mut checked, mut misses) = (0, 0);
    for grid in &grids {
        let surface = WarpSurface::new(grid, prepared::LOOKUP_RESOLUTION).unwrap();
        let n = 25;
        for j in 0..n {
            for i in 0..n {
                // a little beyond −1…1, so the border and the outside are queried too
                let xy = [-1.3 + 2.6 * (i as f64 + 0.37) / n as f64, -1.3 + 2.6 * (j as f64 + 0.61) / n as f64];
                let fast = surface.locate(xy);
                match brute_force(&surface, xy) {
                    Some(uv) => {
                        let hit = fast.unwrap_or_else(|| panic!("the lookup missed {xy:?}"));
                        let d = (hit.uv[0] - uv[0]).hypot(hit.uv[1] - uv[1]) * IMAGE;
                        worst = worst.max(d);
                        checked += 1;
                    }
                    None => {
                        assert!(fast.is_none(), "the lookup found {xy:?} off the surface");
                        misses += 1;
                    }
                }
            }
        }
    }
    eprintln!("worst {worst:.2e} texels over {checked} points");
    assert!(worst <= 0.1, "worst {worst} texels");
    assert!(checked > 1000 && misses > 400, "{checked} on, {misses} off");
}

#[test]
fn a_regular_grid_projects_like_planar() {
    let frame = SceneFrame { centre: [0.0; 3], size: [2.0, 2.0, 2.0] };
    let volume = Volume { offset: [0.1, -0.05, 0.0], rotation: [10.0, 20.0, 30.0], scale: [0.8, 0.6, 1.0], clip: false };
    let warp = Projection { mode: ProjectionMode::Warp(Warp { volume, ..Warp::default() }), ..Projection::default() }.prepare(Some(&frame), true).unwrap();
    let planar =
        Projection { mode: ProjectionMode::Planar(Planar { volume, ..Planar::default() }), ..Projection::default() }.prepare(Some(&frame), true).unwrap();
    let placed = super::super::gizmo::PlacedVolume::of(&volume, &frame);
    let mut n = Noise(3);
    for _ in 0..200 {
        // points within the projection depth (0.1 grid units = 0.05 local) of the grid's plane
        let l = [n.next() * 0.49, n.next() * 0.49, n.next() * 0.04];
        let p = SurfacePoint { pos: placed.point(l), normal: [0.0, 0.0, 1.0], texel_len: 1.0 / 1024.0, ..SurfacePoint::default() };
        let (a, b) = (warp.project(&p), planar.project(&p));
        assert_eq!((a.n, a.coverage), (1, 1.0), "{l:?}");
        let (ta, tb) = (a.taps[0], b.taps[0]);
        assert!((ta.uv[0] - tb.uv[0]).abs() < 1e-4 && (ta.uv[1] - tb.uv[1]).abs() < 1e-4, "{l:?}: {:?} vs {:?}", ta.uv, tb.uv);
        assert!((ta.footprint - tb.footprint).abs() < 1e-5 * tb.footprint.max(1.0) + 1e-6, "{} vs {}", ta.footprint, tb.footprint);
    }
}

#[test]
fn depth_cuts_and_fades_along_the_grid_z() {
    let frame = SceneFrame { centre: [0.0; 3], size: [2.0, 2.0, 2.0] };
    let warp = |falloff: Falloff| {
        let grid = WarpGrid { depth: 0.2, ..WarpGrid::default() };
        Projection { mode: ProjectionMode::Warp(Warp { depth_falloff: falloff, grid, ..Warp::default() }), ..Projection::default() }
            .prepare(Some(&frame), true)
            .unwrap()
    };
    // local z = 0.5 × grid z; the scene is 2 wide, so object = 2 × local
    let at = |grid_z: f32| SurfacePoint { pos: [0.2, 0.3, grid_z], texel_len: 1e-3, ..SurfacePoint::default() };
    let hard = warp(Falloff::default());
    assert_eq!(hard.project(&at(0.19)).coverage, 1.0);
    assert_eq!(hard.project(&at(-0.19)).coverage, 1.0);
    assert_eq!(hard.project(&at(0.21)).coverage, 0.0);
    let soft = warp(Falloff { on: true, hardness: 0.5 });
    assert_eq!(soft.project(&at(0.05)).coverage, 1.0);
    assert!((soft.project(&at(0.15)).coverage - 0.5).abs() < 1e-3);
    // beside the grid: nothing
    assert_eq!(hard.project(&SurfacePoint { pos: [1.5, 0.0, 0.0], ..SurfacePoint::default() }).n, 0);
}

#[test]
fn a_curved_grid_follows_its_surface() {
    // lift the middle anchor of a 3 × 3 grid: points on the lifted surface are covered, points on the flat plane
    // under it fall outside a thin projection depth
    let mut grid = WarpGrid { depth: 0.05, ..WarpGrid::default() }.with_point_moved(4, [0.0, 0.0, 0.5]).unwrap();
    grid.depth = 0.05;
    let surface = surface_for(&grid).unwrap();
    let hit = surface.locate([0.0, 0.0]).unwrap();
    assert!((hit.pos[2] - 0.5).abs() < 1e-9 && (hit.uv[0] - 0.5).abs() < 1e-9 && (hit.uv[1] - 0.5).abs() < 1e-9);
    let frame = SceneFrame { centre: [0.0; 3], size: [2.0, 2.0, 2.0] };
    let p = Projection { mode: ProjectionMode::Warp(Warp { grid, ..Warp::default() }), ..Projection::default() }.prepare(Some(&frame), true).unwrap();
    let on = SurfacePoint { pos: [0.0, 0.0, 0.5], ..SurfacePoint::default() };
    let below = SurfacePoint { pos: [0.0, 0.0, 0.0], ..SurfacePoint::default() };
    assert_eq!(p.project(&on).coverage, 1.0);
    assert_eq!(p.project(&below).coverage, 0.0);
}

#[test]
fn surfaces_are_shared_between_passes_and_invalid_grids_refused() {
    let grid = deformed(3, 4, 5, 0.1, 0.05);
    let a = surface_for(&grid).unwrap();
    let b = surface_for(&WarpGrid { depth: 3.0, ..grid.clone() }).unwrap();
    assert!(Arc::ptr_eq(&a, &b), "the depth does not change the surface");
    let broken = WarpGrid { points: vec![WarpPoint::default(); 2], ..grid };
    let frame = SceneFrame { centre: [0.0; 3], size: [1.0; 3] };
    let err = Projection { mode: ProjectionMode::Warp(Warp { grid: broken, ..Warp::default() }), ..Projection::default() }.prepare(Some(&frame), true);
    assert!(matches!(err, Err(super::super::NotComputable::Invalid { .. })), "{err:?}");
}

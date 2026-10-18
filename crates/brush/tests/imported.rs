//! Imported strokes replayed without the editor (roadmap step 32): a sampled tip replaces the round one, the
//! lazy mouse trails the recorded path, pressure minimums keep thin ends visible, and 2D-view strokes paint in
//! UV space; every replay gives the same bits again.

use materialcraft::mesh::{Mesh, MeshPart, SurfaceKey, SurfaceMap};
use materialcraft::paint::brush::{BrushPreset, PressureUse, SizeSpace};
use materialcraft::paint::stroke::{Camera, Stroke, StrokeSample, StrokeTool, Symmetry, ViewKind};
use materialcraft::paint::{PixelFormat, PixelLayout, SampleType, TiledImage};
use materialcraft::projection::ViewScene;
use materialcraft::stamp::{ChannelTarget, StampMode, TexelIndex};
use materialcraft_brush::{EngineExtras, PaintValues, ReplayBrush, ReplayScene, TipAlpha, replay_imported};

const SIZE: u32 = 128;

fn quad() -> Mesh {
    Mesh {
        materials: vec!["m".into()],
        parts: vec![MeshPart {
            name: "quad".into(),
            material: 0,
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 4],
            tangents: vec![],
            uv_sets: vec![vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
            faces: Default::default(),
        }],
    }
}

fn uv_stroke(points: &[[f32; 2]], pressure: f32) -> Stroke {
    Stroke {
        tool: StrokeTool::Paint,
        preset: "substance:DataBrush/1".into(),
        view: ViewKind::Uv,
        camera: Camera::default(),
        seed: 3,
        symmetry: Symmetry::default(),
        samples: points.iter().enumerate().map(|(i, p)| StrokeSample { pressure, ..StrokeSample::at(*p, i as f32 * 0.01) }).collect(),
    }
}

fn brush(tip: Option<TipAlpha>, lazy: f32, size_minimum: f32) -> ReplayBrush {
    ReplayBrush {
        preset: BrushPreset {
            size: 0.1,
            size_space: SizeSpace::Model,
            spacing: 0.1,
            pressure: PressureUse { size: true, flow: false },
            ..BrushPreset::default()
        },
        extras: EngineExtras { tip, size_minimum, flow_minimum: 0.0 },
        lazy,
        viewport_height: 512.0,
        occlusion: false,
        model: None,
    }
}

/// Paints `stroke` with `brush` into a fresh grey-alpha image; the coverage per texel.
fn paint(stroke: &Stroke, brush: &ReplayBrush) -> Vec<f32> {
    let mesh = quad();
    let map = SurfaceMap::build(&mesh, SurfaceKey::square(&mesh, 0, SIZE)).unwrap();
    let index = TexelIndex::build(&map);
    let view = ViewScene::new(&mesh, materialcraft::mesh::Fit::IDENTITY);
    let scene = ReplayScene { mesh: &mesh, view: &view, surface: &map, index: &index, factors: &[] };
    let mut image = TiledImage::new(SIZE, SIZE, PixelFormat::new(PixelLayout::GrayAlpha, SampleType::U16)).unwrap();
    let values = PaintValues { channels: vec![], mask: 1.0 };
    let mut targets = [ChannelTarget { image: &mut image, value: [1.0; 4], mode: StampMode::Paint, srgb: false, source: None }];
    replay_imported(stroke, brush, &values, &scene, &mut targets).unwrap();
    (0..SIZE * SIZE).map(|i| image.get(i % SIZE, i / SIZE)[3]).collect()
}

fn at(c: &[f32], x: u32, y: u32) -> f32 {
    c[(y * SIZE + x) as usize]
}

#[test]
fn a_sampled_tip_replaces_the_round_one() {
    // a tip lit only in its left half
    let half = TipAlpha::new(16, 16, (0..256).map(|i| if i % 16 < 8 { 1.0 } else { 0.0 }).collect()).unwrap();
    let stroke = uv_stroke(&[[0.5, 0.5]], 1.0);
    let round = paint(&stroke, &brush(None, 0.0, 0.0));
    let sampled = paint(&stroke, &brush(Some(half), 0.0, 0.0));
    // 0.1 of the texture is 12.8 texels across: 4 texels left and right of the centre
    assert!(at(&round, 60, 64) > 0.5 && at(&round, 68, 64) > 0.5);
    assert!(at(&sampled, 60, 64) > 0.5 && at(&sampled, 68, 64) < 0.05, "{} {}", at(&sampled, 60, 64), at(&sampled, 68, 64));
    assert_eq!(sampled, paint(&stroke, &brush(TipAlpha::new(16, 16, (0..256).map(|i| if i % 16 < 8 { 1.0 } else { 0.0 }).collect()), 0.0, 0.0)));
}

#[test]
fn the_lazy_mouse_trails_the_recorded_path() {
    let line: Vec<[f32; 2]> = (0..=40).map(|i| [0.2 + 0.6 * i as f32 / 40.0, 0.5]).collect();
    let direct = paint(&uv_stroke(&line, 1.0), &brush(None, 0.0, 0.0));
    // a string a fifth of the texture long: the brush stops that far short of the path's end
    let lazy = paint(&uv_stroke(&line, 1.0), &brush(None, 0.2, 0.0));
    assert!(at(&direct, 100, 64) > 0.5);
    assert!(at(&lazy, 100, 64) < 0.05 && at(&lazy, 70, 64) > 0.5);
}

#[test]
fn a_pressure_minimum_keeps_light_strokes_visible() {
    let line: Vec<[f32; 2]> = (0..=20).map(|i| [0.3 + 0.4 * i as f32 / 20.0, 0.5]).collect();
    let none = paint(&uv_stroke(&line, 0.0), &brush(None, 0.0, 0.0));
    let floor = paint(&uv_stroke(&line, 0.0), &brush(None, 0.0, 0.5));
    let total = |c: &[f32]| c.iter().sum::<f32>();
    assert!(total(&floor) > total(&none) * 2.0 + 1.0, "{} vs {}", total(&floor), total(&none));
}

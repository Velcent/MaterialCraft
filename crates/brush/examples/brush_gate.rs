//! The brush pipeline's gate, headless (roadmap step 20): on a document's first texture set, at 4096²,
//!
//! 1. timing: a 300-unit brush stroke across the model at one pointer sample per 60 Hz frame; every frame's stamp
//!    work must stay within 16.7 ms;
//! 2. replay: the stroke's record replayed twice gives the same bits;
//! 3. seams: a soft dab centred on UV seam edges facing the camera paints both sides of each seam alike.
//!
//! `cargo run --release -p materialcraft-brush --example brush_gate -- <document.mcpaint> [size]`

use materialcraft::camera::{Camera, Lens};
use materialcraft::mesh::{Mesh, SurfaceKey, SurfaceMap, Topology, UvEdgeKind, Vec3};
use materialcraft::paint::brush::{BrushPreset, SizeSpace};
use materialcraft::paint::stroke::{StrokeTool, ViewKind};
use materialcraft::paint::{PixelFormat, TiledImage};
use materialcraft::projection::{ViewProjector, ViewScene};
use materialcraft::stamp::{ChannelTarget, StampMode, SymmetrySpec, TexelIndex};
use materialcraft_brush::{InputPoint, PaintValues, ReplayScene, SessionScene, StrokeSession, StrokeSetup, replay};
use std::time::Instant;

const VIEWPORT: [f32; 2] = [1920.0, 1080.0];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(path) = args.first() else {
        eprintln!("usage: brush_gate <document.mcpaint> [size]");
        std::process::exit(2);
    };
    let size: u32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(4096);
    let doc = materialcraft::paint::format::load_path(std::path::Path::new(path)).unwrap_or_else(|e| fail(&format!("can't load {path}: {e}")));
    let set = doc.texture_sets.first().unwrap_or_else(|| fail("the document has no texture set"));
    let mesh = &doc.mesh;
    let key = SurfaceKey { material: set.material_slot, uv_set: set.uv_set, width: size, height: size, gutter: 8, mesh_revision: 0 };
    let t = Instant::now();
    let map = SurfaceMap::build(mesh, key).unwrap_or_else(|e| fail(&format!("surface map: {e}")));
    let t_map = t.elapsed();
    let t = Instant::now();
    let index = TexelIndex::build(&map);
    let t_index = t.elapsed();
    let t = Instant::now();
    let view = ViewScene::new(mesh, mesh.fit());
    let t_bvh = t.elapsed();
    let triangles: usize = mesh.parts.iter().map(|p| p.triangles.len()).sum();
    println!("set {:?}: {size}² surface map {:.0} ms, texel index {:.0} ms, BVH {:.0} ms ({triangles} triangles)", set.name, ms(t_map), ms(t_index), ms(t_bvh));

    let timing_ok = timing(mesh, &map, &index, &view);
    let seams_ok = seams(mesh, &map, &index, &view, set.material_slot, set.uv_set);
    if !(timing_ok && seams_ok) {
        std::process::exit(1);
    }
}

fn ms(d: std::time::Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn fail(msg: &str) -> ! {
    eprintln!("{msg}");
    std::process::exit(2);
}

fn image(size: u32) -> TiledImage {
    TiledImage::new(size, size, PixelFormat::from_id("rgba8").unwrap_or_else(|| fail("pixel format"))).unwrap_or_else(|e| fail(&e.to_string()))
}

fn target(img: &mut TiledImage) -> ChannelTarget<'_> {
    ChannelTarget { image: img, value: [0.8, 0.3, 0.1, 1.0], mode: StampMode::Paint, srgb: true }
}

/// A camera at `distance` (fitted units) from `target` (world) along `dir`.
fn camera(target: Vec3, dir: Vec3, distance: f32) -> Camera {
    let lens = Lens::Perspective { fov_y: 40f32.to_radians(), near: 0.03, far: 60.0 };
    let l = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt().max(1e-9);
    let eye = [target[0] + dir[0] / l * distance, target[1] + dir[1] / l * distance, target[2] + dir[2] / l * distance];
    let up = if dir[1].abs() / l > 0.95 { [0.0, 0.0, 1.0] } else { [0.0, 1.0, 0.0] };
    Camera::looking_at(eye, target, up, &lens, VIEWPORT)
}

fn timing(mesh: &Mesh, map: &SurfaceMap, index: &TexelIndex, view: &ViewScene) -> bool {
    let cam = camera([0.0; 3], [0.3, 0.2, 1.0], 2.6);
    let Some(projector) = view.projector(mesh, cam) else { fail("the picking scene does not match the mesh") };
    let preset = BrushPreset { size: 300.0, size_space: SizeSpace::Screen, hardness: 0.6, ..BrushPreset::default() };
    let setup = StrokeSetup {
        preset,
        tool: StrokeTool::Paint,
        seed: 5,
        symmetry: SymmetrySpec::None,
        view: ViewKind::Mesh,
        values: PaintValues { channels: Vec::new(), mask: 1.0 },
        model: mesh.bounding_sphere().unwrap_or_else(|| fail("empty mesh")),
        occlusion: true,
    };
    let scene = SessionScene { surface: map, index, projector: Some(&projector as &dyn ViewProjector), factors: &[] };
    let mut img = image(map.width() as u32);
    let t = Instant::now();
    let mut session = StrokeSession::begin(setup, &scene, mesh, &|_| true, &[target(&mut img)]).unwrap_or_else(|e| fail(&e.to_string()));
    let t_begin = t.elapsed();
    // across the model at 20 units per frame (1200 units/s at 60 Hz), with a wave
    let frames = 90;
    let mut times = Vec::with_capacity(frames + 1);
    for f in 0..frames {
        let x = 960.0 - 900.0 + 20.0 * f as f32;
        let y = 540.0 + 150.0 * (f as f32 * 0.07).sin();
        let p = InputPoint { pressure: 1.0, ..InputPoint::at([x, y], f as f64 / 60.0) };
        let t = Instant::now();
        session.add(&scene, &[p], &mut [target(&mut img)]).unwrap_or_else(|e| fail(&e.to_string()));
        times.push(ms(t.elapsed()));
    }
    let t = Instant::now();
    let finished = session.finish(&scene, &mut [target(&mut img)]).unwrap_or_else(|e| fail(&e.to_string()));
    times.push(ms(t.elapsed()));
    let live = img.to_raw();
    let mut sorted = times.clone();
    sorted.sort_by(f64::total_cmp);
    let worst = sorted.last().copied().unwrap_or(0.0);
    let p95 = sorted.get(sorted.len() * 95 / 100).copied().unwrap_or(0.0);
    let mean = times.iter().sum::<f64>() / times.len().max(1) as f64;
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    println!(
        "timing: 300-unit brush, {} dabs over {frames} frames, {} tiles painted; stroke start (depth map) {:.1} ms; per frame mean {mean:.2} ms, p95 {p95:.2} ms, worst {worst:.2} ms ({threads} threads)",
        finished.dabs,
        finished.touched.len(),
        ms(t_begin)
    );
    let timing_ok = worst <= 1000.0 / 60.0;
    println!("  >= 60 Hz: {}", if timing_ok { "PASS" } else { "FAIL" });

    // replay the recorded stroke twice
    let replay_scene = ReplayScene { mesh, view, surface: map, index, factors: &[] };
    let mut runs = Vec::new();
    for _ in 0..2 {
        let mut img = image(map.width() as u32);
        let t = Instant::now();
        if let Err(e) = replay(&finished.stroke, &replay_scene, &mut [target(&mut img)]) {
            fail(&format!("replay: {e}"));
        }
        runs.push((img.to_raw(), ms(t.elapsed())));
    }
    let identical = runs.first().map(|r| &r.0) == runs.get(1).map(|r| &r.0);
    let differing = runs.first().map_or(0, |r| r.0.iter().zip(&live).filter(|(a, b)| a != b).count());
    println!(
        "replay: twice {} ({:.0} ms each); against the live stroke {differing} of {} bytes differ (sample positions stored as NDC)",
        if identical { "bit-identical: PASS" } else { "DIFFERENT: FAIL" },
        runs.first().map_or(0.0, |r| r.1),
        live.len()
    );
    timing_ok && identical
}

/// One side of a seam edge: its UV end points and its triangle's UV centroid.
type Side = ([[f32; 2]; 2], [f32; 2]);

/// A seam edge seen from both sides: the 3D end points and, per side, the UV end points and the UV centroid of its
/// triangle (to step inside).
struct SeamPair {
    ends: [Vec3; 2],
    normal: Vec3,
    sides: [Side; 2],
}

fn seam_pairs(mesh: &Mesh, material: usize, uv_set: usize) -> Vec<SeamPair> {
    let mut out = Vec::new();
    for part in mesh.parts.iter().filter(|p| p.material == material) {
        let Some(uvs) = part.uv_sets.get(uv_set) else { continue };
        let topo = Topology::of_part(part, uv_set);
        let mut seen: std::collections::HashMap<[i64; 6], (Side, Vec3)> = std::collections::HashMap::new();
        for e in topo.uv_edges().all().iter().filter(|e| e.kind == UvEdgeKind::Seam) {
            let Some(tri) = part.triangles.get(e.triangle as usize) else { continue };
            let k = e.side as usize;
            let (a, b) = (tri[k] as usize, tri[(k + 1) % 3] as usize);
            let (Some(pa), Some(pb)) = (part.positions.get(a), part.positions.get(b)) else { continue };
            let q = |p: &Vec3| p.map(|v| (v * 1e4).round() as i64);
            let (qa, qb) = (q(pa), q(pb));
            let key = if qa <= qb { [qa[0], qa[1], qa[2], qb[0], qb[1], qb[2]] } else { [qb[0], qb[1], qb[2], qa[0], qa[1], qa[2]] };
            let c = tri.map(|i| uvs.get(i as usize).copied().unwrap_or([0.0; 2]));
            let centroid = [(c[0][0] + c[1][0] + c[2][0]) / 3.0, (c[0][1] + c[1][1] + c[2][1]) / 3.0];
            let p = tri.map(|i| part.positions.get(i as usize).copied().unwrap_or([0.0; 3]));
            let n = cross(sub(p[1], p[0]), sub(p[2], p[0]));
            // orient the UV ends like the 3D ends of the key
            let uv = if qa <= qb { e.uv } else { [e.uv[1], e.uv[0]] };
            let ends = if qa <= qb { [*pa, *pb] } else { [*pb, *pa] };
            if let Some(((uv0, c0), n0)) = seen.remove(&key) {
                out.push(SeamPair { ends, normal: norm(add(n0, norm(n))), sides: [(uv0, c0), (uv, centroid)] });
            } else {
                seen.insert(key, ((uv, centroid), norm(n)));
            }
        }
    }
    out
}

fn seams(mesh: &Mesh, map: &SurfaceMap, index: &TexelIndex, view: &ViewScene, material: usize, uv_set: usize) -> bool {
    let pairs = seam_pairs(mesh, material, uv_set);
    let fit = mesh.fit();
    let size = map.width() as f32;
    let model = mesh.bounding_sphere().unwrap_or_else(|| fail("empty mesh"));
    let (mut tested, mut worst, mut sum, mut n) = (0usize, 0.0f32, 0.0f32, 0usize);
    // spread the checks over the model: every k-th seam edge, long enough to sample along
    let step = (pairs.len() / 40).max(1);
    for pair in pairs.iter().step_by(step) {
        let len3 = len(sub(pair.ends[1], pair.ends[0]));
        let uv_len = pair.sides.iter().map(|(uv, _)| ((uv[1][0] - uv[0][0]).powi(2) + (uv[1][1] - uv[0][1]).powi(2)).sqrt() * size).fold(f32::MAX, f32::min);
        if uv_len < 12.0 || len3 <= 0.0 {
            continue;
        }
        let mid = lerp3(pair.ends[0], pair.ends[1], 0.5);
        let cam = camera(fit.apply(mid), pair.normal, 1.2);
        let Some(projector) = view.projector(mesh, cam) else { continue };
        let (Some(s0), Some(s1)) = (projector.to_screen(pair.ends[0]), projector.to_screen(pair.ends[1])) else { continue };
        let centre = projector.to_screen(mid).map(|s| s.pos).unwrap_or([0.0; 2]);
        // the seam must be what the camera sees at the dab's centre
        if projector.pick(centre).is_none_or(|h| len(sub(h.position, mid)) > len3 * 0.25) || projector.occluded(mid) {
            continue;
        }
        let edge_px = ((s1.pos[0] - s0.pos[0]).powi(2) + (s1.pos[1] - s0.pos[1]).powi(2)).sqrt();
        let preset = BrushPreset { size: edge_px.max(8.0), size_space: SizeSpace::Screen, hardness: 0.0, ..BrushPreset::default() };
        let setup = StrokeSetup {
            preset,
            tool: StrokeTool::Paint,
            seed: 1,
            symmetry: SymmetrySpec::None,
            view: ViewKind::Mesh,
            values: PaintValues { channels: Vec::new(), mask: 1.0 },
            model,
            occlusion: true,
        };
        let scene = SessionScene { surface: map, index, projector: Some(&projector as &dyn ViewProjector), factors: &[] };
        let mut img = TiledImage::new(map.width() as u32, map.height() as u32, PixelFormat::from_id("graya32f").unwrap_or_else(|| fail("format")))
            .unwrap_or_else(|e| fail(&e.to_string()));
        let Ok(mut session) = StrokeSession::begin(setup, &scene, mesh, &|_| true, &[white(&mut img)]) else { continue };
        if session.add(&scene, &[InputPoint::at(centre, 0.0)], &mut [white(&mut img)]).is_err() || session.finish(&scene, &mut [white(&mut img)]).is_err() {
            continue;
        }
        // what a renderer shows at points along the edge, sampled bilinearly from each side's UVs
        let mut edge_worst = 0.0f32;
        let mut covered = false;
        for f in [0.3f32, 0.4, 0.5, 0.6, 0.7] {
            let side = |(uv, _): &([[f32; 2]; 2], [f32; 2])| {
                let p = [uv[0][0] + (uv[1][0] - uv[0][0]) * f, uv[0][1] + (uv[1][1] - uv[0][1]) * f];
                bilinear(&img, p[0] * size - 0.5, p[1] * size - 0.5)
            };
            let (a, b) = (side(&pair.sides[0]), side(&pair.sides[1]));
            covered |= a.max(b) > 0.2;
            edge_worst = edge_worst.max((a - b).abs());
        }
        if !covered {
            continue;
        }
        tested += 1;
        worst = worst.max(edge_worst);
        sum += edge_worst;
        n += 1;
    }
    let mean = if n > 0 { sum / n as f32 } else { 0.0 };
    let ok = tested >= 5 && worst <= 0.1;
    println!(
        "seams: {} seam edges, {tested} checked from a camera facing them; coverage difference across the seam mean {mean:.3}, worst {worst:.3}: {}",
        pairs.len(),
        if ok { "PASS" } else { "FAIL" }
    );
    ok
}

/// Coverage (alpha) of `img` at texel-centre coordinates `(x, y)`, interpolated bilinearly as a GPU samples it.
fn bilinear(img: &TiledImage, x: f32, y: f32) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (tx, ty) = (x - x0, y - y0);
    let at = |dx: f32, dy: f32| img.get((x0 + dx).max(0.0) as u32, (y0 + dy).max(0.0) as u32)[3];
    let top = at(0.0, 0.0) * (1.0 - tx) + at(1.0, 0.0) * tx;
    let bottom = at(0.0, 1.0) * (1.0 - tx) + at(1.0, 1.0) * tx;
    top * (1.0 - ty) + bottom * ty
}

fn white(img: &mut TiledImage) -> ChannelTarget<'_> {
    ChannelTarget { image: img, value: [1.0; 4], mode: StampMode::Paint, srgb: false }
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn len(a: Vec3) -> f32 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

fn norm(a: Vec3) -> Vec3 {
    let l = len(a).max(1e-12);
    [a[0] / l, a[1] / l, a[2] / l]
}

fn lerp3(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

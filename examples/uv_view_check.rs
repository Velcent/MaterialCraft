//! 2D (UV) view check: draws texture set parts flat in UV space through `gpu3d::paint_uv` at one screen pixel per
//! texel, reads the frames back and compares every texel the parts fully cover with the uploaded composite (the
//! base colour as the unlit view shows it). Exits with 0 when no texel is off by more than one step (half-float
//! rounding), 4 otherwise.
//!
//! usage: cargo run --release --features gui --example uv_view_check -- [document.mcpaint [set]] [--window N]
//!   without a document: a synthetic part (two islands, one mirrored) with a noise texture.
//!   With one: texture set `set` (default 0) is composited from its layers (uniform and texture fills; material
//!   fills that need the editor's resolvers stay as the compositor leaves them) and drawn tile by tile.
use eframe::egui;
use eframe::egui_glow;
use materialcraft::gpu3d::{self, Antialiasing, Display, GpuMaps, MapInputs, ScenePart, Shading, Shared};
use materialcraft::render3d::{Mesh, View};
use materialcraft::tex::Tex;
use std::sync::{Arc, Mutex};

/// What is checked: the parts' meshes (UVs), the uploaded maps and the texture size.
struct Case {
    parts: Vec<(Mesh, Vec<f32>)>,
    maps_base: Vec<u8>,
    size: usize,
}

/// A grid of `n` × `n` quads over the UV rectangle `[u0, u1] × [v0, v1]`; `mirrored` runs u right to left.
fn island(n: usize, u: (f32, f32), v: (f32, f32), mirrored: bool) -> (Mesh, Vec<f32>) {
    let mut m = Mesh { pos: vec![], nrm: vec![], tan: vec![], uv: vec![], su: vec![], sv: vec![], fade: vec![], base_nrm: vec![], tris: vec![] };
    for j in 0..=n {
        for i in 0..=n {
            let (s, t) = (i as f32 / n as f32, j as f32 / n as f32);
            let uu = if mirrored { u.1 - s * (u.1 - u.0) } else { u.0 + s * (u.1 - u.0) };
            m.pos.push([s - 0.5, 0.5 - t, 0.0]);
            m.nrm.push([0.0, 0.0, 1.0]);
            m.base_nrm.push([0.0, 0.0, 1.0]);
            m.tan.push([1.0, 0.0, 0.0]);
            m.uv.push([uu, v.0 + t * (v.1 - v.0)]);
            m.su.push(1.0);
            m.sv.push(1.0);
            m.fade.push(1.0);
        }
    }
    let at = |i: usize, j: usize| (j * (n + 1) + i) as u32;
    for j in 0..n {
        for i in 0..n {
            m.tris.push([at(i, j), at(i + 1, j), at(i + 1, j + 1)]);
            m.tris.push([at(i, j), at(i + 1, j + 1), at(i, j + 1)]);
        }
    }
    let w = vec![if mirrored { -1.0 } else { 1.0 }; m.pos.len()];
    (m, w)
}

fn hash(x: u32) -> f32 {
    let mut h = x.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0xFFFF) as f32 / 65535.0
}

fn synthetic() -> Case {
    let size = 256;
    // every texel different: any shift, blur or wrong row shows
    let base = Tex::from_fn(size, size, |u, v| {
        let (x, y) = ((u * size as f32) as u32, (v * size as f32) as u32);
        [hash(x * 7919 + y), hash(x * 104_729 + y * 13 + 1), hash(x + y * 65_537 + 2), 1.0]
    });
    let maps = gpu3d::prepare(&MapInputs { base: Some(&base), normal: None, rough: None, metal: None, ao: None, emissive: None, height: None });
    let parts = vec![island(6, (0.05, 0.45), (0.1, 0.9), false), island(5, (0.55, 0.95), (0.2, 0.7), true)];
    Case { parts, maps_base: maps.base, size: maps.size }
}

fn from_document(path: &str, set: usize) -> Result<Case, String> {
    use materialcraft::paint::{FillContext, MaterialMaps, NO_FILL_CONTEXT, OutputOptions};
    let doc = materialcraft::paint::format::load_path(std::path::Path::new(path)).map_err(|e| format!("{path}: {e}"))?;
    let ts = doc.texture_sets.get(set).ok_or(format!("{path} has no texture set {set}"))?;
    let fills: &FillContext = &NO_FILL_CONTEXT;
    let maps = MaterialMaps::build_with(ts, &OutputOptions::for_document(&doc), materialcraft::generators::builtin(), fills);
    let gpu = gpu3d::prepare(&MapInputs { base: Some(&maps.base_color), normal: None, rough: None, metal: None, ao: None, emissive: None, height: None });
    let fit = doc.mesh.fit();
    let mut parts = Vec::new();
    for part in doc.mesh.parts.iter().filter(|p| p.material == ts.material_slot) {
        let vp = part.to_viewport(&fit, ts.uv_set).map_err(|e| e.to_string())?;
        parts.push((vp.mesh, vp.tangent_w));
    }
    println!("{path}: set {set} '{}', {} part(s), {}² texels", ts.name, parts.len(), gpu.size);
    Ok(Case { parts, maps_base: gpu.base, size: gpu.size })
}

/// Texels whose whole square lies inside one triangle of the parts (all window samples covered).
fn full_coverage(case: &Case) -> Vec<bool> {
    let n = case.size;
    let mut full = vec![false; n * n];
    let s = n as f32;
    for (m, _) in &case.parts {
        for t in &m.tris {
            let p = t.map(|i| m.uv[i as usize]).map(|uv| [uv[0] * s, uv[1] * s]);
            let edge = |a: [f32; 2], b: [f32; 2], q: [f32; 2]| (b[0] - a[0]) * (q[1] - a[1]) - (b[1] - a[1]) * (q[0] - a[0]);
            let area = edge(p[0], p[1], p[2]);
            if area.abs() < 1e-9 {
                continue;
            }
            let inside = |q: [f32; 2]| {
                let e = [edge(p[0], p[1], q), edge(p[1], p[2], q), edge(p[2], p[0], q)];
                e.iter().all(|v| v * area.signum() >= 1e-4 * area.abs())
            };
            let lo = |k: usize| p.iter().map(|v| v[k]).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
            let hi = |k: usize| (p.iter().map(|v| v[k]).fold(f32::MIN, f32::max).ceil().max(0.0) as usize).min(n);
            for y in lo(1)..hi(1) {
                for x in lo(0)..hi(0) {
                    let (fx, fy) = (x as f32, y as f32);
                    if [[fx, fy], [fx + 1.0, fy], [fx, fy + 1.0], [fx + 1.0, fy + 1.0]].into_iter().all(inside) {
                        full[y * n + x] = true;
                    }
                }
            }
        }
    }
    full
}

/// Results so far.
#[derive(Default)]
struct Tally {
    compared: usize,
    exact: usize,
    max_diff: u8,
    worst: Option<(usize, usize, [u8; 3], [u8; 3])>,
}

struct App {
    gpu: Shared,
    case: Case,
    full: Vec<bool>,
    /// The tile being drawn: its top-left texel, shared with the paint callback.
    tile: Arc<Mutex<[usize; 2]>>,
    tiles: Vec<[usize; 2]>,
    next: usize,
    frame: u32,
    asked_at: Option<u32>,
    tally: Tally,
}

impl App {
    fn compare(&mut self, w: usize, h: usize, rgba: &[u8], at: [usize; 2]) {
        let n = self.case.size;
        for y in 0..h {
            for x in 0..w {
                let (tx, ty) = (at[0] + x, at[1] + y);
                if tx >= n || ty >= n || !self.full[ty * n + tx] {
                    continue;
                }
                let got = [rgba[(y * w + x) * 4], rgba[(y * w + x) * 4 + 1], rgba[(y * w + x) * 4 + 2]];
                let k = (ty * n + tx) * 4;
                let want = [self.case.maps_base[k], self.case.maps_base[k + 1], self.case.maps_base[k + 2]];
                let d = (0..3).map(|c| got[c].abs_diff(want[c])).max().unwrap_or(0);
                self.tally.compared += 1;
                if d == 0 {
                    self.tally.exact += 1;
                }
                if d > self.tally.max_diff {
                    self.tally.max_diff = d;
                    self.tally.worst = Some((tx, ty, got, want));
                }
            }
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _f: &mut eframe::Frame) {
        let ctx = &ui.ctx().clone();
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| {
            let rect = ui.max_rect();
            let (g, tile, size) = (self.gpu.clone(), self.tile.clone(), self.case.size as f32);
            let display = Display { antialiasing: Antialiasing::Off, shading: Shading::Unlit, ..Display::default() };
            let cb = egui_glow::CallbackFn::new(move |info, painter| {
                // one window pixel per texel, the view's top-left corner on the tile's first texel
                let px = info.viewport_in_pixels();
                let (w, h) = (px.width_px as f32, px.height_px as f32);
                let at = tile.lock().map(|t| *t).unwrap_or([0, 0]);
                let centre = [(at[0] as f32 + w * 0.5) / size, (at[1] as f32 + h * 0.5) / size];
                let camera = gpu3d::uv_camera(centre, h / size, [w, h]);
                gpu3d::paint_uv(&g, &info, painter, &View::default(), &display, &camera, &materialcraft::diag::NullSink);
            });
            ui.painter().add(egui::PaintCallback { rect, callback: Arc::new(cb) });
        });
        self.frame += 1;
        let mut g = self.gpu.lock().unwrap();
        if let Some(e) = &g.error {
            eprintln!("GPU ERROR: {e}");
            std::process::exit(2);
        }
        if let Some((w, h, rgba)) = g.uv.shot.take() {
            drop(g);
            let at = self.tiles[self.next];
            self.compare(w, h, &rgba, at);
            self.next += 1;
            self.asked_at = None;
            if self.next == self.tiles.len() {
                let t = &self.tally;
                println!("compared {} fully covered texels: {} exact, max difference {}", t.compared, t.exact, t.max_diff);
                if let Some((x, y, got, want)) = t.worst {
                    println!("worst texel ({x}, {y}): drawn {got:?}, composite {want:?}");
                }
                let ok = t.compared > 0 && t.max_diff <= 1;
                println!("{}", if ok { "PASS: the 2D view shows the composite texel for texel" } else { "FAIL" });
                std::process::exit(if ok { 0 } else { 4 });
            }
            *self.tile.lock().unwrap() = self.tiles[self.next];
        } else if self.asked_at.is_none_or(|a| self.frame > a + 20) && self.frame > 4 {
            // a few frames after the scene was uploaded and the tile moved
            g.uv.want_shot = true;
            self.asked_at = Some(self.frame);
        }
        if self.frame > 4000 {
            eprintln!("no read-back");
            std::process::exit(3);
        }
        ctx.request_repaint();
    }
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = args.iter().position(|a| a == "--window");
    let window = flag.and_then(|i| args.get(i + 1)).and_then(|s| s.parse().ok()).unwrap_or(512.0f32);
    let plain: Vec<&String> = args.iter().enumerate().filter(|(i, _)| flag.is_none_or(|f| *i != f && *i != f + 1)).map(|(_, a)| a).collect();
    let mut case = match plain.first() {
        Some(path) => match from_document(path, plain.get(1).and_then(|s| s.parse().ok()).unwrap_or(0)) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        },
        None => synthetic(),
    };
    let full = full_coverage(&case);
    println!("{} texels fully covered by the parts", full.iter().filter(|f| **f).count());
    let gpu = gpu3d::new_shared();
    {
        let maps = GpuMaps {
            size: case.size,
            base: case.maps_base.clone(),
            normal: vec![128; case.size * case.size * 4],
            rma: vec![200; case.size * case.size * 4],
            emissive: None,
            height: None,
        };
        // the meshes go to the GPU; the coverage was taken from them above
        let parts = std::mem::take(&mut case.parts).into_iter().map(|(mesh, tangent_w)| ScenePart { mesh, tangent_w, maps: copy(&maps) }).collect();
        gpu.lock().unwrap().set_scene(parts);
    }
    // tiles of the window's size in pixels; the window is asked for in points, so the tile step follows the read-back
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([window, window]).with_resizable(false),
        multisampling: 4,
        depth_buffer: 24,
        ..Default::default()
    };
    let size = case.size;
    eframe::run_native(
        "uv_view_check",
        options,
        Box::new(move |cc| {
            let ppp = cc.egui_ctx.pixels_per_point();
            let step = ((window * ppp).round() as usize).max(1);
            let tiles: Vec<[usize; 2]> = (0..size.div_ceil(step)).flat_map(|j| (0..size.div_ceil(step)).map(move |i| [i * step, j * step])).collect();
            println!("{} tile(s) of {step} pixels", tiles.len());
            Ok(Box::new(App { gpu, case, full, tile: Arc::new(Mutex::new(tiles[0])), tiles, next: 0, frame: 0, asked_at: None, tally: Tally::default() }))
        }),
    )
}

/// `GpuMaps` is not `Clone`.
fn copy(m: &GpuMaps) -> GpuMaps {
    GpuMaps { size: m.size, base: m.base.clone(), normal: m.normal.clone(), rma: m.rma.clone(), emissive: m.emissive.clone(), height: m.height.clone() }
}

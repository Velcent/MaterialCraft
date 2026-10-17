//! GPU custom-scene test: several parts with their own maps (a model's texture sets) drawn instead of the built-in
//! shape, written to a screenshot. Left: red studded sphere, middle: blue checker cube, right: two quads with the same
//! dome normal map, the lower one with mirrored uvs (tangent w = -1); both must be lit the same way.
//! usage: cargo run --release --features gui --example scene_shot -- out.png [relief 0..3] [width height] [big] [wire] [swap] [clear] [nosign]
//!   big: the sphere gets ~600k triangles; swap: replace the sphere's maps after the first frames (set_part_maps);
//!   clear: clear the scene after the first frames (the built-in shape must come back); nosign: drop the mirrored
//!   quad's tangent signs (control: its domes must then look lit from the wrong side)
use eframe::egui;
use eframe::egui_glow;
use materialcraft::gpu3d::{self, GpuMaps, MapInputs, ScenePart, Shared};
use materialcraft::render3d::{Mesh, View, build_mesh};
use materialcraft::tex::Tex;
use std::sync::Arc;

/// Normal map (OpenGL convention, as the editor's graphs produce it) of a height function on the unit square.
fn normal_of(n: usize, k: f32, height: impl Fn(f32, f32) -> f32 + Sync) -> Tex {
    let e = 1.0 / n as f32;
    Tex::from_fn(n, n, |u, v| {
        let dx = (height(u + e, v) - height(u - e, v)) / (2.0 * e);
        let dy = (height(u, v + e) - height(u, v - e)) / (2.0 * e);
        let (nx, ny) = (-dx * k, dy * k);
        let l = (nx * nx + ny * ny + 1.0).sqrt();
        [nx / l * 0.5 + 0.5, ny / l * 0.5 + 0.5, 1.0 / l * 0.5 + 0.5, 1.0]
    })
}

/// Round studs on a grid, 0..1.
fn studs(u: f32, v: f32, cells: f32) -> f32 {
    let (x, y) = ((u * cells).rem_euclid(1.0) - 0.5, (v * cells).rem_euclid(1.0) - 0.5);
    (1.0 - (x * x + y * y).sqrt() / 0.38).clamp(0.0, 1.0).sqrt()
}

fn maps(base: [f32; 4], rough: f32, metal: f32, height: Option<&(dyn Fn(f32, f32) -> f32 + Sync)>, nk: f32) -> GpuMaps {
    let n = 512;
    let b = Tex::solid(4, 4, base);
    let (r, m) = (Tex::solid(4, 4, [rough, rough, rough, 1.0]), Tex::solid(4, 4, [metal, metal, metal, 1.0]));
    let nm = normal_of(n, nk, |u, v| height.map_or(0.0, |f| f(u, v)));
    let h = height.map(|f| Tex::gray_fn(n, n, f));
    gpu3d::prepare(&MapInputs { base: Some(&b), normal: Some(&nm), rough: Some(&r), metal: Some(&m), ao: None, emissive: None, height: h.as_ref() })
}

fn checker_maps() -> GpuMaps {
    let n = 512;
    let cell = |u: f32, v: f32| ((u * 4.0).floor() as i32 + (v * 4.0).floor() as i32).rem_euclid(2) as f32;
    let base = Tex::from_fn(n, n, |u, v| if cell(u, v) > 0.5 { [0.1, 0.25, 0.85, 1.0] } else { [0.85, 0.88, 0.95, 1.0] });
    let rough = Tex::gray_fn(n, n, |u, v| 0.25 + 0.4 * cell(u, v));
    let h = Tex::gray_fn(n, n, |u, v| 0.3 + 0.7 * cell(u, v));
    gpu3d::prepare(&MapInputs { base: Some(&base), normal: None, rough: Some(&rough), metal: None, ao: None, emissive: None, height: Some(&h) })
}

/// Scale and move a mesh (uv scales follow the object-space size).
fn place(mut m: Mesh, scale: f32, off: [f32; 3]) -> Mesh {
    for p in &mut m.pos {
        *p = [p[0] * scale + off[0], p[1] * scale + off[1], p[2] * scale + off[2]];
    }
    m.su.iter_mut().chain(m.sv.iter_mut()).for_each(|s| *s *= scale);
    m
}

/// A camera-facing quad (normal +z) of side `s` centred at `c`. Mirrored: u runs right to left, so the tangent (+u)
/// points to -x and the bitangent sign is -1 (bitangent = cross(N, T) * w points along -v, i.e. up).
fn quad(s: f32, c: [f32; 3], mirrored: bool) -> (Mesh, Vec<f32>) {
    let corners = [(-0.5f32, -0.5f32), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)];
    let mut m =
        Mesh { pos: vec![], nrm: vec![], tan: vec![], uv: vec![], su: vec![], sv: vec![], fade: vec![], base_nrm: vec![], tris: vec![[0, 1, 2], [0, 2, 3]] };
    for (x, y) in corners {
        m.pos.push([c[0] + x * s, c[1] + y * s, c[2]]);
        m.nrm.push([0.0, 0.0, 1.0]);
        m.base_nrm.push([0.0, 0.0, 1.0]);
        m.tan.push(if mirrored { [-1.0, 0.0, 0.0] } else { [1.0, 0.0, 0.0] });
        m.uv.push([if mirrored { 0.5 - x } else { x + 0.5 }, 0.5 - y]);
        m.su.push(s);
        m.sv.push(s);
        m.fade.push(1.0);
    }
    (m, vec![if mirrored { -1.0 } else { 1.0 }; 4])
}

fn scene(big: bool, nosign: bool) -> Vec<ScenePart> {
    let stud = |u: f32, v: f32| studs(u, v, 8.0);
    // a dome per cell: with the wrong handedness the mirrored quad's domes would be lit from the other side
    let dome = |u: f32, v: f32| studs(u, v, 2.0) * 0.5;
    let sphere = place(build_mesh(0, if big { 768 } else { 128 }, 1.0, None, 0.0, 0.0), 0.62, [-1.45, 0.0, 0.0]);
    let cube = place(build_mesh(1, 128, 1.0, None, 0.0, 0.0), 0.5, [0.0, 0.0, 0.0]);
    let (q0, w0) = quad(0.8, [1.35, 0.45, 0.0], false);
    let (q1, w1) = quad(0.8, [1.35, -0.45, 0.0], true);
    let tris: usize = [&sphere, &cube, &q0, &q1].iter().map(|m| m.tris.len()).sum();
    println!("scene: 4 parts, {tris} triangles");
    // the same direction-revealing normal map on both quads
    let dome_maps = || {
        let (nm, b) = (normal_of(512, 0.02, dome), Tex::solid(4, 4, [0.7, 0.7, 0.7, 1.0]));
        gpu3d::prepare(&MapInputs { base: Some(&b), normal: Some(&nm), rough: None, metal: None, ao: None, emissive: None, height: None })
    };
    vec![
        ScenePart { mesh: sphere, tangent_w: vec![], maps: maps([0.75, 0.08, 0.06, 1.0], 0.35, 0.0, Some(&stud), 0.004) },
        ScenePart { mesh: cube, tangent_w: vec![], maps: checker_maps() },
        ScenePart { mesh: q0, tangent_w: w0, maps: dome_maps() },
        ScenePart { mesh: q1, tangent_w: if nosign { vec![] } else { w1 }, maps: dome_maps() },
    ]
}

struct App {
    gpu: Shared,
    view: View,
    out: String,
    swap: bool,
    clear: bool,
    frame: u32,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _f: &mut eframe::Frame) {
        egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| {
            let rect = ui.max_rect();
            let (g, v) = (self.gpu.clone(), self.view.clone());
            ui.painter()
                .add(egui::PaintCallback { rect, callback: Arc::new(egui_glow::CallbackFn::new(move |info, painter| gpu3d::paint(&g, &info, painter, &v))) });
        });
        self.frame += 1;
        if self.frame == 3 {
            let mut g = self.gpu.lock().unwrap();
            if self.swap {
                g.set_part_maps(0, maps([0.9, 0.75, 0.3, 1.0], 0.25, 1.0, Some(&|u, v| studs(u, v, 4.0)), 0.004));
            }
            if self.clear {
                g.clear_scene();
            }
        }
        if self.frame == 6 {
            self.gpu.lock().unwrap().want_shot = true;
        }
        if self.frame >= 8 {
            let mut g = self.gpu.lock().unwrap();
            if let Some(e) = &g.error {
                eprintln!("GPU ERROR: {e}");
                std::process::exit(2);
            }
            if let Some((w, h, rgba)) = g.shot.take() {
                let px: Vec<[f32; 4]> = rgba.chunks(4).map(|c| [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, 1.0]).collect();
                std::fs::write(&self.out, materialcraft::png::encode(w, h, &px, 8)).unwrap();
                println!("renderer: {} | {w}x{h} written to {} | has_scene {}", g.renderer, self.out, g.has_scene());
                std::process::exit(0);
            }
            if self.frame > 60 {
                eprintln!("no screenshot produced");
                std::process::exit(3);
            }
        }
        ctx.request_repaint();
    }
}

fn main() -> eframe::Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let out = a.get(1).cloned().unwrap_or_else(|| "scene_shot.png".into());
    let relief = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(3u32);
    let (w, h) = (a.get(3).and_then(|s| s.parse().ok()).unwrap_or(1100.0f32), a.get(4).and_then(|s| s.parse().ok()).unwrap_or(600.0f32));
    let has = |k: &str| a.iter().any(|s| s == k);
    let gpu = gpu3d::new_shared();
    gpu.lock().unwrap().set_scene(scene(has("big"), has("nosign")));
    let view = View { relief, wireframe: has("wire"), yaw: 0.15, pitch: 0.25, dist: 5.2, height_scale: 0.04, ..View::default() };
    let options =
        eframe::NativeOptions { viewport: egui::ViewportBuilder::default().with_inner_size([w, h]), multisampling: 4, depth_buffer: 24, ..Default::default() };
    let (swap, clear) = (has("swap"), has("clear"));
    eframe::run_native("scene_shot", options, Box::new(move |_cc| Ok(Box::new(App { gpu, view, out, swap, clear, frame: 0 }))))
}

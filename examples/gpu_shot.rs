// SPDX-License-Identifier: MIT OR Apache-2.0
//! Headless-friendly GPU viewer test: renders a procedural brick material with the GPU renderer and writes a screenshot.
//! usage: cargo run --release --features gui --example gpu_shot -- out.png [shape 0|1|2] [relief 0..3] [width height] [nomsaa] [wire] [rough=0.3 metal=0 color=r,g,b]
//!   [still] [env=map.hdr|map.exr] [bg=opacity] [blur=0..1] [envexp=stops] [noshadow] [world] [lightyaw=radians]
//!   still: the Paint viewport's default display (accumulation, shadows), shot once the view has refined;
//!   env: light with a prefiltered environment map instead of the studio; world: the light stays in the world
use eframe::egui;
use eframe::egui_glow;
use materialcraft::gpu3d::{self, Display, MapInputs, Shared};
use materialcraft::render3d::View;
use materialcraft::tex::Tex;
use std::sync::Arc;

fn bricks(n: usize) -> (Tex, Tex, Tex, Tex, Tex) {
    let height = |u: f32, v: f32| -> f32 {
        let row = (v * 8.0).floor();
        let x = (u * 4.0 + (row as i32 % 2) as f32 * 0.5).rem_euclid(1.0);
        let y = (v * 8.0).rem_euclid(1.0);
        let d = (x.min(1.0 - x) * 2.0).min(y.min(1.0 - y) * 4.0);
        let s = ((d - 0.05) / 0.2).clamp(0.0, 1.0);
        let m = s * s * (3.0 - 2.0 * s);
        let grain = ((u * 190.0).sin() * (v * 230.0).cos() + (u * 63.0 + v * 41.0).sin()) * 0.03;
        (0.15 + 0.85 * m + grain * m).clamp(0.0, 1.0)
    };
    let h = Tex::gray_fn(n, n, |u, v| height(u, v));
    let e = 1.0 / n as f32;
    let normal = Tex::from_fn(n, n, |u, v| {
        let dx = (height(u + e, v) - height(u - e, v)) / (2.0 * e);
        let dy = (height(u, v + e) - height(u, v - e)) / (2.0 * e);
        let k = 0.006;
        let nx = -dx * k;
        let ny = dy * k;
        let l = (nx * nx + ny * ny + 1.0).sqrt();
        [nx / l * 0.5 + 0.5, ny / l * 0.5 + 0.5, 1.0 / l * 0.5 + 0.5, 1.0]
    });
    let base = Tex::from_fn(n, n, |u, v| {
        let hh = height(u, v);
        let t = ((hh - 0.15) / 0.85).clamp(0.0, 1.0);
        let k = ((u * 37.0).floor() * 7.0 + (v * 8.0).floor() * 13.0).sin() * 0.06;
        [0.20 + t * (0.62 + k), 0.12 + t * (0.28 + k * 0.5), 0.10 + t * (0.22 + k * 0.4), 1.0]
    });
    let rough = Tex::gray_fn(n, n, |u, v| 0.95 - 0.3 * height(u, v));
    let ao = Tex::gray_fn(n, n, |u, v| 0.35 + 0.65 * height(u, v));
    (base, normal, rough, ao, h)
}

struct App {
    gpu: Shared,
    view: View,
    display: Display,
    /// Shoot once the accumulation is complete (else at frame 4).
    still: bool,
    shot_asked: bool,
    out: String,
    frame: u32,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _f: &mut eframe::Frame) {
        let ctx = &ui.ctx().clone();
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| {
            let rect = ui.max_rect();
            let (g, v, d) = (self.gpu.clone(), self.view.clone(), self.display);
            ui.painter().add(egui::PaintCallback {
                rect,
                callback: Arc::new(egui_glow::CallbackFn::new(move |info, painter| {
                    gpu3d::paint_with(&g, &info, painter, &v, &d, &materialcraft::diag::NullSink)
                })),
            });
        });
        self.frame += 1;
        let ready = if self.still { self.frame >= 4 && !self.gpu.lock().unwrap().refining() } else { self.frame == 4 };
        if ready && !self.shot_asked {
            self.gpu.lock().unwrap().want_shot = true;
            self.shot_asked = true;
        }
        if self.shot_asked {
            let mut g = self.gpu.lock().unwrap();
            if let Some(e) = &g.error {
                eprintln!("GPU ERROR: {e}");
                std::process::exit(2);
            }
            if let Some((w, h, rgba)) = g.shot.take() {
                let px: Vec<[f32; 4]> = rgba.chunks(4).map(|c| [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, 1.0]).collect();
                std::fs::write(&self.out, materialcraft::png::encode(w, h, &px, 8)).unwrap();
                println!("renderer: {} | {w}x{h} written to {}", g.renderer, self.out);
                std::process::exit(0);
            }
            if self.frame > 600 {
                eprintln!("no screenshot produced");
                std::process::exit(3);
            }
        }
        ctx.request_repaint();
    }
}

fn main() -> eframe::Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let out = a.get(1).cloned().unwrap_or_else(|| "gpu_shot.png".into());
    let shape = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(2u32);
    let relief = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(3u32);
    let (w, h) = (a.get(4).and_then(|s| s.parse().ok()).unwrap_or(900.0f32), a.get(5).and_then(|s| s.parse().ok()).unwrap_or(700.0f32));
    let wire = a.iter().any(|s| s == "wire");
    let arg = |k: &str| a.iter().find_map(|s| s.strip_prefix(k).map(String::from));
    let gsize: usize = arg("size=").and_then(|s| s.parse().ok()).unwrap_or(1024);
    let gout;
    let maps = if let Some(path) = arg("graph=") {
        // real material: evaluate a .mcg and hand its outputs to the viewer
        let g = materialcraft::graph::Graph::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
        gout = g.evaluate(Some(gsize)).unwrap();
        let get = |k: &str| gout.get(k);
        gpu3d::prepare(&MapInputs {
            base: get("basecolor"),
            normal: get("normal"),
            rough: get("roughness"),
            metal: get("metallic"),
            ao: get("ambient_occlusion"),
            emissive: get("emissive"),
            height: get("height"),
        })
    } else if let Some(r) = arg("rough=") {
        // uniform material test: rough=0.3 metal=0 color=0.8,0.2,0.2
        let r: f32 = r.parse().unwrap_or(0.5);
        let m: f32 = arg("metal=").and_then(|s| s.parse().ok()).unwrap_or(0.0);
        let c: Vec<f32> = arg("color=").map(|s| s.split(',').filter_map(|x| x.parse().ok()).collect()).unwrap_or_default();
        let c = if c.len() == 3 { [c[0], c[1], c[2], 1.0] } else { [0.8, 0.2, 0.2, 1.0] };
        let (b, n, ro, me) = (Tex::solid(4, 4, c), Tex::solid(4, 4, [0.5, 0.5, 1.0, 1.0]), Tex::solid(4, 4, [r, r, r, 1.0]), Tex::solid(4, 4, [m, m, m, 1.0]));
        gpu3d::prepare(&MapInputs { base: Some(&b), normal: Some(&n), rough: Some(&ro), metal: Some(&me), ao: None, emissive: None, height: None })
    } else {
        let (base, normal, rough, ao, height) = bricks(1024);
        gpu3d::prepare(&MapInputs {
            base: Some(&base),
            normal: Some(&normal),
            rough: Some(&rough),
            metal: None,
            ao: Some(&ao),
            emissive: None,
            height: Some(&height),
        })
    };
    let gpu = gpu3d::new_shared();
    gpu.lock().unwrap().set_maps(maps);
    if let Some(path) = arg("env=") {
        let t0 = std::time::Instant::now();
        let env = materialcraft::envmap::EnvMap::from_bytes(&std::fs::read(&path).unwrap()).unwrap();
        println!("environment {path}: prefiltered in {:?}, {} dominant lights", t0.elapsed(), env.lights.len());
        gpu.lock().unwrap().set_environment(Some(Arc::new(env)));
    }
    let mut view =
        View { shape, relief, wireframe: wire, yaw: 0.35, pitch: 0.5, dist: if shape == 2 { 3.2 } else { 3.4 }, height_scale: 0.08, ..View::default() };
    view.light_follows_camera = !a.iter().any(|s| s == "world");
    if let Some(y) = arg("lightyaw=").and_then(|s| s.parse().ok()) {
        view.light_yaw = y;
    }
    let still = a.iter().any(|s| s == "still");
    let mut display = if still { Display::default() } else { Display::plain() };
    display.environment.backdrop.opacity = arg("bg=").and_then(|s| s.parse().ok()).unwrap_or(0.0);
    display.environment.backdrop.blur = arg("blur=").and_then(|s| s.parse().ok()).unwrap_or(display.environment.backdrop.blur);
    display.environment.exposure_stops = arg("envexp=").and_then(|s| s.parse().ok()).unwrap_or(0.0);
    if a.iter().any(|s| s == "noshadow") {
        display.shadows.enabled = false;
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([w, h]),
        multisampling: if a.iter().any(|s| s == "nomsaa") { 0 } else { 4 },
        depth_buffer: 24,
        ..Default::default()
    };
    eframe::run_native("gpu_shot", options, Box::new(move |_cc| Ok(Box::new(App { gpu, view, display, still, shot_asked: false, out, frame: 0 }))))
}

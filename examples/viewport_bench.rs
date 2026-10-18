// SPDX-License-Identifier: MIT OR Apache-2.0
//! Frame times of the GPU viewport while the camera moves and while it stands still, with and without frame
//! accumulation (the gate of step 33: accumulation must never lower the interaction frame rate).
//! usage: cargo run --release --features gui --example viewport_bench -- [plain|standard|high|dof] [width height] [frames]
//! Vsync is off, so frames follow each other as fast as the GPU draws them; prints the median and 95th percentile
//! of the time between frames (the interaction frame time) for the moving and the still phase.
use eframe::egui;
use eframe::egui_glow;
use materialcraft::gpu3d::{self, Antialiasing, Display, MapInputs, Shared};
use materialcraft::render3d::{RELIEF_DISPLACE, View};
use materialcraft::tex::Tex;
use std::sync::Arc;
use std::time::Instant;

struct App {
    gpu: Shared,
    view: View,
    display: Display,
    frame: u32,
    frames: u32,
    times: Vec<f64>,
    last: Option<Instant>,
}

fn stats(name: &str, t: &mut [f64]) {
    t.sort_by(|a, b| a.total_cmp(b));
    let at = |q: f64| t.get(((t.len() as f64 - 1.0) * q).round() as usize).copied().unwrap_or(0.0);
    println!("{name}: {} frames, median {:.3} ms, p95 {:.3} ms", t.len(), at(0.5), at(0.95));
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _f: &mut eframe::Frame) {
        let ctx = &ui.ctx().clone();
        let warmup = 20;
        // moving: the camera turns every frame; still: it stays
        if self.frame < warmup + self.frames {
            self.view.yaw += 0.01;
        }
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
        let now = Instant::now();
        if let Some(last) = self.last.replace(now)
            && self.frame > warmup
        {
            self.times.push((now - last).as_secs_f64() * 1000.0);
        }
        self.frame += 1;
        if self.frame == warmup + self.frames {
            stats("moving", &mut self.times);
            self.times.clear();
        }
        if self.frame == warmup + 2 * self.frames {
            stats("still ", &mut self.times);
            if let Some(e) = self.gpu.lock().ok().and_then(|g| g.error.clone()) {
                eprintln!("GPU ERROR: {e}");
            }
            std::process::exit(0);
        }
        ctx.request_repaint();
    }
}

fn main() -> eframe::Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let mut display = Display::default();
    match a.get(1).map(String::as_str).unwrap_or("standard") {
        "plain" => display.antialiasing = Antialiasing::Off,
        "high" => display.antialiasing = Antialiasing::High,
        "dof" => display.camera.depth_of_field.enabled = true,
        _ => {}
    }
    let (w, h) = (a.get(2).and_then(|s| s.parse().ok()).unwrap_or(1280.0f32), a.get(3).and_then(|s| s.parse().ok()).unwrap_or(800.0f32));
    let frames = a.get(4).and_then(|s| s.parse().ok()).unwrap_or(300u32);
    let n = 1024;
    let height = Tex::gray_fn(n, n, |u, v| (0.5 + 0.5 * (u * 40.0).sin() * (v * 30.0).cos()).clamp(0.0, 1.0));
    let base = Tex::from_fn(n, n, |u, v| [0.6 + 0.3 * (u * 9.0).sin(), 0.4, 0.3 + 0.2 * (v * 7.0).cos(), 1.0]);
    let rough = Tex::gray_fn(n, n, |u, _| 0.3 + 0.5 * u);
    let maps =
        gpu3d::prepare(&MapInputs { base: Some(&base), normal: None, rough: Some(&rough), metal: None, ao: None, emissive: None, height: Some(&height) });
    let gpu = gpu3d::new_shared();
    if let Ok(mut g) = gpu.lock() {
        g.set_maps(maps);
    }
    let view = View { shape: 0, relief: RELIEF_DISPLACE, height_scale: 0.05, ..View::default() };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([w, h]),
        multisampling: 4,
        depth_buffer: 24,
        glow_options: eframe::egui_glow::GlowConfiguration { vsync: false, ..Default::default() },
        ..Default::default()
    };
    eframe::run_native(
        "viewport_bench",
        options,
        Box::new(move |_cc| Ok(Box::new(App { gpu, view, display, frame: 0, frames, times: Vec::new(), last: None }))),
    )
}

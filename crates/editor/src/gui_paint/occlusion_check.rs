//! [`OcclusionCheck`]: compares the CPU view projector with the GPU viewport's depth buffer on the open document
//! (`--occlusion-check <out.png>`): once the scene has settled it reads the depth of a frame back from each of a few
//! camera positions (the current one, then [`ORBITS`]), prints how often picks and occlusion rays agree with it,
//! writes a map of the agreement per position (`out.png`, `out-1.png`, ...) and closes the editor.
//!
//! The map: dark where both see nothing, grey where both see the same surface, red where only the CPU picks a
//! surface, blue where only the GPU drew one, yellow where both see a surface at different depths.

use eframe::egui::{self, ViewportCommand};
use materialcraft::camera::Camera;
use materialcraft::deflate::Level;
use materialcraft::gpu3d::{DepthShot, Shared};
use materialcraft::paint::Document;
use materialcraft::png::{self, ColorType};
use materialcraft::projection::view::{DepthAgreement, DepthBuffer, PixelAgreement, compare_depth};
use materialcraft::projection::{CpuViewProjector, ViewProjector};
use materialcraft::render3d::View;
use std::path::PathBuf;

/// Frames to draw after settling before the depth is read (as for screenshots).
const SETTLE_FRAMES: u64 = 3;
/// Depths agree within this many world units (the model is fitted into a unit sphere).
const TOLERANCE: f32 = 0.005;
/// Every how many pixels a pick is compared.
const STRIDE: usize = 2;
/// At most this many surface points (triangle centres) are checked for occlusion.
const MAX_POINTS: usize = 200_000;
/// Barycentric weights of the points checked per triangle: the centre first, then spread over the triangle.
const WEIGHTS: [[f32; 3]; 10] = [
    [1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0],
    [0.6, 0.2, 0.2],
    [0.2, 0.6, 0.2],
    [0.2, 0.2, 0.6],
    [0.1, 0.45, 0.45],
    [0.45, 0.1, 0.45],
    [0.45, 0.45, 0.1],
    [0.8, 0.1, 0.1],
    [0.1, 0.8, 0.1],
    [0.1, 0.1, 0.8],
];
/// Camera moves after each check: yaw and pitch added (radians), distance factor, pan.
const ORBITS: [(f32, f32, f32, [f32; 2]); 3] = [(1.2, 0.45, 1.0, [0.0, 0.0]), (3.0, -0.35, 0.8, [0.1, 0.05]), (-0.9, 0.9, 0.55, [-0.05, 0.1])];

pub struct OcclusionCheck {
    out: PathBuf,
    settled_at: Option<u64>,
    requested: bool,
    /// Camera positions checked so far.
    done: usize,
    /// Covered pixels and agreeing ones, surface points and agreeing ones, over all positions.
    totals: [usize; 4],
}

impl OcclusionCheck {
    pub fn new(out: PathBuf) -> OcclusionCheck {
        OcclusionCheck { out, settled_at: None, requested: false, done: 0, totals: [0; 4] }
    }

    /// Call every frame. `settled`: nothing is loading or building; `frames`: the GPU frame count; `view`: the
    /// viewport's camera settings (moved between checks); `projector` makes the CPU projector for the read-back's
    /// camera.
    #[allow(clippy::too_many_arguments)]
    pub fn update<'a>(
        &mut self,
        ctx: &egui::Context,
        settled: bool,
        frames: u64,
        gpu: &Shared,
        view: &mut View,
        doc: Option<&Document>,
        projector: impl FnOnce(Camera) -> Option<CpuViewProjector<'a>>,
    ) {
        ctx.request_repaint();
        if self.requested {
            let Some(shot) = gpu.lock().ok().and_then(|mut g| g.depth.take()) else { return };
            let result = match (doc, projector(shot.camera)) {
                (Some(doc), Some(cpu)) => self.report(doc, &cpu, &shot),
                _ => Err("the view scene is not ready".to_string()),
            };
            if let Err(e) = result {
                eprintln!("occlusion check failed: {e}");
                std::process::exit(1);
            }
            self.done += 1;
            self.requested = false;
            self.settled_at = None;
            self.next_view(ctx, view);
            return;
        }
        match (settled, self.settled_at) {
            (false, _) => self.settled_at = None,
            (true, None) => self.settled_at = Some(frames),
            (true, Some(at)) if frames >= at + SETTLE_FRAMES => {
                if let Ok(mut g) = gpu.lock() {
                    g.want_depth = true;
                    self.requested = true;
                }
            }
            _ => {}
        }
    }

    /// Moves the camera to the next position, or prints the totals and closes the editor after the last.
    fn next_view(&self, ctx: &egui::Context, view: &mut View) {
        match self.done.checked_sub(1).and_then(|i| ORBITS.get(i)) {
            Some(&(yaw, pitch, dist, pan)) => {
                view.yaw += yaw;
                view.pitch = (view.pitch + pitch).clamp(-1.5, 1.5);
                view.dist *= dist;
                (view.pan_x, view.pan_y) = (pan[0], pan[1]);
            }
            None => {
                let [covered, same, points, agree] = self.totals;
                println!(
                    "occlusion check, all {} views: picks agree on {:.2}% of {covered} covered pixels, occlusion on {:.2}% of {points} surface points",
                    self.done,
                    percent(same, covered),
                    percent(agree, points)
                );
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        }
    }

    fn report(&mut self, doc: &Document, view: &CpuViewProjector, shot: &DepthShot) -> Result<(), String> {
        let scene = view.scene();
        let shown = || doc.mesh.parts.iter().enumerate().filter(|(i, _)| scene.is_shown(*i)).map(|(_, p)| p);
        let total: usize = shown().map(|p| p.triangles.len()).sum();
        // up to MAX_POINTS points: several per triangle on small meshes, every n-th triangle's centre on large ones
        let every = total.div_ceil(MAX_POINTS).max(1);
        let per = (MAX_POINTS / total.max(1)).clamp(1, WEIGHTS.len());
        let points = shown()
            .flat_map(|p| p.triangles.iter().map(move |t| t.map(|v| p.positions.get(v as usize).copied().unwrap_or_default())))
            .step_by(every)
            .flat_map(|c| WEIGHTS.iter().take(per).map(move |w| [0, 1, 2].map(|k| c[0][k] * w[0] + c[1][k] * w[1] + c[2][k] * w[2])));
        let buffer = DepthBuffer { width: shot.width, height: shot.height, depth: &shot.depth };
        let r = compare_depth(view as &dyn ViewProjector, &buffer, points, STRIDE, TOLERANCE);
        for (t, v) in self.totals.iter_mut().zip([r.covered, r.covered_same, r.points, r.points_same]) {
            *t += v;
        }
        println!(
            "occlusion check, view {}: {}x{} px, picks agree on {:.2}% of {} covered pixels (every {STRIDE}th); occlusion agrees on {:.2}% of {} surface points ({} occluded by ray, {} by depth; {} within a pixel of an edge not counted); pick → screen round trip within {:.4} px",
            self.done,
            shot.width,
            shot.height,
            r.pixel_percent(),
            r.covered,
            r.point_percent(),
            r.points,
            r.occluded_by_ray,
            r.occluded_by_buffer,
            r.points_ambiguous,
            r.worst_round_trip
        );
        self.save_map(&r)
    }

    fn save_map(&self, r: &DepthAgreement) -> Result<(), String> {
        let w = r.map_width.max(1);
        let h = r.map.len() / w;
        let rgba: Vec<u8> = r
            .map
            .iter()
            .flat_map(|c| match c {
                PixelAgreement::Empty => [20, 20, 24, 255],
                PixelAgreement::Same => [150, 150, 150, 255],
                PixelAgreement::ProjectorOnly => [230, 40, 40, 255],
                PixelAgreement::BufferOnly => [40, 90, 230, 255],
                PixelAgreement::DepthDiffers => [240, 210, 40, 255],
            })
            .collect();
        let bytes = png::encode_raw(w, h, ColorType::Rgba, 8, &rgba, Level::Default);
        let out = self.map_path();
        std::fs::write(&out, bytes).map_err(|e| format!("{}: {e}", out.display()))?;
        println!("occlusion map {}", out.display());
        Ok(())
    }

    /// `out.png` for the first view, `out-1.png`, `out-2.png`, ... for the others.
    fn map_path(&self) -> PathBuf {
        if self.done == 0 {
            return self.out.clone();
        }
        let stem = self.out.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let ext = self.out.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
        self.out.with_file_name(format!("{stem}-{}{ext}", self.done))
    }
}

fn percent(part: usize, total: usize) -> f32 {
    if total == 0 { 100.0 } else { 100.0 * part as f32 / total as f32 }
}

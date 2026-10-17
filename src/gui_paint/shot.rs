//! [`ShotJob`]: automated screenshots for testing. Waits until the scene has settled and a few frames
//! were drawn with the final maps, saves the whole window as PNG and closes the editor.

use eframe::egui::{self, ColorImage, ViewportCommand};
use materialcraft::deflate::Level;
use materialcraft::png::{self, ColorType};
use std::path::PathBuf;

/// Frames to draw after the last upload before the screenshot (uploads happen during paint).
const SETTLE_FRAMES: u64 = 3;

pub struct ShotJob {
    out: PathBuf,
    /// GPU frame count when the scene was first seen settled.
    settled_at: Option<u64>,
    requested: bool,
}

impl ShotJob {
    pub fn new(out: PathBuf) -> ShotJob {
        ShotJob { out, settled_at: None, requested: false }
    }

    /// Call every frame. `settled` is true when no work is pending; `frames` is the GPU frame count.
    pub fn update(&mut self, ctx: &egui::Context, settled: bool, frames: u64) {
        ctx.request_repaint();
        if let Some(image) = screenshot_event(ctx) {
            let code = match self.save(&image) {
                Ok(()) => {
                    println!("screenshot {}", self.out.display());
                    0
                }
                Err(e) => {
                    eprintln!("screenshot failed: {e}");
                    1
                }
            };
            ctx.send_viewport_cmd(ViewportCommand::Close);
            if code != 0 {
                std::process::exit(code);
            }
            return;
        }
        if self.requested {
            return;
        }
        match (settled, self.settled_at) {
            (false, _) => self.settled_at = None,
            (true, None) => self.settled_at = Some(frames),
            (true, Some(at)) if frames >= at + SETTLE_FRAMES => {
                self.requested = true;
                ctx.send_viewport_cmd(ViewportCommand::Screenshot);
            }
            _ => {}
        }
    }

    /// Gives up (e.g. the document failed to load): reports the error and exits with status 1.
    pub fn abort(&self, why: &str) -> ! {
        eprintln!("screenshot aborted: {why}");
        std::process::exit(1)
    }

    fn save(&self, image: &ColorImage) -> Result<(), String> {
        let [w, h] = image.size;
        let rgba: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
        let bytes = png::encode_raw(w, h, ColorType::Rgba, 8, &rgba, Level::Default);
        std::fs::write(&self.out, bytes).map_err(|e| format!("{}: {e}", self.out.display()))
    }
}

fn screenshot_event(ctx: &egui::Context) -> Option<std::sync::Arc<ColorImage>> {
    ctx.input(|i| {
        i.raw.events.iter().find_map(|e| match e {
            egui::Event::Screenshot { image, .. } => Some(image.clone()),
            _ => None,
        })
    })
}

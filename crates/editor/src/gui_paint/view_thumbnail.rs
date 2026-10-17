//! A thumbnail of the 3D view, for a smart material or mask being saved: a screenshot of the window cropped to the
//! largest centred square of the view and scaled down to at most 256 pixels. The screenshot arrives a frame after
//! it was asked for, so the caller asks before it shows anything over the view.

use eframe::egui::{self, ColorImage, Rect, ViewportCommand};
use materialcraft::tex::Tex;
use std::sync::Arc;

/// Longest side of the thumbnail.
const MAX_SIZE: usize = 256;
/// Frames to wait for the screenshot before going on without one.
const PATIENCE: u32 = 10;

/// Marks our screenshot requests (the `--shot` test mode takes screenshots too).
struct Tag;

/// A screenshot on its way.
pub struct ViewShot {
    tag: Arc<Tag>,
    frames: u32,
}

impl ViewShot {
    /// Asks for a screenshot of this frame.
    pub fn request(ctx: &egui::Context) -> ViewShot {
        let tag = Arc::new(Tag);
        let data: Arc<dyn std::any::Any + Send + Sync> = tag.clone();
        ctx.send_viewport_cmd(ViewportCommand::Screenshot(egui::UserData { data: Some(data) }));
        ctx.request_repaint();
        ViewShot { tag, frames: 0 }
    }

    /// Call every frame: `Some` once done, with the thumbnail of `view` (in points), or `None` if no screenshot came.
    pub fn poll(&mut self, ctx: &egui::Context, view: Option<Rect>) -> Option<Option<Tex>> {
        let ours = |d: &egui::UserData| d.data.as_ref().is_some_and(|a| a.downcast_ref::<Tag>().is_some_and(|t| std::ptr::eq(t, &*self.tag)));
        let image = ctx.input(|i| {
            i.raw.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, user_data, .. } if ours(user_data) => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = image {
            let ppp = ctx.pixels_per_point();
            return Some(view.and_then(|r| thumbnail(&image, (r.min.x * ppp, r.min.y * ppp, r.width() * ppp, r.height() * ppp))));
        }
        self.frames += 1;
        ctx.request_repaint();
        (self.frames > PATIENCE).then_some(None)
    }
}

/// The largest centred square of `view` (`x, y, w, h` in pixels of `image`), scaled down by averaging to at most
/// [`MAX_SIZE`] pixels: sRGB values 0…1, opaque. `None` if the square is empty.
pub fn thumbnail(image: &ColorImage, view: (f32, f32, f32, f32)) -> Option<Tex> {
    let [iw, ih] = image.size;
    let (x0, y0) = (view.0.max(0.0) as usize, view.1.max(0.0) as usize);
    let (x1, y1) = (((view.0 + view.2).max(0.0) as usize).min(iw), ((view.1 + view.3).max(0.0) as usize).min(ih));
    let side = x1.saturating_sub(x0).min(y1.saturating_sub(y0));
    if side == 0 {
        return None;
    }
    let (sx, sy) = (x0 + (x1 - x0 - side) / 2, y0 + (y1 - y0 - side) / 2);
    let out = side.min(MAX_SIZE);
    let mut px = Vec::with_capacity(out * out);
    for ty in 0..out {
        let (ya, yb) = (sy + ty * side / out, sy + ((ty + 1) * side / out).max(ty * side / out + 1));
        for tx in 0..out {
            let (xa, xb) = (sx + tx * side / out, sx + ((tx + 1) * side / out).max(tx * side / out + 1));
            let mut sum = [0u32; 3];
            for y in ya..yb {
                for x in xa..xb {
                    let c = image.pixels.get(y * iw + x).copied().unwrap_or_default();
                    sum[0] += u32::from(c.r());
                    sum[1] += u32::from(c.g());
                    sum[2] += u32::from(c.b());
                }
            }
            let n = ((yb - ya) * (xb - xa)).max(1) as f32 * 255.0;
            px.push([sum[0] as f32 / n, sum[1] as f32 / n, sum[2] as f32 / n, 1.0]);
        }
    }
    Some(Tex { w: out, h: out, px })
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::Color32;

    #[test]
    fn the_view_is_cropped_to_a_centred_square_and_scaled_down() {
        // A 1000×600 window whose view is the right 800×600 part: red in the middle third, blue elsewhere.
        let (w, h) = (1000, 600);
        let pixels = (0..w * h).map(|i| if (i % w) >= 200 + 300 && (i % w) < 200 + 500 { Color32::RED } else { Color32::BLUE }).collect();
        let image = ColorImage { size: [w, h], source_size: egui::vec2(w as f32, h as f32), pixels };
        let t = thumbnail(&image, (200.0, 0.0, 800.0, 600.0)).unwrap();
        assert_eq!((t.w, t.h), (256, 256));
        // The square is x 300…900 of the view's 200…1000: red from 500 to 700 is its middle third.
        assert_eq!(t.px[128 * 256 + 128], [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(t.px[128 * 256 + 10], [0.0, 0.0, 1.0, 1.0]);
        assert!(thumbnail(&image, (0.0, 0.0, 0.0, 10.0)).is_none());
        let small = thumbnail(&image, (0.0, 0.0, 100.0, 50.0)).unwrap();
        assert_eq!((small.w, small.h), (50, 50), "never scaled up");
    }
}

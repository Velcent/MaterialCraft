//! Showing the stencil and the projection image over the view: the image, repeated where it repeats, see-through,
//! with its outline (and the centre the gizmo turns and scales about while the stencil key is held).

use super::{ScreenImage, ScreenImages};
use eframe::egui::{self, Color32, ColorImage, Mesh, Pos2, Rect, Stroke, TextureOptions};
use materialcraft::projection::ScreenImageTransform;

/// Most repetitions drawn (a tiny repeating image is shown as its outline only beyond this).
const MAX_TILES: i64 = 2500;

/// How an image is shown.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Look {
    /// See-through amount, 0 … 1.
    pub opacity: f32,
    /// Draw the gizmo's centre mark.
    pub gizmo: bool,
}

impl ScreenImages {
    /// Draws the stencil (unless hidden) and, while the projection tool is on, the projection image over the view
    /// at `rect` with `painter`; `gizmo` while the stencil key is held.
    pub fn draw(&mut self, ctx: &egui::Context, painter: &egui::Painter, rect: Rect, projection_tool: bool, gizmo: bool) {
        let viewport = [rect.width(), rect.height()];
        if projection_tool {
            if let Some(image) = self.projection.as_mut() {
                draw(ctx, painter, rect, image, viewport, Look { opacity: 0.6, gizmo });
            }
        } else if let (false, Some(image)) = (self.hidden, self.stencil.as_mut()) {
            draw(ctx, painter, rect, image, viewport, Look { opacity: 0.35, gizmo });
        }
    }
}

fn draw(ctx: &egui::Context, painter: &egui::Painter, rect: Rect, image: &mut ScreenImage, viewport: [f32; 2], look: Look) {
    let t = image.placement(viewport);
    if image.texture.is_none() {
        image.texture = Some(ctx.load_texture(format!("screen_image_{}", image.name), color_image(image), TextureOptions::LINEAR));
    }
    let Some(texture) = &image.texture else { return };
    let at = |p: [f32; 2]| Pos2::new(rect.min.x + p[0], rect.min.y + p[1]);
    let tint = Color32::from_white_alpha((look.opacity.clamp(0.0, 1.0) * 255.0) as u8);
    let (us, vs) = tiles(&t, viewport);
    if (us.end - us.start).saturating_mul(vs.end - vs.start) <= MAX_TILES {
        let mut mesh = Mesh::with_texture(texture.id());
        for j in vs.clone() {
            for i in us.clone() {
                let (i, j) = (i as f32, j as f32);
                let corners = [[i, j], [i + 1.0, j], [i + 1.0, j + 1.0], [i, j + 1.0]];
                let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
                let base = mesh.vertices.len() as u32;
                for (c, uv) in corners.iter().zip(uvs) {
                    mesh.vertices.push(egui::epaint::Vertex { pos: at(t.to_screen(*c)), uv: Pos2::new(uv[0], uv[1]), color: tint });
                }
                mesh.add_triangle(base, base + 1, base + 2);
                mesh.add_triangle(base, base + 2, base + 3);
            }
        }
        painter.add(egui::Shape::mesh(mesh));
    }
    let outline: Vec<Pos2> = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0], [0.0, 0.0]].iter().map(|c| at(t.to_screen(*c))).collect();
    painter.add(egui::Shape::line(outline, Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 150))));
    if look.gizmo {
        let c = at(t.centre);
        painter.circle_stroke(c, 6.0, Stroke::new(1.5, Color32::from_rgb(255, 205, 90)));
        painter.line_segment([c - egui::vec2(10.0, 0.0), c + egui::vec2(10.0, 0.0)], Stroke::new(1.0, Color32::from_rgb(255, 205, 90)));
        painter.line_segment([c - egui::vec2(0.0, 10.0), c + egui::vec2(0.0, 10.0)], Stroke::new(1.0, Color32::from_rgb(255, 205, 90)));
    }
}

/// The repetitions (image-coordinate cells) that reach into a view of `viewport` units: one without repetition.
fn tiles(t: &ScreenImageTransform, viewport: [f32; 2]) -> (std::ops::Range<i64>, std::ops::Range<i64>) {
    let corners = [[0.0, 0.0], [viewport[0], 0.0], [viewport[0], viewport[1]], [0.0, viewport[1]]].map(|p| t.to_image(p));
    let span = |axis: usize, repeats: bool| {
        if !repeats {
            return 0..1;
        }
        let lo = corners.iter().map(|c| c[axis]).fold(f32::MAX, f32::min).floor();
        let hi = corners.iter().map(|c| c[axis]).fold(f32::MIN, f32::max).ceil();
        if lo.is_finite() && hi.is_finite() { (lo as i64)..(hi as i64).max(lo as i64 + 1) } else { 0..1 }
    };
    (span(0, t.repeat.u()), span(1, t.repeat.v()))
}

fn color_image(image: &ScreenImage) -> ColorImage {
    let p = &image.preview;
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    let rgba: Vec<u8> = p.px.iter().flat_map(|c| [byte(c[0]), byte(c[1]), byte(c[2]), byte(c[3])]).collect();
    ColorImage::from_rgba_unmultiplied([p.w, p.h], &rgba)
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft::projection::Repeat;

    #[test]
    fn repeated_images_cover_the_view_and_single_ones_draw_once() {
        let mut t = ScreenImageTransform { centre: [200.0, 150.0], size: [50.0, 50.0], rotation: 0.0, repeat: Repeat::None };
        assert_eq!(tiles(&t, [400.0, 300.0]), (0..1, 0..1));
        t.repeat = Repeat::Both;
        let (u, v) = tiles(&t, [400.0, 300.0]);
        assert_eq!((u.end - u.start, v.end - v.start), (9, 7), "{u:?} {v:?}");
        t.repeat = Repeat::U;
        assert_eq!(tiles(&t, [400.0, 300.0]).1, 0..1);
    }
}

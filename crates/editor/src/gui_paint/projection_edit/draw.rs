//! Drawing the projection gizmo over the view: the volume's box, its handles in the axis colours (x red, y green,
//! z blue, the centre handles light), the handle under the pointer brighter; in UV mode the frame of one image
//! repetition over the mesh with its knobs.

use super::view::{GizmoHit, GizmoView, KNOB_RADIUS};
use eframe::egui::{self, Color32, Pos2, Stroke};
use materialcraft::projection::gizmo::{ScreenShape, ShapeKind};

const BOX: Color32 = Color32::from_rgba_premultiplied(150, 150, 150, 150);
const FRAME: Color32 = Color32::from_rgb(255, 205, 90);

fn axis_color(axis: Option<usize>) -> Color32 {
    match axis {
        Some(0) => Color32::from_rgb(230, 80, 70),
        Some(1) => Color32::from_rgb(110, 205, 80),
        Some(2) => Color32::from_rgb(80, 140, 240),
        _ => Color32::from_gray(225),
    }
}

/// Draws `gizmo` with `painter`, the view's top-left corner at `origin`; `hover` is the part under the pointer (or
/// being dragged).
pub fn draw(gizmo: &GizmoView, painter: &egui::Painter, origin: Pos2, hover: Option<GizmoHit>) {
    let at = |p: [f32; 2]| Pos2::new(origin.x + p[0], origin.y + p[1]);
    match gizmo {
        GizmoView::Volume(v) => {
            for [a, b] in &v.outline {
                painter.line_segment([at(*a), at(*b)], Stroke::new(1.0, BOX));
            }
            if let Some([a, b]) = v.direction {
                painter.arrow(at(a), at(b) - at(a), Stroke::new(1.0, BOX));
            }
            for s in &v.shapes {
                let hot = hover == Some(GizmoHit::Volume(s.handle));
                shape(painter, s, axis_color(s.handle.axis()), hot, &at);
            }
        }
        GizmoView::Uv(v) => {
            for run in &v.outline {
                painter.add(egui::Shape::line(run.iter().map(|p| at(*p)).collect(), Stroke::new(1.5, FRAME)));
            }
            for s in &v.shapes {
                let hot = hover == Some(GizmoHit::Uv(s.handle));
                if s.kind != ShapeKind::Area {
                    shape(painter, s, FRAME, hot, &at);
                }
            }
        }
    }
}

fn shape<H>(painter: &egui::Painter, s: &ScreenShape<H>, color: Color32, hot: bool, at: &impl Fn([f32; 2]) -> Pos2) {
    let color = if hot { color.gamma_multiply(1.4) } else { color.gamma_multiply(0.85) };
    let width = if hot { 3.0 } else { 2.0 };
    let points: Vec<Pos2> = s.points.iter().map(|p| at(*p)).collect();
    match s.kind {
        ShapeKind::Line => {
            painter.add(egui::Shape::line(points, Stroke::new(width, color)));
        }
        ShapeKind::Area => {
            painter.add(egui::Shape::convex_polygon(points, color.gamma_multiply(if hot { 0.7 } else { 0.35 }), Stroke::new(1.0, color)));
        }
        ShapeKind::Knob => {
            if let Some(p) = points.first() {
                painter.circle_filled(*p, KNOB_RADIUS * if hot { 0.8 } else { 0.6 }, color);
            }
        }
    }
}

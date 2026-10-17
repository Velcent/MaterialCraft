//! One small icon per asset usage, drawn with the painter in the line style of the editor's icons (WS-14): the
//! type filter buttons and the stand-in for an asset without a picture.

use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, vec2};
use materialcraft::assets::Usage;

/// The display name of a usage (plural, for the filter).
pub fn label(usage: Usage) -> &'static str {
    match usage {
        Usage::Material => "Materials",
        Usage::SmartMaterial => "Smart materials",
        Usage::SmartMask => "Smart masks",
        Usage::Generator => "Generators",
        Usage::Filter => "Filters",
        Usage::Brush => "Brushes",
        Usage::Alpha => "Alphas",
        Usage::Texture => "Textures",
        Usage::Environment => "Environments",
    }
}

/// Paints `usage`'s icon centred in `rect`.
pub fn paint(painter: &Painter, rect: Rect, usage: Usage, color: Color32) {
    let s = rect.width().min(rect.height()) * 0.5;
    let c = rect.center();
    let p = |x: f32, y: f32| c + vec2(x * s, y * s);
    let line = Stroke::new((s * 0.12).max(1.2), color);
    let poly = |pts: &[Pos2]| {
        for w in pts.windows(2) {
            if let [a, b] = w {
                painter.line_segment([*a, *b], line);
            }
        }
    };
    match usage {
        Usage::Material => {
            painter.circle_stroke(c, s * 0.6, line);
            painter.circle_filled(p(-0.22, -0.22), s * 0.14, color);
        }
        Usage::SmartMaterial => {
            painter.circle_stroke(p(0.0, 0.1), s * 0.5, line);
            poly(&[p(-0.6, -0.55), p(-0.1, -0.55)]);
            poly(&[p(-0.35, -0.8), p(-0.35, -0.3)]);
        }
        Usage::SmartMask => {
            painter.rect_stroke(Rect::from_center_size(c, vec2(s * 1.2, s * 1.2)), 2.0, line, StrokeKind::Middle);
            painter.circle_filled(c, s * 0.35, color);
        }
        Usage::Generator => {
            poly(&[p(-0.6, 0.6), p(-0.2, -0.1), p(0.1, 0.25), p(0.6, -0.6)]);
            painter.circle_filled(p(0.6, -0.6), s * 0.12, color);
        }
        Usage::Filter => {
            painter.add(Shape::convex_polygon(vec![p(-0.65, -0.55), p(0.65, -0.55), p(0.12, 0.1), p(-0.12, 0.1)], Color32::TRANSPARENT, line));
            poly(&[p(0.0, 0.1), p(0.0, 0.65)]);
        }
        Usage::Brush => {
            poly(&[p(0.55, -0.6), p(-0.05, 0.0)]);
            painter.circle_filled(p(-0.25, 0.25), s * 0.28, color);
        }
        Usage::Alpha => {
            painter.circle_filled(c, s * 0.6, color.gamma_multiply(0.35));
            painter.circle_filled(c, s * 0.3, color);
        }
        Usage::Texture => {
            painter.rect_stroke(Rect::from_center_size(c, vec2(s * 1.3, s * 1.3)), 1.0, line, StrokeKind::Middle);
            poly(&[p(-0.65, 0.4), p(-0.2, -0.05), p(0.15, 0.3), p(0.35, 0.1), p(0.65, 0.4)]);
        }
        Usage::Environment => {
            painter.circle_stroke(c, s * 0.65, line);
            poly(&[p(-0.65, 0.0), p(0.65, 0.0)]);
            painter.add(Shape::ellipse_stroke(c, vec2(s * 0.3, s * 0.65), line));
        }
    }
}

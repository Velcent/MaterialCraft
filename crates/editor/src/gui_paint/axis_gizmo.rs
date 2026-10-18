//! The axis gizmo (VP-08): the world's X, Y and Z axes as seen from the current camera, drawn in a corner of the
//! 3D view. Axes pointing away from the viewer are drawn first and dimmed, so the nearest one stays on top.

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Stroke};
use materialcraft::camera::Camera;

/// Length of an axis on screen, in points.
const LENGTH: f32 = 22.0;
/// Distance of the gizmo's centre from the view's corner.
const INSET: f32 = 34.0;

const AXES: [([f32; 3], &str, Color32); 3] = [
    ([1.0, 0.0, 0.0], "X", Color32::from_rgb(230, 80, 80)),
    ([0.0, 1.0, 0.0], "Y", Color32::from_rgb(110, 200, 90)),
    ([0.0, 0.0, 1.0], "Z", Color32::from_rgb(90, 140, 240)),
];

/// The screen direction (x right, y down) and depth towards the viewer of a world direction under `camera`.
fn on_screen(camera: &Camera, dir: [f32; 3]) -> ([f32; 2], f32) {
    let m = &camera.view;
    let row = |r: usize| m[r] * dir[0] + m[4 + r] * dir[1] + m[8 + r] * dir[2];
    ([row(0), -row(1)], row(2))
}

/// Draws the gizmo in the lower-right corner of `rect`.
pub fn draw(painter: &egui::Painter, rect: egui::Rect, camera: &Camera) {
    let centre = Pos2::new(rect.right() - INSET, rect.bottom() - INSET);
    painter.circle_filled(centre, LENGTH + 8.0, Color32::from_black_alpha(70));
    let mut axes: Vec<([f32; 2], f32, &str, Color32)> = AXES
        .iter()
        .map(|(dir, name, colour)| {
            let (s, depth) = on_screen(camera, *dir);
            (s, depth, *name, *colour)
        })
        .collect();
    // farthest first; view space looks down -z, so a larger z is nearer the viewer
    axes.sort_by(|a, b| a.1.total_cmp(&b.1));
    for (s, depth, name, colour) in axes {
        let colour = if depth < -0.05 { colour.gamma_multiply(0.45) } else { colour };
        let end = centre + egui::vec2(s[0], s[1]) * LENGTH;
        painter.line_segment([centre, end], Stroke::new(2.0, colour));
        painter.circle_filled(end, 6.5, colour);
        painter.text(end, Align2::CENTER_CENTER, name, FontId::proportional(9.5), Color32::from_gray(20));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft::camera::{CameraSettings, Orbit};

    #[test]
    fn the_front_view_shows_x_right_y_up_and_z_towards_the_viewer() {
        let front = CameraSettings::default().camera(&Orbit { yaw: 0.0, pitch: 0.0, ..Orbit::default() }, [100.0, 100.0]);
        let (x, _) = on_screen(&front, [1.0, 0.0, 0.0]);
        let (y, _) = on_screen(&front, [0.0, 1.0, 0.0]);
        let (z, zd) = on_screen(&front, [0.0, 0.0, 1.0]);
        assert!(x[0] > 0.99 && y[1] < -0.99, "{x:?} {y:?}");
        assert!(z[0].abs() < 1e-5 && z[1].abs() < 1e-5 && zd > 0.99);
    }
}

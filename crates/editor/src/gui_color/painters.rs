// The colour field painter (a grid mesh with a colour per vertex) and the transparency checker follow PhotoCraft's
// crates/ui-egui/src/color_picker_ui.rs (`grid_mesh`) and crates/ui-egui/src/widgets.rs (`checker`), revision
// 5f65579, adapted to MaterialCraft's theme. See ATTRIBUTION.md.
//
// Copyright (c) 2026 ArtCraft Team and the PhotoCraft contributors.
// Licensed under either of the Apache License, Version 2.0 or the MIT license, at your option.

//! Painters for colour widgets: smooth colour fields, the hue strip and the transparency checker.

use eframe::egui::{Color32, Mesh, Painter, Pos2, Rect, Vec2, pos2};

/// A field over `rect` coloured by `color(x, y)` (both 0…1, y down) on an `n` × `n` grid of blended cells.
pub fn grid_mesh(rect: Rect, n: usize, color: impl Fn(f32, f32) -> Color32) -> Mesh {
    let n = n.max(1);
    let mut m = Mesh::default();
    for j in 0..=n {
        for i in 0..=n {
            let (x, y) = (i as f32 / n as f32, j as f32 / n as f32);
            m.colored_vertex(pos2(rect.left() + x * rect.width(), rect.top() + y * rect.height()), color(x, y));
        }
    }
    let w = (n + 1) as u32;
    for j in 0..n as u32 {
        for i in 0..n as u32 {
            let a = j * w + i;
            m.add_triangle(a, a + 1, a + w + 1);
            m.add_triangle(a, a + w + 1, a + w);
        }
    }
    m
}

/// A small checkerboard (behind colours with alpha) in `rect`.
pub fn checker(painter: &Painter, rect: Rect, cell: f32) {
    let cell = if cell.is_finite() && cell > 0.0 { cell } else { 4.0 };
    painter.rect_filled(rect, 0.0, Color32::from_gray(200));
    let nx = (rect.width() / cell).ceil().max(0.0) as i32;
    let ny = (rect.height() / cell).ceil().max(0.0) as i32;
    for j in 0..ny {
        for i in 0..nx {
            if (i + j) % 2 == 1 {
                let r = Rect::from_min_size(Pos2::new(rect.left() + i as f32 * cell, rect.top() + j as f32 * cell), Vec2::splat(cell)).intersect(rect);
                painter.rect_filled(r, 0.0, Color32::from_gray(150));
            }
        }
    }
}

//! The image placement block (PJ-05 … PJ-08, PJ-11): one block in every mode but texture space (ADR 0008
//! mc2-q-pj-10): the size mode (tiling, physical, custom: three equal modes; physical is off where it means nothing,
//! and the two size modes are stored but not evaluated yet, mc2-q-pj-04), tiling per axis with an optional locked
//! ratio, rotation, offset (not in tri-planar, mc2-q-pj-11), mirroring, sampling filter and repetition (tri-planar
//! always repeats, mc2-q-pj-12).

use super::heading;
use eframe::egui::{DragValue, Ui};
use materialcraft::projection::{Filter, ImagePlacement, Projection, ProjectionMode, Repeat, SizeMode};

pub fn show(ui: &mut Ui, p: &mut Projection) {
    if matches!(p.mode, ProjectionMode::TextureSpace) {
        filter(ui, p);
        return;
    }
    heading(ui, "Image placement");
    let tri_planar = matches!(p.mode, ProjectionMode::TriPlanar(_));
    let spherical = matches!(p.mode, ProjectionMode::Spherical(_));
    size_mode(ui, &mut p.image, spherical);
    match p.image.size_mode {
        SizeMode::Tiling => tiling(ui, &mut p.image),
        SizeMode::Physical | SizeMode::Custom => physical(ui, &mut p.image),
    }
    ui.horizontal(|ui| {
        ui.label("Rotation");
        ui.add(DragValue::new(&mut p.image.rotation).speed(0.5).suffix("°").max_decimals(2));
    });
    ui.add_enabled_ui(!tri_planar, |ui| {
        ui.horizontal(|ui| {
            ui.label("Offset");
            ui.add(DragValue::new(&mut p.image.offset[0]).speed(0.005).prefix("u ").max_decimals(4));
            ui.add(DragValue::new(&mut p.image.offset[1]).speed(0.005).prefix("v ").max_decimals(4));
        })
        .response
        .on_disabled_hover_text("Tri-planar has no image offset: move its volume instead.");
    });
    ui.horizontal(|ui| {
        ui.label("Mirror");
        for (k, name) in ["U", "V"].iter().enumerate() {
            if ui.button(*name).on_hover_text("Mirror the image along this axis").clicked()
                && let Some(t) = p.image.tiling.get_mut(k)
            {
                *t = -*t;
            }
        }
        if ui.button("Reset").on_hover_text("Tiling 1, no rotation or offset").clicked() {
            p.image = ImagePlacement { size_mode: p.image.size_mode, physical_size: p.image.physical_size, ..ImagePlacement::default() };
        }
    });
    filter(ui, p);
    ui.add_enabled_ui(!tri_planar, |ui| {
        ui.horizontal(|ui| {
            ui.label("Repeat");
            for r in Repeat::ALL {
                let label = match r {
                    Repeat::None => "None",
                    Repeat::U => "U",
                    Repeat::V => "V",
                    Repeat::Both => "Both",
                };
                ui.selectable_value(&mut p.repeat, r, label);
            }
        })
        .response
        .on_disabled_hover_text("Tri-planar always repeats.");
    });
}

fn size_mode(ui: &mut Ui, image: &mut ImagePlacement, spherical: bool) {
    ui.horizontal(|ui| {
        ui.label("Size");
        ui.selectable_value(&mut image.size_mode, SizeMode::Tiling, "Tiling");
        ui.add_enabled_ui(!spherical, |ui| {
            ui.selectable_value(&mut image.size_mode, SizeMode::Physical, "Physical")
                .on_hover_text("The source's real-world size against the mesh's units")
                .on_disabled_hover_text("A spherical projection has no physical size: it wraps the image around the volume.");
        });
        ui.selectable_value(&mut image.size_mode, SizeMode::Custom, "Custom").on_hover_text("A size typed in, in centimetres");
    });
}

/// Tiling per axis; with the ratio locked, changing one changes the other in proportion.
fn tiling(ui: &mut Ui, image: &mut ImagePlacement) {
    let id = ui.id().with("tiling_locked");
    let mut locked = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(true);
    let before = image.tiling;
    ui.horizontal(|ui| {
        ui.label("Tiling");
        ui.add(DragValue::new(&mut image.tiling[0]).speed(0.01).prefix("u ").max_decimals(3));
        ui.add(DragValue::new(&mut image.tiling[1]).speed(0.01).prefix("v ").max_decimals(3));
        ui.toggle_value(&mut locked, "Lock").on_hover_text("Keep the ratio of u and v");
    });
    ui.data_mut(|d| d.insert_temp(id, locked));
    if locked {
        let ratio = |a: f32, b: f32| if a.abs() > 1e-6 { b / a } else { 1.0 };
        if image.tiling[0] != before[0] {
            image.tiling[1] = before[1] * ratio(before[0], image.tiling[0]);
        } else if image.tiling[1] != before[1] {
            image.tiling[0] = before[0] * ratio(before[1], image.tiling[1]);
        }
    }
}

fn physical(ui: &mut Ui, image: &mut ImagePlacement) {
    let mut size = image.physical_size.unwrap_or([100.0, 100.0]);
    let custom = image.size_mode == SizeMode::Custom;
    ui.add_enabled_ui(custom, |ui| {
        ui.horizontal(|ui| {
            ui.label("Size");
            ui.add(DragValue::new(&mut size[0]).speed(0.5).range(0.01..=f32::MAX).suffix(" cm"));
            ui.add(DragValue::new(&mut size[1]).speed(0.5).range(0.01..=f32::MAX).suffix(" cm"));
        });
    });
    if custom || image.physical_size.is_some() {
        image.physical_size = Some(size);
    }
    ui.weak("Physical and custom sizes are kept but not evaluated yet: the fill is shown with its tiling.");
}

fn filter(ui: &mut Ui, p: &mut Projection) {
    ui.horizontal(|ui| {
        ui.label("Filter");
        for f in Filter::ALL {
            let label = match f {
                Filter::Smooth => "Smooth",
                Filter::Bilinear => "Bilinear",
                Filter::Nearest => "Nearest",
            };
            ui.selectable_value(&mut p.filter, f, label);
        }
    });
}

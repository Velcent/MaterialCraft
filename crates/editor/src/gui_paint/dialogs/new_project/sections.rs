//! The New project dialog's sections, one function each.

use super::NewProjectDialog;
use eframe::egui::{self, Ui};
use materialcraft::mesh::LengthUnit;
use materialcraft::paint::NormalMapFormat;
use materialcraft::paint::new_project::baked_names::MAP_NAMES;
use materialcraft::paint::new_project::{MeshFormat, RESOLUTIONS, UnitScale};
use std::path::PathBuf;

const GRID_SPACING: [f32; 2] = [12.0, 8.0];

fn section(ui: &mut Ui, title: &str, open: bool, body: impl FnOnce(&mut Ui)) {
    ui.add_space(4.0);
    egui::CollapsingHeader::new(egui::RichText::new(title).strong()).default_open(open).show(ui, body);
}

/// Mesh file, template and texture size.
pub fn basics(ui: &mut Ui, d: &mut NewProjectDialog) {
    egui::Grid::new("new_project_basics").num_columns(2).spacing(GRID_SPACING).show(ui, |ui| {
        ui.label("Mesh file");
        ui.horizontal(|ui| {
            let shown = if d.settings.mesh.as_os_str().is_empty() { "(none)".to_string() } else { d.settings.mesh.display().to_string() };
            ui.add(egui::Label::new(shown).truncate()).on_hover_text(readable_list());
            if ui.button("Choose…").clicked()
                && let Some(p) = pick_mesh()
            {
                d.settings.mesh = p;
            }
        });
        ui.end_row();
        ui.label("Template");
        let current = d.templates.get(d.template).map_or("", |t| t.template.name.as_str()).to_owned();
        let mut picked = d.template;
        egui::ComboBox::from_id_salt("new_project_template").selected_text(current).width(300.0).show_ui(ui, |ui| {
            for (i, t) in d.templates.iter().enumerate() {
                let hover = t.file.as_ref().map_or_else(|| "Built in".to_string(), |f| f.display().to_string());
                ui.selectable_value(&mut picked, i, &t.template.name).on_hover_text(hover);
            }
        });
        if picked != d.template
            && let Some(t) = d.templates.get(picked)
        {
            d.template = picked;
            d.settings.use_template(t.template.clone());
        }
        ui.end_row();
        ui.label("Texture size");
        egui::ComboBox::from_id_salt("new_project_resolution")
            .selected_text(size_label(d.settings.resolution))
            .show_ui(ui, |ui| {
                for r in RESOLUTIONS {
                    ui.selectable_value(&mut d.settings.resolution, r, size_label(r));
                }
            })
            .response
            .on_hover_text("The size of every texture set's layers. Painting goes up to 4096; exporting larger maps comes with the export step.");
        ui.end_row();
    });
}

fn size_label(r: u32) -> String {
    format!("{r} × {r}")
}

fn readable_list() -> String {
    let list: Vec<String> = MeshFormat::readable().map(|f| format!("{} (.{})", f.label(), f.extensions().join(", ."))).collect();
    format!("Readable: {}", list.join(", "))
}

/// A mesh file: the readable formats first, then the ones that are recognised but not read yet (so choosing one says
/// why it can't be used).
fn pick_mesh() -> Option<PathBuf> {
    let readable: Vec<&str> = MeshFormat::readable().flat_map(|f| f.extensions().iter().copied()).collect();
    let others: Vec<&str> = MeshFormat::ALL.into_iter().filter(|f| !f.is_readable()).flat_map(|f| f.extensions().iter().copied()).collect();
    rfd::FileDialog::new().add_filter("Mesh files MaterialCraft reads", &readable).add_filter("Other mesh files (not read yet)", &others).pick_file()
}

/// Scope and variants, subdivision level and frame: USD files only, and not read yet.
pub fn usd(ui: &mut Ui, d: &NewProjectDialog) {
    let is_usd = d.format().is_some_and(MeshFormat::is_usd);
    section(ui, "USD files", is_usd, |ui| {
        ui.weak("These settings apply to USD files only. USD files can't be read yet, so they stay greyed out.");
        ui.add_enabled_ui(false, |ui| {
            egui::Grid::new("new_project_usd").num_columns(2).spacing(GRID_SPACING).show(ui, |ui| {
                ui.label("Part of the file");
                ui.horizontal(|ui| {
                    ui.label("Root");
                    let _ = ui.button("Change…");
                });
                ui.end_row();
                ui.label("Subdivision level");
                let mut level = 0u32;
                ui.add(egui::DragValue::new(&mut level));
                ui.end_row();
                ui.label("Animation frame");
                let mut frame = 0u32;
                ui.add(egui::DragValue::new(&mut frame));
                ui.end_row();
            });
        });
    });
}

/// Normal map format and where the bitangent is built.
pub fn normals(ui: &mut Ui, d: &mut NewProjectDialog) {
    section(ui, "Normal maps", true, |ui| {
        ui.horizontal(|ui| {
            ui.label("Format");
            for f in NormalMapFormat::ALL {
                ui.radio_value(&mut d.settings.normal_map_format, f, f.label());
            }
        })
        .response
        .on_hover_text(
            "How exported normal maps and baked maps read from files store green. Engines differ: DirectX-style engines expect Y-, OpenGL-style ones Y+.",
        );
        ui.checkbox(&mut d.settings.tangent_per_fragment, "Build bitangents per pixel").on_hover_text(
            "Decode normal maps with bitangents computed per pixel instead of per vertex, to match engines that do so. \
             Normal maps baked with one choice look wrong with the other, so changing it later means baking them again.",
        );
    });
}

/// UV tiles (UDIMs): not supported yet.
pub fn uv_tiles(ui: &mut Ui) {
    section(ui, "UV tiles", false, |ui| {
        ui.weak("UV tiles (UDIMs) aren't supported yet: textures cover the 0–1 UV square only.");
        ui.add_enabled_ui(false, |ui| {
            let mut off = false;
            ui.checkbox(&mut off, "Paint across UV tiles outside 0–1");
            ui.checkbox(&mut off, "One texture set per material, showing all its tiles");
            ui.checkbox(&mut off, "One texture set per tile");
        });
    });
}

/// Cameras, generated UVs and baked map files.
pub fn imports(ui: &mut Ui, d: &mut NewProjectDialog) {
    let cameras = d.format().is_some_and(MeshFormat::has_cameras);
    section(ui, "Import", true, |ui| {
        ui.add_enabled_ui(cameras, |ui| {
            ui.checkbox(&mut d.settings.import_cameras, "Add the file's cameras to the camera list");
        })
        .response
        .on_disabled_hover_text("Only glTF binary files bring cameras.");
        ui.add_enabled_ui(false, |ui| {
            let mut off = false;
            ui.checkbox(&mut off, "Generate UVs where the mesh has none");
        })
        .response
        .on_disabled_hover_text("Not built yet: a mesh without UVs is refused, naming its materials.");
        ui.add_space(4.0);
        ui.label("Baked maps from files");
        let names: Vec<&str> = MAP_NAMES.iter().map(|(n, _)| *n).collect();
        ui.weak(format!("Name each file <texture set>_<map>, e.g. Body_normal.png; <map> is one of {}. PNG or OpenEXR.", names.join(", ")));
        let mut remove = None;
        for (i, f) in d.settings.baked_maps.iter().enumerate() {
            ui.horizontal(|ui| {
                if ui.small_button("✕").on_hover_text("Leave this file out").clicked() {
                    remove = Some(i);
                }
                ui.label(f.file_name().map_or_else(|| f.display().to_string(), |n| n.to_string_lossy().into_owned())).on_hover_text(f.display().to_string());
            });
        }
        if let Some(i) = remove {
            d.settings.baked_maps.remove(i);
        }
        if ui.button("Add files…").clicked()
            && let Some(files) = rfd::FileDialog::new().add_filter("PNG or OpenEXR image", &["png", "exr"]).pick_files()
        {
            for f in files {
                if !d.settings.baked_maps.contains(&f) {
                    d.settings.baked_maps.push(f);
                }
            }
        }
    });
}

/// The unit one mesh unit measures, and physically sized fills.
pub fn physical_size(ui: &mut Ui, d: &mut NewProjectDialog) {
    let file_unit = d.format().and_then(MeshFormat::unit);
    section(ui, "Physical size", true, |ui| {
        let from_file = match file_unit {
            Some(u) => format!("The file's unit ({})", unit_label(u)),
            None => "The file's unit (OBJ files state none: set it later)".to_string(),
        };
        ui.radio_value(&mut d.settings.unit, UnitScale::FromFile, from_file);
        ui.horizontal(|ui| {
            let custom = matches!(d.settings.unit, UnitScale::Custom(_));
            if ui.radio(custom, "One mesh unit is").clicked() {
                d.settings.unit = UnitScale::Custom(d.custom_unit);
            }
            egui::ComboBox::from_id_salt("new_project_unit").selected_text(unit_label(d.custom_unit)).show_ui(ui, |ui| {
                for (u, label) in LengthUnit::ALL {
                    if ui.selectable_value(&mut d.custom_unit, u, label).clicked() {
                        d.settings.unit = UnitScale::Custom(u);
                    }
                }
            });
        });
        ui.checkbox(&mut d.settings.fill_physical_size, "Size dropped materials by physical size instead of UV tiling");
    });
}

fn unit_label(u: LengthUnit) -> &'static str {
    LengthUnit::ALL.iter().find(|(v, _)| *v == u).map_or("?", |(_, l)| *l)
}

/// Colour management: one mode for now.
pub fn colour(ui: &mut Ui) {
    section(ui, "Colour", false, |ui| {
        ui.weak("Colour textures are sRGB and layers blend in linear light, the only mode for now.");
    });
}

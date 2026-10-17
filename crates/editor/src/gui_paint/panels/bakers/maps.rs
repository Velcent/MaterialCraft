//! One row per bakeable map (BK2-01): include check, name, show-in-viewport, bake-only-this, auto-rebake and link
//! toggles, and below the open row its baker's settings.

use super::BakerEvent;
use super::fields::{Fields, num};
use eframe::egui::{self, Ui};
use materialcraft::bake::{
    AoSettings, Attenuation, CurvatureRange, CurvatureSettings, MapKind, MapSelection, PositionBounds, PositionMode, PositionSettings, RayDistribution,
    TangentSource, ThicknessSettings,
};
use materialcraft::paint::bake_config::{BakeLinks, BakePart, MapEntry, MapFlags, MapSettings};

/// What the rows of one set need.
pub struct MapRows<'a> {
    pub set: &'a str,
    pub links: BakeLinks,
    /// Link and copy controls only make sense with several sets (BK2-09).
    pub several_sets: bool,
    /// Every set id, for "copy to all".
    pub all_sets: &'a [String],
    pub busy: bool,
}

pub fn label(kind: MapKind) -> &'static str {
    match kind {
        MapKind::Normal => "Normal",
        MapKind::WorldNormal => "World space normal",
        MapKind::Position => "Position",
        MapKind::AmbientOcclusion => "Ambient occlusion",
        MapKind::Curvature => "Curvature",
        MapKind::Thickness => "Thickness",
        MapKind::Id => "ID",
    }
}

impl MapRows<'_> {
    /// The row of `entry`; returns true if its name was clicked (open or close its settings).
    pub fn row(&self, ui: &mut Ui, entry: &MapEntry, open: bool, f: &mut Fields) -> bool {
        let kind = entry.kind();
        let flags = entry.flags;
        let set = self.set.to_string();
        let mut toggled = false;
        ui.horizontal(|ui| {
            let mut include = flags.include;
            if ui.checkbox(&mut include, "").on_hover_text("Include in the bake").changed() {
                f.events.push(BakerEvent::Flags { set: set.clone(), kind, flags: MapFlags { include, ..flags } });
            }
            let arrow = if open { "⏷" } else { "⏵" };
            if ui.selectable_label(open, format!("{arrow} {}", label(kind))).clicked() {
                toggled = true;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.several_sets {
                    let linked = self.links.is_linked(BakePart::Map(kind));
                    let r = ui.selectable_label(linked, "Link").on_hover_text("Linked: a change of these settings is made in every texture set");
                    if r.clicked() {
                        f.events.push(BakerEvent::Link { set: set.clone(), part: BakePart::Map(kind), on: !linked });
                    }
                    r.context_menu(|ui| {
                        if ui.button("Copy these settings to every set").clicked() {
                            f.events.push(BakerEvent::CopyPart { from: set.clone(), part: BakePart::Map(kind), to: self.all_sets.to_vec() });
                            ui.close();
                        }
                    });
                }
                if ui.selectable_label(flags.auto_rebake, "Auto").on_hover_text("Rebake this map when its settings change").clicked() {
                    f.events.push(BakerEvent::Flags { set: set.clone(), kind, flags: MapFlags { auto_rebake: !flags.auto_rebake, ..flags } });
                }
                if ui.add_enabled(!self.busy, egui::Button::new("Bake").small()).on_hover_text("Bake only this map").clicked() {
                    f.events.push(BakerEvent::Bake { sets: vec![set.clone()], maps: Some(MapSelection::of(&[kind])) });
                }
                if ui.selectable_label(flags.show, "Show").on_hover_text("Show this map in the viewport").clicked() {
                    f.events.push(BakerEvent::Show { set: set.clone(), kind: (!flags.show).then_some(kind) });
                }
            });
        });
        toggled
    }

    /// The settings of `entry`'s baker.
    pub fn settings(&self, ui: &mut Ui, entry: &MapEntry, f: &mut Fields) {
        let name = entry.kind().name();
        let field = |s: &str| format!("{name}.{s}");
        let set = self.set.to_string();
        let emit = |f: &mut Fields, settings: MapSettings, key| f.events.push(BakerEvent::Settings { set: set.clone(), settings, key });
        let dist = [(RayDistribution::Uniform, "Uniform"), (RayDistribution::Cosine, "Cosine")];
        match *entry.settings() {
            MapSettings::Normal { tangents } => {
                let options = [(TangentSource::Mesh, "From the mesh"), (TangentSource::Generated, "Generated")];
                if let Some(t) = f.choice(ui, &field("tangents"), "Tangents", tangents, &options) {
                    emit(f, MapSettings::Normal { tangents: t }, None);
                }
            }
            MapSettings::WorldNormal | MapSettings::Id => {
                ui.weak("No settings of its own.");
            }
            MapSettings::Position(p) => {
                let modes = [(PositionMode::Uniform, "All axes, uniform"), (PositionMode::PerAxis, "Each axis stretched")];
                if let Some(mode) = f.choice(ui, &field("mode"), "Normalisation", p.mode, &modes) {
                    emit(f, MapSettings::Position(PositionSettings { mode, ..p }), None);
                }
                let bounds = [(PositionBounds::Scene, "Whole mesh"), (PositionBounds::Set, "This texture set"), (PositionBounds::High, "High-poly mesh")];
                if let Some(b) = f.choice(ui, &field("bounds"), "Bounding box", p.bounds, &bounds) {
                    emit(f, MapSettings::Position(PositionSettings { bounds: b, ..p }), None);
                }
            }
            MapSettings::AmbientOcclusion { rays, distance, ao } => {
                let with = |rays, distance, ao| MapSettings::AmbientOcclusion { rays, distance, ao };
                if let Some((v, k)) = f.count(ui, &field("rays"), "Secondary rays", rays, 1..=1024, 64) {
                    emit(f, with(v, distance, ao), Some(k));
                }
                if let Some((v, k)) = f.slider(ui, &field("distance"), "Max occluder distance", distance, num(0.0..=2.0, 0.01, 0.5)) {
                    emit(f, with(rays, v, ao), Some(k));
                }
                if let Some((v, k)) = f.slider(ui, &field("min_dist"), "Min occluder distance", ao.min_dist, num(0.0..=0.1, 0.0001, 0.00001)) {
                    emit(f, with(rays, distance, AoSettings { min_dist: v, ..ao }), Some(k));
                }
                if let Some((v, k)) = f.slider(ui, &field("spread"), "Spread angle", ao.spread, num(0.0..=180.0, 1.0, 180.0)) {
                    emit(f, with(rays, distance, AoSettings { spread: v, ..ao }), Some(k));
                }
                let att = [(Attenuation::None, "None"), (Attenuation::Smooth, "Smooth"), (Attenuation::Linear, "Linear")];
                if let Some(a) = f.choice(ui, &field("attenuation"), "Attenuation", ao.attenuation, &att) {
                    emit(f, with(rays, distance, AoSettings { attenuation: a, ..ao }), None);
                }
                if let Some(d) = f.choice(ui, &field("distribution"), "Distribution", ao.distribution, &dist) {
                    emit(f, with(rays, distance, AoSettings { distribution: d, ..ao }), None);
                }
            }
            MapSettings::Curvature(c) => {
                if let Some((v, k)) = f.slider(ui, &field("radius"), "Sampling radius", c.radius, num(0.0001..=0.1, 0.0001, 0.001)) {
                    emit(f, MapSettings::Curvature(CurvatureSettings { radius: v, ..c }), Some(k));
                }
                if let Some((v, k)) = f.count(ui, &field("rays"), "Samples", c.rays, 4..=256, 32) {
                    emit(f, MapSettings::Curvature(CurvatureSettings { rays: v, ..c }), Some(k));
                }
                let auto = matches!(c.range, CurvatureRange::Auto);
                if let Some(on) = f.check(ui, &field("range"), "Automatic range", auto) {
                    let range = if on { CurvatureRange::Auto } else { CurvatureRange::Fixed(100.0) };
                    emit(f, MapSettings::Curvature(CurvatureSettings { range, ..c }), None);
                }
                if let CurvatureRange::Fixed(r) = c.range
                    && let Some((v, k)) = f.slider(ui, &field("range"), "Range", r, num(1.0..=1000.0, 1.0, 100.0))
                {
                    emit(f, MapSettings::Curvature(CurvatureSettings { range: CurvatureRange::Fixed(v), ..c }), Some(k));
                }
            }
            MapSettings::Thickness(t) => {
                if let Some((v, k)) = f.count(ui, &field("rays"), "Rays", t.rays, 1..=1024, 64) {
                    emit(f, MapSettings::Thickness(ThicknessSettings { rays: v, ..t }), Some(k));
                }
                if let Some((v, k)) = f.slider(ui, &field("max_dist"), "Max distance", t.max_dist, num(0.0..=1.0, 0.001, 0.1)) {
                    emit(f, MapSettings::Thickness(ThicknessSettings { max_dist: v, ..t }), Some(k));
                }
                if let Some(d) = f.choice(ui, &field("distribution"), "Distribution", t.distribution, &dist) {
                    emit(f, MapSettings::Thickness(ThicknessSettings { distribution: d, ..t }), None);
                }
                if let Some(m) = f.check(ui, &field("maximise_range"), "Stretch to the full range", t.maximise_range) {
                    emit(f, MapSettings::Thickness(ThicknessSettings { maximise_range: m, ..t }), None);
                }
            }
        }
        ui.add_space(4.0);
    }
}

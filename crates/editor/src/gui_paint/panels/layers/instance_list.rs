//! The instance list (IN-10): for a focused source or instance, the source first and its instances below, each as
//! "texture set › layer". Clicking an entry selects that layer and switches the panel to its texture set.

use crate::gui_theme;
use eframe::egui::{self, Ui};
use materialcraft::paint::edit::LayerRef;
use materialcraft::paint::instance::{InstanceIndex, InstanceLink};
use materialcraft::paint::{Document, LayerKind};

/// One line of the list.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub r: LayerRef,
    pub label: String,
    pub source: bool,
    pub broken: bool,
}

/// The source of `focus` and its instances; empty when `focus` is neither (or a broken instance without a source).
pub fn entries(doc: &Document, index: &InstanceIndex, focus: &LayerRef) -> Vec<Entry> {
    let Some(layer) = doc.texture_set(&focus.set).and_then(|s| s.layer(&focus.layer)) else { return Vec::new() };
    let source = match &layer.kind {
        LayerKind::Instance(link) => match doc.instance_source(link) {
            Some(s) => s,
            None => return Vec::new(),
        },
        _ => InstanceLink::new(focus.set.clone(), focus.layer.clone()),
    };
    let label = |set: &str, layer: &str| {
        let s = doc.texture_set(set);
        let set_name = s.map_or(set, |s| s.name.as_str());
        let layer_name = s.and_then(|s| s.layer(layer)).map_or(layer, |l| l.name.as_str());
        format!("{set_name} › {layer_name}")
    };
    let instances: Vec<Entry> = index
        .instances_of(&source.set, &source.layer)
        .map(|e| Entry { r: LayerRef::new(e.set.clone(), e.layer.clone()), label: label(&e.set, &e.layer), source: false, broken: e.broken.is_some() })
        .collect();
    if instances.is_empty() {
        return Vec::new();
    }
    let head = Entry { r: LayerRef::new(source.set.clone(), source.layer.clone()), label: label(&source.set, &source.layer), source: true, broken: false };
    std::iter::once(head).chain(instances).collect()
}

/// Draws the list (collapsed by default); returns the entry clicked.
pub fn show(ui: &mut Ui, entries: &[Entry], current: &LayerRef) -> Option<LayerRef> {
    let mut clicked = None;
    let title = format!("Instances ({})", entries.len().saturating_sub(1));
    egui::CollapsingHeader::new(title).id_salt("layers_instance_list").show(ui, |ui| {
        for e in entries {
            let mark = if e.source { "◆ source: " } else { "" };
            let text = egui::RichText::new(format!("{mark}{}", e.label));
            let text = if e.broken {
                text.color(egui::Color32::from_rgb(220, 70, 60))
            } else if e.source {
                text.color(gui_theme::ACCENT)
            } else {
                text
            };
            let tip = if e.broken { "Broken: shows nothing. Click to select it." } else { "Click to select it and show its texture set" };
            if ui.selectable_label(e.r == *current, text).on_hover_text(tip).clicked() {
                clicked = Some(e.r.clone());
            }
        }
    });
    clicked
}

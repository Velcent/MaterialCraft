//! Paint-model findings as session log entries (LG-01, LG-10, LG-12): the import report's summary
//! line and compositing warnings, each pointing at what it concerns. Texts are brand-neutral
//! (LG-09): the report's source names the application it came from, so it is left out here.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use super::report::{Fidelity, ImportReport};
use super::stack::CompositeWarning;
use super::texture_set::TextureSet;
use crate::diag::{Area, Entry, Level, Target};

/// The summary line of an import: what was carried over how faithfully. `file` is the imported
/// project, `detail` the importer's own summary (timing, caching), if any. A warning when
/// something was kept raw (not translated at all), else info.
pub fn import_summary(report: &ImportReport, file: &str, detail: Option<&str>) -> Entry {
    let name = file.rsplit(['/', '\\']).next().unwrap_or(file);
    let mut text = format!("Imported {name}: {} items", report.entries.len());
    let counts: Vec<String> = [Fidelity::Exact, Fidelity::Approximated, Fidelity::Baked, Fidelity::KeptRaw]
        .into_iter()
        .filter_map(|f| {
            let n = report.count(f);
            (n > 0).then(|| format!("{n} {}", fidelity_word(f)))
        })
        .collect();
    if !counts.is_empty() {
        text.push_str(&format!(" ({})", counts.join(", ")));
    }
    if let Some(d) = detail.map(str::trim).filter(|d| !d.is_empty()) {
        text.push_str(&format!("; {d}"));
    }
    let level = if report.count(Fidelity::KeptRaw) > 0 { Level::Warning } else { Level::Info };
    Entry::new(level, Area::Import, text).with_target(Target::File(file.to_owned()))
}

fn fidelity_word(f: Fidelity) -> &'static str {
    match f {
        Fidelity::Exact => "exact",
        Fidelity::Approximated => "approximated",
        Fidelity::Baked => "baked",
        Fidelity::KeptRaw => "kept raw",
    }
}

/// One warning entry per compositing warning of `set`, pointing at the layer concerned.
pub fn composite_warnings(set: &TextureSet, warnings: &[CompositeWarning]) -> Vec<Entry> {
    warnings.iter().map(|w| composite_warning(set, w)).collect()
}

/// The entry for one compositing warning of `set`.
pub fn composite_warning(set: &TextureSet, warning: &CompositeWarning) -> Entry {
    let layer_id = layer_of(warning);
    let layer = set.layer(layer_id).map_or(layer_id, |l| l.name.as_str());
    let what = match warning {
        CompositeWarning::MaterialFillSkipped { .. } => "its material fill can't be computed yet, so the layer is left out".to_owned(),
        CompositeWarning::GroupTooDeep { .. } => "the group is nested too deeply and is left out".to_owned(),
        CompositeWarning::ImageSizeMismatch { .. } => "an image or mask doesn't match the texture set's resolution and is read as is".to_owned(),
        CompositeWarning::EffectNotComputable { in_mask, index, kind, .. } => {
            let stack = if *in_mask { "mask" } else { "content" };
            format!("{stack} effect {} ({kind}) can't be computed yet and is left out", index.saturating_add(1))
        }
        CompositeWarning::GroupEffectsSkipped { .. } => "effects on a pass-through group are not applied".to_owned(),
        CompositeWarning::GeometryMaskNeedsMesh { .. } => "its geometry mask needs the mesh and is not applied".to_owned(),
        CompositeWarning::UvTileMaskNotComputable { .. } => "its geometry mask lists UV tiles, which are not supported yet, and is not applied".to_owned(),
        CompositeWarning::FillNeedsMesh { effect, mode, .. } => {
            format!("{} uses a {mode} projection, which needs the mesh, and is left out", fill_name(*effect))
        }
        CompositeWarning::ProjectionNotComputable { effect, reason, .. } => format!("{} is left out: {reason}", fill_name(*effect)),
    };
    let text = format!("{} / {layer}: {what}.", set.name);
    Entry::new(Level::Warning, Area::Checks, text).with_target(Target::Layer { texture_set: set.id.clone(), layer: layer_id.to_owned() })
}

fn layer_of(w: &CompositeWarning) -> &str {
    match w {
        CompositeWarning::MaterialFillSkipped { layer }
        | CompositeWarning::GroupTooDeep { layer }
        | CompositeWarning::ImageSizeMismatch { layer }
        | CompositeWarning::EffectNotComputable { layer, .. }
        | CompositeWarning::GroupEffectsSkipped { layer }
        | CompositeWarning::GeometryMaskNeedsMesh { layer }
        | CompositeWarning::UvTileMaskNotComputable { layer }
        | CompositeWarning::FillNeedsMesh { layer, .. }
        | CompositeWarning::ProjectionNotComputable { layer, .. } => layer,
    }
}

/// "its fill" or "content effect n's fill".
fn fill_name(effect: Option<usize>) -> String {
    match effect {
        None => "its fill".to_owned(),
        Some(i) => format!("the fill of content effect {}", i.saturating_add(1)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::Tab;

    #[test]
    fn the_import_summary_counts_fidelities_and_points_at_the_file() {
        let mut r = ImportReport::new("some-app 1.0: robot.spp");
        r.add("a", Fidelity::Exact, "");
        r.add("b", Fidelity::Exact, "");
        r.add("c", Fidelity::Baked, "");
        let e = import_summary(&r, "/p/robot.spp", Some("imported in 2.0 s"));
        assert_eq!(e.text, "Imported robot.spp: 3 items (2 exact, 1 baked); imported in 2.0 s");
        assert_eq!((e.level, e.area.tab()), (Level::Info, Tab::Assets));
        assert_eq!(e.target, Some(Target::File("/p/robot.spp".into())));
        assert!(!e.text.contains("some-app"), "the source's application name stays out of the log");
        r.add("d", Fidelity::KeptRaw, "");
        assert_eq!(import_summary(&r, "x.spp", None).level, Level::Warning);
    }

    #[test]
    fn composite_warnings_name_the_layer_and_point_at_it() {
        use crate::paint::{Layer, LayerKind, PaintContent};
        let mut set = TextureSet::new("ts1", "Body", (4, 4));
        set.layers.push(Layer::new("l7", "Rust", LayerKind::Paint(PaintContent::default())));
        let w = [
            CompositeWarning::EffectNotComputable { layer: "l7".into(), in_mask: true, index: 0, kind: "hologram".into() },
            CompositeWarning::GeometryMaskNeedsMesh { layer: "gone".into() },
        ];
        let es = composite_warnings(&set, &w);
        assert_eq!(es[0].text, "Body / Rust: mask effect 1 (hologram) can't be computed yet and is left out.");
        assert_eq!(es[0].target, Some(Target::Layer { texture_set: "ts1".into(), layer: "l7".into() }));
        assert_eq!((es[0].level, es[0].area), (Level::Warning, Area::Checks));
        assert!(es[1].text.starts_with("Body / gone: "), "an unknown layer is named by id");
    }
}

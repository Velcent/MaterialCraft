//! Turns the bakers panel's configuration events into history steps on the open document (each one
//! `paint::bake_config::ops` operation inside [`OpenDocument::edit`]), and says which maps an edit asks to rebake
//! (their auto-rebake flag is on and their settings changed, BK2-07).

use super::document_state::OpenDocument;
use super::panels::bakers::BakerEvent;
use materialcraft::bake::{MapKind, MapSelection};
use materialcraft::paint::Document;
use materialcraft::paint::bake_config::{BakePart, SetBakeConfig, ops};
use materialcraft::paint::edit::EditError;
use std::collections::BTreeMap;

/// What an applied event changed beyond the configuration.
#[derive(Debug, Default, PartialEq)]
pub struct Applied {
    /// Maps to rebake per set id.
    pub rebake: BTreeMap<String, MapSelection>,
    /// The map the viewport should show now (`Some(None)`: back to the material).
    pub show: Option<Option<MapKind>>,
}

/// Applies a configuration event as one history step. Bake, cancel and gesture ends are not handled here.
pub fn apply(doc: &mut OpenDocument, ev: BakerEvent) -> Result<Applied, EditError> {
    use BakerEvent::*;
    let before = configs(doc.doc());
    let mut applied = Applied::default();
    match ev {
        Common { set, common, key } => doc.edit("Bake settings", key, |tx| ops::set_common(tx, &set, &common))?,
        Settings { set, settings, key } => doc.edit("Baker settings", key, |tx| ops::set_map_settings(tx, &set, &settings))?,
        Flags { set, kind, flags } => doc.edit("Baker options", None, |tx| ops::set_flags(tx, &set, kind, flags))?,
        Show { set, kind } => {
            doc.edit("Show mesh map", None, |tx| ops::show_map(tx, &set, kind))?;
            applied.show = Some(kind);
        }
        Link { set, part, on } => doc.edit(if on { "Link bake settings" } else { "Unlink bake settings" }, None, |tx| ops::set_link(tx, &set, part, on))?,
        CopyPart { from, part, to } => doc.edit("Copy bake settings", None, |tx| ops::copy_part(tx, &from, part, &to))?,
        CopyAll { from, to } => doc.edit("Copy bake settings", None, |tx| ops::copy_all(tx, &from, &to))?,
        CheckAll { set, on } => doc.edit(if on { "Check all bakers" } else { "Uncheck all bakers" }, None, |tx| ops::check_all(tx, &set, on))?,
        InvertChecks { set } => doc.edit("Invert baker checks", None, |tx| ops::invert_checks(tx, &set))?,
        CopyChecks { from, to } => doc.edit("Copy baker checks", None, |tx| ops::copy_checks(tx, &from, &to))?,
        EndGesture => doc.close_step(),
        Bake { .. } | Cancel => {}
    }
    applied.rebake = rebake(&before, &configs(doc.doc()));
    Ok(applied)
}

fn configs(doc: &Document) -> BTreeMap<String, SetBakeConfig> {
    doc.texture_sets.iter().map(|s| (s.id.clone(), s.bake.clone())).collect()
}

/// The auto-rebake maps whose settings (or the set's common settings) differ between `before` and `after`.
fn rebake(before: &BTreeMap<String, SetBakeConfig>, after: &BTreeMap<String, SetBakeConfig>) -> BTreeMap<String, MapSelection> {
    let mut out = BTreeMap::new();
    for (id, new) in after {
        let Some(old) = before.get(id) else { continue };
        let changed = |part: BakePart| match part {
            BakePart::Common => old.common != new.common,
            BakePart::Map(k) => old.maps.get(k).settings() != new.maps.get(k).settings(),
        };
        let maps = MapKind::ALL
            .into_iter()
            .filter(|k| new.maps.get(*k).flags.auto_rebake && (changed(BakePart::Common) || changed(BakePart::Map(*k))))
            .fold(MapSelection::none(), MapSelection::with);
        if !maps.is_empty() {
            out.insert(id.clone(), maps);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft::bake::ThicknessSettings;
    use materialcraft::paint::TextureSet;
    use materialcraft::paint::bake_config::{MapFlags, MapSettings};

    fn open() -> OpenDocument {
        let mut doc = Document::default();
        doc.texture_sets.push(TextureSet::new("a", "A", (64, 64)));
        doc.texture_sets.push(TextureSet::new("b", "B", (64, 64)));
        OpenDocument::new(doc, "x.mcpaint".into(), None)
    }

    #[test]
    fn settings_changes_are_steps_and_ask_for_auto_rebakes() {
        let mut d = open();
        let thick = |rays| MapSettings::Thickness(ThicknessSettings { rays, ..ThicknessSettings::default() });
        let auto = MapFlags { auto_rebake: true, ..MapFlags::default() };
        apply(&mut d, BakerEvent::Flags { set: "b".into(), kind: MapKind::Thickness, flags: auto }).unwrap();
        apply(&mut d, BakerEvent::Link { set: "a".into(), part: BakePart::Map(MapKind::Thickness), on: true }).unwrap();
        let a = apply(&mut d, BakerEvent::Settings { set: "a".into(), settings: thick(5), key: None }).unwrap();
        assert_eq!(a.rebake.keys().collect::<Vec<_>>(), ["b"], "linked into b, whose thickness rebakes itself");
        assert_eq!(a.rebake["b"], MapSelection::of(&[MapKind::Thickness]));
        assert_eq!(d.history().len(), 3);
        let s = apply(&mut d, BakerEvent::Show { set: "a".into(), kind: Some(MapKind::Curvature) }).unwrap();
        assert_eq!(s.show, Some(Some(MapKind::Curvature)));
        d.undo().unwrap();
        d.undo().unwrap();
        assert_ne!(d.doc().texture_sets[1].bake.maps.get(MapKind::Thickness).settings(), &thick(5));
    }
}

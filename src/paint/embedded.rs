//! Assets a document embeds so it opens on a machine without the library (AS-21, the "project" scope): the
//! material graphs its fills name and the smart-material fragments its folders came from, each stored under the
//! library reference it is used by (`materials/rust.mca`, `smart-materials/worn-paint.mcfrag`).
//!
//! [`references`] lists what a layer stack uses; [`refresh`] brings a list of embedded assets in line with it,
//! reading missing ones through a lookup the caller provides (the paint model does no I/O).

use super::doc::Document;
use super::effect::EffectKind;
use super::layer::{FillSource, Layer, LayerKind, LayerWalk};
use std::collections::BTreeSet;

/// The bytes of a library asset, kept under the reference the document uses for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedAsset {
    /// A library-relative path: `/`-separated, no empty, `.` or `..` segments, no `\` or `:`.
    pub reference: String,
    pub bytes: Vec<u8>,
}

impl EmbeddedAsset {
    pub fn new(reference: impl Into<String>, bytes: Vec<u8>) -> EmbeddedAsset {
        EmbeddedAsset { reference: reference.into(), bytes }
    }
}

/// True if `reference` can be embedded: relative, `/`-separated, without empty, `.` or `..` segments, `\` or
/// `:` (so it is also a safe archive entry name). Absolute paths are used where they are and never embedded.
pub fn is_embeddable(reference: &str) -> bool {
    !reference.is_empty() && !reference.contains(['\\', ':']) && reference.split('/').all(|s| !s.is_empty() && s != "." && s != "..")
}

/// Every embeddable asset reference `layers` use (children included): material fills of layers and content
/// effects, and the fragments folders came from.
pub fn references(layers: &[Layer]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for (_, l) in LayerWalk::new(layers) {
        if let Some(a) = &l.asset {
            out.insert(a.clone());
        }
        if let LayerKind::Fill(f) = &l.kind
            && let FillSource::Material { path, .. } = &f.source
        {
            out.insert(path.clone());
        }
        for e in &l.effects {
            if let EffectKind::Fill(f) = &e.kind
                && let FillSource::Material { path, .. } = &f.source
            {
                out.insert(path.clone());
            }
        }
    }
    out.retain(|r| is_embeddable(r));
    out
}

/// Every embeddable asset reference of every texture set of `doc`.
pub fn document_references(doc: &Document) -> BTreeSet<String> {
    doc.texture_sets.iter().flat_map(|s| references(&s.layers)).collect()
}

/// What [`refresh`] changed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EmbedReport {
    /// References read through the lookup and embedded.
    pub added: Vec<String>,
    /// Embedded assets dropped because nothing uses them any more.
    pub removed: Vec<String>,
    /// Used references neither embedded nor found by the lookup.
    pub missing: Vec<String>,
}

impl EmbedReport {
    pub fn changed(&self) -> bool {
        !self.added.is_empty() || !self.removed.is_empty()
    }
}

/// Makes `assets` hold exactly the `used` references: ones already embedded stay as they are (the embedded copy
/// wins over a changed library file, so a document looks the same everywhere), missing ones are read with
/// `lookup`, unused ones are dropped. The result is sorted by reference.
pub fn refresh(assets: &mut Vec<EmbeddedAsset>, used: &BTreeSet<String>, mut lookup: impl FnMut(&str) -> Option<Vec<u8>>) -> EmbedReport {
    let mut report = EmbedReport::default();
    let old = std::mem::take(assets);
    for a in old {
        if used.contains(&a.reference) && !assets.iter().any(|b| b.reference == a.reference) {
            assets.push(a);
        } else {
            report.removed.push(a.reference);
        }
    }
    for r in used {
        if assets.iter().any(|a| &a.reference == r) {
            continue;
        }
        match lookup(r) {
            Some(bytes) => {
                assets.push(EmbeddedAsset::new(r.clone(), bytes));
                report.added.push(r.clone());
            }
            None => report.missing.push(r.clone()),
        }
    }
    assets.sort_by(|a, b| a.reference.cmp(&b.reference));
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::layer::{Fill, PaintContent};

    #[test]
    fn references_and_refresh() {
        assert!(is_embeddable("materials/rust.mca"));
        for bad in ["", "/abs/x.mca", "a//b", "../x", "a/./b", "C:/x", "a\\b"] {
            assert!(!is_embeddable(bad), "{bad}");
        }
        let fill = |p: &str| Layer::new(p, p, LayerKind::Fill(Fill::new(FillSource::Material { path: p.into(), params: vec![] })));
        let mut folder = Layer::new("g", "G", LayerKind::Group(vec![fill("materials/rust.mca"), fill("/abs/steel.mca")]));
        folder.asset = Some("smart-materials/worn.mcfrag".into());
        let layers = vec![folder, Layer::new("p", "P", LayerKind::Paint(PaintContent::default()))];
        let used = references(&layers);
        assert_eq!(used.iter().map(String::as_str).collect::<Vec<_>>(), ["materials/rust.mca", "smart-materials/worn.mcfrag"]);

        let mut assets = vec![EmbeddedAsset::new("old.mca", vec![1]), EmbeddedAsset::new("materials/rust.mca", vec![2])];
        let report = refresh(&mut assets, &used, |r| (r == "materials/rust.mca").then(|| vec![9]));
        assert_eq!(assets, [EmbeddedAsset::new("materials/rust.mca", vec![2])], "the embedded copy wins");
        assert_eq!(report, EmbedReport { added: vec![], removed: vec!["old.mca".into()], missing: vec!["smart-materials/worn.mcfrag".into()] });
        let report = refresh(&mut assets, &used, |_| Some(vec![7]));
        assert_eq!(report.added, ["smart-materials/worn.mcfrag"]);
        assert!(report.changed());
        assert_eq!(assets.len(), 2);
    }
}

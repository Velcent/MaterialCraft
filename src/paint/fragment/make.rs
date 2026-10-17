//! Making fragments from a document ("Save as smart material", "Save as smart mask"; MK-15): what a selection or a
//! mask becomes, and completing a fragment with the baked maps it needs and the assets it uses.

use super::{Fragment, needs};
use crate::paint::edit::ops::normalize;
use crate::paint::embedded::{self, EmbedReport};
use crate::paint::layer::LayerKind;
use crate::paint::texture_set::{BakedKind, TextureSet};

/// The smart material the selected layers `ids` of `set` make: one selected folder gives its children under its
/// name (the folder itself is what a drop makes again); otherwise the selected layers in stack order, named after
/// the topmost one. `None` if nothing known is selected.
pub fn smart_material(set: &TextureSet, ids: &[String]) -> Option<Fragment> {
    let chosen = normalize(set, ids);
    let layers: Vec<_> = chosen.iter().filter_map(|id| set.layer(id)).collect();
    match layers.as_slice() {
        [] => None,
        [folder] if matches!(folder.kind, LayerKind::Group(_)) => Some(Fragment::smart_material(folder.name.clone(), set, folder.children().to_vec())),
        [.., top] => {
            let name = if top.name.trim().is_empty() { "Smart material".to_string() } else { top.name.clone() };
            Some(Fragment::smart_material(name, set, layers.iter().map(|l| (*l).clone()).collect()))
        }
    }
}

/// The smart mask the mask of layer `id` of `set` makes, named after the layer. `None` without a mask.
pub fn smart_mask(set: &TextureSet, id: &str) -> Option<Fragment> {
    let layer = set.layer(id)?;
    let name = if layer.name.trim().is_empty() { "Smart mask".to_string() } else { format!("{} mask", layer.name) };
    Some(Fragment::smart_mask(name, set, layer.mask.clone()?))
}

impl Fragment {
    /// Declares the baked maps the fragment needs (see [`needs::required_maps`]) and embeds the library assets its
    /// layers use, read with `lookup`. Returns what could not be embedded among the rest.
    pub fn complete(&mut self, procedural_maps: &dyn Fn(&str) -> Vec<BakedKind>, lookup: impl FnMut(&str) -> Option<Vec<u8>>) -> EmbedReport {
        self.needs = needs::required_maps(&self.content, procedural_maps);
        let used = match &self.content {
            super::FragmentContent::Layers(layers) => embedded::references(layers),
            super::FragmentContent::Mask(_) => Default::default(),
        };
        embedded::refresh(&mut self.assets, &used, lookup)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::effect::{EffectKind, GreyBlend, MaskEffect, Procedural};
    use crate::paint::layer::{Fill, FillSource, Layer, PaintContent};
    use crate::paint::mask::Mask;
    use crate::paint::{Channel, FragmentContent};

    fn set() -> TextureSet {
        let mut s = TextureSet::new("s", "S", (8, 8)).with_channel(Channel::BaseColor);
        let rust = Layer::new("rust", "Rust", LayerKind::Fill(Fill::new(FillSource::Material { path: "materials/rust.mca".into(), params: vec![] })));
        let mut dirt = Layer::new("dirt", "Dirt", LayerKind::Paint(PaintContent::default()));
        dirt.mask = Some(Mask::new(0.0).with_effect(MaskEffect::new(GreyBlend::default(), EffectKind::Generator(Procedural::new("dirt")))));
        s.layers = vec![Layer::new("g", "Worn", LayerKind::Group(vec![rust, dirt])), Layer::new("top", "", LayerKind::Paint(PaintContent::default()))];
        s
    }

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_folder_gives_its_children_and_a_selection_its_layers() {
        let s = set();
        let f = smart_material(&s, &ids(&["g", "rust"])).unwrap();
        assert_eq!(f.name, "Worn");
        assert!(matches!(&f.content, FragmentContent::Layers(l) if l.len() == 2 && l[0].id == "rust"));
        let f = smart_material(&s, &ids(&["top", "dirt"])).unwrap();
        assert_eq!(f.name, "Smart material");
        assert!(matches!(&f.content, FragmentContent::Layers(l) if l.iter().map(|l| l.id.as_str()).collect::<Vec<_>>() == ["dirt", "top"]));
        assert!(smart_material(&s, &ids(&["nope"])).is_none());
        let m = smart_mask(&s, "dirt").unwrap();
        assert_eq!(m.name, "Dirt mask");
        assert!(smart_mask(&s, "rust").is_none());
    }

    #[test]
    fn completing_declares_needs_and_embeds_used_assets() {
        let mut f = smart_material(&set(), &ids(&["g"])).unwrap();
        let maps = |id: &str| if id == "dirt" { vec![BakedKind::Curvature] } else { vec![] };
        let report = f.complete(&maps, |r| Some(r.as_bytes().to_vec()));
        assert_eq!(f.needs, [BakedKind::Curvature]);
        assert_eq!(report.added, ["materials/rust.mca"]);
        assert_eq!(f.assets.len(), 1);
    }
}

//! What a dragged asset becomes on the layer stack (LS-18): the drop rules themselves live in
//! `paint::edit::ops::drop`; this only turns an asset into the payload, or says why it can't be dropped.
//!
//! - a material graph → [`DropPayload::Material`] (a fill layer with the material, referenced by its
//!   library-relative path, which the material lookup finds under the library roots);
//! - a generator or filter made by a built-in procedural (its provenance names it) → [`DropPayload::Effect`]
//!   (between layers a new effect layer, onto a layer its effects, onto a mask the mask's effects, Ctrl replaces);
//! - smart materials and smart masks → not yet: their file format comes with roadmap step 17, which produces
//!   [`DropPayload::Folder`] and [`DropPayload::SmartMask`] here;
//! - `.sbsar` files are importable only (ADR 0008) and never dropped.

use materialcraft::assets::{Format, Usage};
use materialcraft::paint::edit::ops::{AssetEffect, DropPayload, EffectAsset, MaterialAsset};
use materialcraft::paint::effect::Procedural;

use super::results::Item;

/// The payload `item` drops as, or why it can't be dropped on the layer stack.
pub fn payload(item: &Item) -> Result<DropPayload, &'static str> {
    if item.importable_only() {
        return Err("Importable only: MaterialCraft does not run this file; the import review offers a replacement.");
    }
    match item.usage {
        Usage::Material if item.format == Format::Graph => {
            Ok(DropPayload::Material(MaterialAsset { name: item.name.clone(), path: item.path.clone(), params: Vec::new() }))
        }
        Usage::Material => Err("Only material graphs (.mca) can be dropped as materials."),
        Usage::Generator | Usage::Filter => {
            let Some((id, params)) = &item.procedural else {
                return Err("This effect does not name the generator or filter it was made with, so it can't be added yet.");
            };
            let p = Procedural::new(id.clone()).with_params(params.clone());
            let kind = if item.usage == Usage::Filter { AssetEffect::Filter(p) } else { AssetEffect::Generator(p) };
            Ok(DropPayload::Effect(EffectAsset { name: item.name.clone(), kind }))
        }
        Usage::SmartMaterial => Err("Smart materials can be dropped once their file format exists (roadmap step 17)."),
        Usage::SmartMask => Err("Smart masks can be dropped once their file format exists (roadmap step 17)."),
        Usage::Brush | Usage::Alpha | Usage::Texture | Usage::Environment => Err("This kind of asset is not dropped onto the layer stack."),
    }
}

#[cfg(test)]
mod tests {
    use super::super::results::ThumbSource;
    use super::*;
    use materialcraft::assets::RootId;

    fn item(usage: Usage, format: Format) -> Item {
        Item {
            root: RootId::new("user"),
            path: "materials/rust.mca".into(),
            name: "Rust".into(),
            usage,
            format,
            library: "My assets".into(),
            root_dir: None,
            tags: Vec::new(),
            description: None,
            thumb: ThumbSource::None,
            stamp: (None, 0),
            procedural: None,
        }
    }

    #[test]
    fn materials_and_named_effects_drop_the_rest_says_why_not() {
        let m = payload(&item(Usage::Material, Format::Graph)).unwrap();
        assert_eq!(m, DropPayload::Material(MaterialAsset { name: "Rust".into(), path: "materials/rust.mca".into(), params: Vec::new() }));
        assert!(payload(&item(Usage::Material, Format::Sbsar)).unwrap_err().starts_with("Importable only"));
        assert!(payload(&item(Usage::Filter, Format::Graph)).is_err());
        let mut f = item(Usage::Filter, Format::Graph);
        f.procedural = Some(("blur".into(), vec![("radius".into(), "2".into())]));
        assert!(matches!(payload(&f), Ok(DropPayload::Effect(EffectAsset { kind: AssetEffect::Filter(p), .. })) if p.id == "blur" && p.params.len() == 1));
        assert!(payload(&item(Usage::SmartMaterial, Format::Graph)).is_err());
        assert!(payload(&item(Usage::Texture, Format::Png)).is_err());
    }
}

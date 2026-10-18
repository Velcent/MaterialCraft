//! What a dragged asset becomes on the layer stack (LS-18): the drop rules themselves live in
//! `paint::edit::ops::drop`; this only turns an asset into the payload, or says why it can't be dropped.
//!
//! - a material graph → [`DropPayload::Material`] (a fill layer with the material, referenced by its
//!   library-relative path, which the material lookup finds under the library roots);
//! - a generator or filter made by a built-in procedural (its provenance names it) → [`DropPayload::Effect`]
//!   (between layers a new effect layer, onto a layer its effects, onto a mask the mask's effects, Ctrl replaces);
//! - a smart material (`.mcfrag`) → [`DropPayload::Folder`] with its layers, recording the asset's library path as
//!   the folder's provenance; a smart mask → [`DropPayload::SmartMask`] with its mask's effects and the same provenance. The file is read
//!   when the drag starts;
//! - `.sbsar` files are importable only (ADR 0008) and never dropped.

use materialcraft::assets::{Format, Usage};
use materialcraft::paint::edit::ops::{AssetEffect, DropPayload, EffectAsset, MaterialAsset};
use materialcraft::paint::effect::Procedural;
use materialcraft::paint::format::fragment;
use materialcraft::paint::{EmbeddedAsset, FragmentContent, FragmentKind};

use super::results::Item;

/// The payload `item` drops as, or why it can't be dropped on the layer stack.
pub fn payload(item: &Item) -> Result<DropPayload, String> {
    if item.importable_only() {
        return Err("Importable only: MaterialCraft does not run this file; the import review offers a replacement.".into());
    }
    match item.usage {
        Usage::Material if item.format == Format::Graph => {
            Ok(DropPayload::Material(MaterialAsset { name: item.name.clone(), path: item.path.clone(), params: Vec::new() }))
        }
        Usage::Material => Err("Only material graphs (.mca) can be dropped as materials.".into()),
        Usage::Generator | Usage::Filter => {
            let Some((id, params)) = &item.procedural else {
                return Err("This effect does not name the generator or filter it was made with, so it can't be added yet.".into());
            };
            let p = Procedural::new(id.clone()).with_params(params.clone());
            let kind = if item.usage == Usage::Filter { AssetEffect::Filter(p) } else { AssetEffect::Generator(p) };
            Ok(DropPayload::Effect(EffectAsset { name: item.name.clone(), kind }))
        }
        Usage::SmartMaterial | Usage::SmartMask => fragment_payload(item),
        Usage::Brush | Usage::Alpha | Usage::Texture | Usage::Environment => Err("This kind of asset is not dropped onto the layer stack.".into()),
        Usage::Template => Err("Project templates are picked in File > New project.".into()),
    }
}

/// A smart material or smart mask read from its file.
fn fragment_payload(item: &Item) -> Result<DropPayload, String> {
    if item.format != Format::Fragment {
        return Err("Smart materials and smart masks are .mcfrag files; this one is another kind of file.".into());
    }
    let Some(dir) = &item.root_dir else { return Err("This library has no folder to read the file from.".into()) };
    let bytes = std::fs::read(dir.join(&item.path)).map_err(|e| format!("The file can't be read: {e}."))?;
    let f = fragment::load(std::io::Cursor::new(&bytes)).map_err(|e| format!("The file can't be read: {e}."))?;
    let kind = f.kind();
    let name = if item.name.trim().is_empty() { f.name } else { item.name.clone() };
    match (kind, f.content) {
        (FragmentKind::SmartMaterial, FragmentContent::Layers(layers)) => {
            // The document embeds the smart material and what it embeds, so it opens without this library (AS-21).
            let mut embed = f.assets;
            embed.push(EmbeddedAsset::new(item.path.clone(), bytes));
            Ok(DropPayload::Folder { name, layers, asset: Some(item.path.clone()), embed })
        }
        (FragmentKind::SmartMask, FragmentContent::Mask(mask)) => {
            let mut embed = f.assets;
            embed.push(EmbeddedAsset::new(item.path.clone(), bytes));
            Ok(DropPayload::SmartMask { effects: mask.effects, asset: Some(item.path.clone()), embed })
        }
        _ => Err("The file's content does not match its kind.".into()),
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
            needs: Vec::new(),
            writable: true,
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
        assert!(payload(&item(Usage::SmartMask, Format::Fragment)).unwrap_err().contains("no folder"));
        assert!(payload(&item(Usage::Texture, Format::Png)).is_err());
    }

    #[test]
    fn smart_materials_and_masks_drop_what_their_file_holds() {
        use materialcraft::paint::effect::{EffectKind, GreyBlend, MaskEffect};
        use materialcraft::paint::{Fragment, Layer, LayerKind, Mask, PaintContent, TextureSet};
        let dir = std::env::temp_dir().join(format!("mc-payload-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let set = TextureSet::new("s", "S", (8, 8));
        let layers = vec![Layer::new("a", "A", LayerKind::Paint(PaintContent::default()))];
        fragment::save_path(&Fragment::smart_material("Worn", &set, layers.clone()), &dir.join("worn.mcfrag")).unwrap();
        let mask = Mask::new(0.0).with_effect(MaskEffect::new(GreyBlend::default(), EffectKind::Fill(0.5)));
        fragment::save_path(&Fragment::smart_mask("Edges", &set, mask.clone()), &dir.join("edges.mcfrag")).unwrap();
        let at = |usage, path: &str| Item { root_dir: Some(dir.clone()), path: path.into(), ..item(usage, Format::Fragment) };
        let folder = payload(&at(Usage::SmartMaterial, "worn.mcfrag")).unwrap();
        let DropPayload::Folder { name, layers: dropped, asset, embed } = folder else { panic!("{folder:?}") };
        assert_eq!((name.as_str(), dropped, asset.as_deref()), ("Rust", layers, Some("worn.mcfrag")));
        assert_eq!(embed.iter().map(|a| a.reference.as_str()).collect::<Vec<_>>(), ["worn.mcfrag"]);
        let DropPayload::SmartMask { effects, asset, embed } = payload(&at(Usage::SmartMask, "edges.mcfrag")).unwrap() else { panic!("not a smart mask") };
        assert_eq!((effects, asset.as_deref()), (mask.effects, Some("edges.mcfrag")));
        assert_eq!(embed.iter().map(|a| a.reference.as_str()).collect::<Vec<_>>(), ["edges.mcfrag"]);
        assert!(payload(&at(Usage::SmartMask, "missing.mcfrag")).unwrap_err().contains("can't be read"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

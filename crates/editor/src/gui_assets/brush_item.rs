//! What a dragged brush or alpha becomes for the brush tools (AS-06, PT-11), or why it can't be used: the drop rules
//! of the tools strip and the brush panel live with the Paint screen; this only reads the asset.
//!
//! - a brush preset (`.mcbrush`) → [`BrushDrop::Preset`], read from its file when the drag starts (the asset's name
//!   wins over the file's);
//! - an alpha, or a brush that is only an image (a PNG under `brushes/`) → [`BrushDrop::Tip`]: the procedural tip
//!   itself when the alpha was made from one (its provenance names it, so the tip stays sharp at any size and needs
//!   no file), else the image by its library path.

use materialcraft::assets::{Format, Usage};
use materialcraft::paint::brush::{BrushPreset, Tip};
use materialcraft_brush::{PROCEDURAL_TIP_GENERATOR, PROCEDURAL_TIPS};

use super::results::Item;

/// What a brush or alpha dropped on the brush tools does.
#[derive(Clone, Debug, PartialEq)]
pub enum BrushDrop {
    /// Load the whole preset into a brush tool.
    Preset(BrushPreset),
    /// Make this the brush tool's tip.
    Tip(Tip),
}

/// What `item` does for the brush tools, or why it does nothing there.
pub fn brush_drop(item: &Item) -> Result<BrushDrop, String> {
    match (item.usage, item.format) {
        (Usage::Brush, Format::Brush) => preset(item).map(BrushDrop::Preset),
        (Usage::Brush | Usage::Alpha, Format::Png) => Ok(BrushDrop::Tip(tip(item))),
        (Usage::Brush | Usage::Alpha, _) => Err("Only PNG alphas and .mcbrush presets can be used as brushes.".into()),
        _ => Err("Only brushes and alphas go onto the brush tools.".into()),
    }
}

/// The tip an alpha image gives.
fn tip(item: &Item) -> Tip {
    let procedural = item.procedural.as_ref().filter(|(generator, _)| generator == PROCEDURAL_TIP_GENERATOR).and_then(|(_, params)| {
        let (_, id) = params.iter().find(|(k, _)| k == "tip")?;
        PROCEDURAL_TIPS.iter().any(|t| t.id == id).then(|| id.clone())
    });
    match procedural {
        Some(id) => Tip::Procedural(id),
        None => Tip::Alpha(item.path.clone()),
    }
}

/// The preset in `item`'s file.
fn preset(item: &Item) -> Result<BrushPreset, String> {
    let Some(dir) = &item.root_dir else { return Err("This library has no folder to read the preset from.".into()) };
    let text = std::fs::read_to_string(dir.join(&item.path)).map_err(|e| format!("The preset can't be read: {e}."))?;
    let mut preset = BrushPreset::from_file_text(&text).map_err(|e| format!("The preset can't be read: {e}."))?;
    if !item.name.trim().is_empty() {
        preset.name = item.name.clone();
    }
    Ok(preset)
}

/// How the brush panel names a tip.
pub fn tip_label(tip: &Tip) -> String {
    match tip {
        Tip::Round => "Round".into(),
        Tip::Procedural(id) => PROCEDURAL_TIPS.iter().find(|t| t.id == id).map_or_else(|| id.clone(), |t| t.label.to_string()),
        Tip::Alpha(path) => path.rsplit('/').next().unwrap_or(path).trim_end_matches(".png").to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::results::ThumbSource;
    use super::*;
    use materialcraft::assets::RootId;

    fn item(usage: Usage, format: Format, path: &str) -> Item {
        Item {
            root: RootId::new("starter"),
            path: path.into(),
            name: "Mine".into(),
            usage,
            format,
            library: "Starter".into(),
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
    fn alphas_become_tips_procedural_ones_the_tip_itself() {
        let plain = item(Usage::Alpha, Format::Png, "alphas/round-soft.png");
        assert_eq!(brush_drop(&plain), Ok(BrushDrop::Tip(Tip::Alpha("alphas/round-soft.png".into()))));
        let mut chalk = item(Usage::Alpha, Format::Png, "alphas/chalk.png");
        chalk.procedural = Some((PROCEDURAL_TIP_GENERATOR.into(), vec![("function".into(), "chalk_tip(seed 1)".into()), ("tip".into(), "chalk".into())]));
        assert_eq!(brush_drop(&chalk), Ok(BrushDrop::Tip(Tip::Procedural("chalk".into()))));
        chalk.procedural = Some((PROCEDURAL_TIP_GENERATOR.into(), vec![("tip".into(), "unheard-of".into())]));
        assert_eq!(brush_drop(&chalk), Ok(BrushDrop::Tip(Tip::Alpha("alphas/chalk.png".into()))), "an unknown tip stays an image");
        assert_eq!(brush_drop(&item(Usage::Brush, Format::Png, "brushes/pencil.png")), Ok(BrushDrop::Tip(Tip::Alpha("brushes/pencil.png".into()))));
        assert!(brush_drop(&item(Usage::Alpha, Format::Exr, "alphas/x.exr")).is_err());
        assert!(brush_drop(&item(Usage::Texture, Format::Png, "textures/x.png")).is_err());
        assert!(brush_drop(&item(Usage::Material, Format::Graph, "materials/x.mca")).is_err());
        assert_eq!(tip_label(&Tip::Procedural("chalk".into())), "Chalk");
        assert_eq!(tip_label(&Tip::Alpha("alphas/round-soft.png".into())), "round-soft");
    }

    #[test]
    fn presets_load_from_their_file_under_the_assets_name() {
        let dir = std::env::temp_dir().join(format!("mc-brush-item-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("brushes")).unwrap();
        let p = BrushPreset { name: "In file".into(), tip: Tip::Procedural("rake".into()), spacing: 0.04, ..BrushPreset::default() };
        std::fs::write(dir.join("brushes/rake.mcbrush"), p.to_file_text()).unwrap();
        std::fs::write(dir.join("brushes/bad.mcbrush"), "{").unwrap();
        let at = |path: &str| Item { root_dir: Some(dir.clone()), ..item(Usage::Brush, Format::Brush, path) };
        assert_eq!(brush_drop(&at("brushes/rake.mcbrush")), Ok(BrushDrop::Preset(BrushPreset { name: "Mine".into(), ..p })));
        assert!(brush_drop(&at("brushes/bad.mcbrush")).unwrap_err().contains("can't be read"));
        assert!(brush_drop(&at("brushes/missing.mcbrush")).is_err());
        assert!(brush_drop(&item(Usage::Brush, Format::Brush, "brushes/rake.mcbrush")).unwrap_err().contains("no folder"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

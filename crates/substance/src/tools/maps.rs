//! `mcsub maps`: the final maps of every texture set of an imported document, as PNGs for visual checks.

use super::png_out::PngMap;
use materialcraft::generators;
use materialcraft::paint::{BakedKind, Channel, Document, MaterialMaps, NO_FILL_CONTEXT, OutputOptions};
use std::io;
use std::path::{Path, PathBuf};

/// Channels written, with gray/bit depth: 16 bits for height and normal, 8 for the rest.
pub const MAP_CHANNELS: &[(Channel, PngMap)] = &[
    (Channel::BaseColor, PngMap { gray: false, bits: 8 }),
    (Channel::Roughness, PngMap { gray: true, bits: 8 }),
    (Channel::Metallic, PngMap { gray: true, bits: 8 }),
    (Channel::Normal, PngMap { gray: false, bits: 16 }),
    (Channel::Height, PngMap { gray: true, bits: 16 }),
    (Channel::AmbientOcclusion, PngMap { gray: true, bits: 8 }),
    (Channel::Emissive, PngMap { gray: false, bits: 8 }),
    (Channel::Opacity, PngMap { gray: true, bits: 8 }),
];

/// Writes `<set>_<channel>.png` for each set and each channel the set declares (AO and normal also when baked),
/// and `<set>_baked_<kind>.png` for each baked map.
pub fn write_maps(doc: &Document, dir: &Path, progress: &mut dyn FnMut(&str)) -> io::Result<Vec<PathBuf>> {
    std::fs::create_dir_all(dir)?;
    let mut written = Vec::new();
    for set in &doc.texture_sets {
        progress(&set.name);
        let maps = MaterialMaps::build_with(set, &OutputOptions::default(), generators::builtin(), &NO_FILL_CONTEXT);
        for (channel, png) in MAP_CHANNELS {
            let baked = match channel {
                Channel::AmbientOcclusion => set.baked(&BakedKind::AmbientOcclusion).is_some(),
                Channel::Normal => set.baked(&BakedKind::Normal).is_some(),
                _ => false,
            };
            let Some(tex) = maps.map(*channel).filter(|_| set.has_channel(*channel) || baked) else {
                continue;
            };
            let path = dir.join(format!("{}_{}.png", file_safe(&set.name), channel.id()));
            png.write(tex, &path)?;
            written.push(path);
        }
        for baked in &set.baked {
            let path = dir.join(format!(
                "{}_baked_{}.png",
                file_safe(&set.name),
                file_safe(baked.kind.id())
            ));
            PngMap { gray: false, bits: 16 }.write(&baked.image.to_tex(), &path)?;
            written.push(path);
        }
    }
    Ok(written)
}

/// A file-name-safe version of a set name (path separators and control characters replaced).
pub fn file_safe(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c == '/' || c == '\\' || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect()
}

//! The baker's [`MapKind`]s as the document's [`BakedKind`]s, and a bake's result as the set's [`BakedMap`]s.

use crate::bake::{BakedMaps, MapKind};
use crate::paint::image::{ImageError, TiledImage};
use crate::paint::pixel::{PixelFormat, PixelLayout, SampleType};
use crate::paint::texture_set::{BakedKind, BakedMap};

pub fn baked_kind(kind: MapKind) -> BakedKind {
    match kind {
        MapKind::Normal => BakedKind::Normal,
        MapKind::WorldNormal => BakedKind::WorldSpaceNormal,
        MapKind::Position => BakedKind::Position,
        MapKind::AmbientOcclusion => BakedKind::AmbientOcclusion,
        MapKind::Curvature => BakedKind::Curvature,
        MapKind::Thickness => BakedKind::Thickness,
        MapKind::Id => BakedKind::Id,
    }
}

/// The baker's kind of a baked map; `None` for maps the baker does not make.
pub fn map_kind(kind: &BakedKind) -> Option<MapKind> {
    MapKind::ALL.into_iter().find(|k| &baked_kind(*k) == kind)
}

/// How a baked map of `kind` is stored: 16 bits per sample, grey for the single-value maps.
pub fn storage_format(kind: MapKind) -> PixelFormat {
    let layout = match kind {
        MapKind::AmbientOcclusion | MapKind::Curvature | MapKind::Thickness => PixelLayout::Gray,
        MapKind::Normal | MapKind::WorldNormal | MapKind::Position | MapKind::Id => PixelLayout::Rgb,
    };
    PixelFormat::new(layout, SampleType::U16)
}

/// The maps of one bake as the set stores them, in baking order.
pub fn to_baked(maps: &BakedMaps) -> Result<Vec<BakedMap>, ImageError> {
    maps.maps.iter().map(|(kind, tex)| Ok(BakedMap { kind: baked_kind(*kind), image: TiledImage::from_tex(tex, storage_format(*kind))? })).collect()
}

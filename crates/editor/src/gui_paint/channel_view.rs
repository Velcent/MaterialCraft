//! [`ChannelView`]: what the viewport shows, the lit material or one channel, and how a texture
//! set's [`MaterialMaps`] become GPU maps for it. A channel is shown as the base colour of a plain rough
//! surface; the viewport draws it unlit (as it is) or, with lit channels on, lit by the scene.

use materialcraft::bake::MapKind;
use materialcraft::gpu3d::{self, GpuMaps, MapInputs};
use materialcraft::paint::bake_config::baked_kind;
use materialcraft::paint::{Channel, MaterialMaps, TextureSet};
use materialcraft::tex::Tex;

/// The viewport's display mode.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChannelView {
    /// Full PBR shading, as in Painter's material mode.
    Material,
    /// One channel's values (colour channels in colour, the others in grey).
    Single(Channel),
    /// A baked mesh map of each set (the bakers panel's show toggle, BK2-11); black where a set has none.
    Baked(MapKind),
}

impl ChannelView {
    /// Every choice with its label, in menu order.
    pub const ALL: [(ChannelView, &'static str); 8] = [
        (ChannelView::Material, "Material"),
        (ChannelView::Single(Channel::BaseColor), "Base colour"),
        (ChannelView::Single(Channel::Roughness), "Roughness"),
        (ChannelView::Single(Channel::Metallic), "Metallic"),
        (ChannelView::Single(Channel::Normal), "Normal"),
        (ChannelView::Single(Channel::Height), "Height"),
        (ChannelView::Single(Channel::AmbientOcclusion), "AO"),
        (ChannelView::Single(Channel::Emissive), "Emissive"),
    ];

    /// GPU maps of one texture set for this display mode. Base colour and emissive are kept
    /// sRGB-encoded, which is what the viewer's sRGB textures expect (no second gamma).
    pub fn prepare(self, maps: &MaterialMaps, set: &TextureSet) -> GpuMaps {
        let height = set.has_channel(Channel::Height).then_some(&maps.height);
        match self {
            ChannelView::Material => gpu3d::prepare(&MapInputs {
                base: Some(&maps.base_color),
                normal: Some(&maps.normal),
                rough: Some(&maps.roughness),
                metal: Some(&maps.metallic),
                ao: Some(&maps.ambient_occlusion),
                emissive: set.has_channel(Channel::Emissive).then_some(&maps.emissive),
                height,
            }),
            ChannelView::Single(c) => shown_maps(maps.map(c).unwrap_or(&maps.base_color), height),
            ChannelView::Baked(kind) => match set.baked(&baked_kind(kind)) {
                Some(image) => shown_maps(&image.to_tex(), height),
                None => shown_maps(&Tex::solid(1, 1, [0.0, 0.0, 0.0, 1.0]), height),
            },
        }
    }
}

/// `source`'s values as the base colour of a rough, flat, non-metal surface (the values are display values, as
/// the base colour's sRGB texture expects, so unlit shading shows them exactly).
fn shown_maps(source: &Tex, height: Option<&Tex>) -> GpuMaps {
    let opaque = source.map(|p| [p[0], p[1], p[2], 1.0]);
    let rough = Tex::solid(1, 1, [0.8, 0.8, 0.8, 1.0]);
    gpu3d::prepare(&MapInputs { base: Some(&opaque), normal: None, rough: Some(&rough), metal: None, ao: None, emissive: None, height })
}

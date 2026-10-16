//! [`ChannelView`]: what the viewport shows, the lit material or one channel, and how a texture
//! set's [`MaterialMaps`] become GPU maps for it.

use super::unlit;
use materialcraft::gpu3d::{self, GpuMaps, MapInputs};
use materialcraft::paint::{Channel, MaterialMaps, TextureSet};
use materialcraft::render3d::View;
use materialcraft::tex::Tex;

/// The viewport's display mode.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChannelView {
    /// Full PBR shading, as in Painter's material mode.
    Material,
    /// One channel's values, unlit (colour channels in colour, the others in grey).
    Single(Channel),
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
            ChannelView::Single(c) => {
                let source = maps.map(c).unwrap_or(&maps.base_color);
                let shown = source.map(|p| {
                    let e = unlit::emissive_for([p[0], p[1], p[2]]);
                    [e[0], e[1], e[2], 1.0]
                });
                let black = Tex::solid(1, 1, [0.0, 0.0, 0.0, 1.0]);
                let rough = Tex::solid(1, 1, [1.0; 4]);
                gpu3d::prepare(&MapInputs {
                    base: Some(&black),
                    normal: None,
                    rough: Some(&rough),
                    metal: None,
                    ao: None,
                    emissive: Some(&shown),
                    height,
                })
            }
        }
    }

    /// The camera and light settings to draw with: single channels are drawn with the lights off.
    pub fn view_for(self, view: &View) -> View {
        match self {
            ChannelView::Material => view.clone(),
            ChannelView::Single(_) => View {
                light_intensity: 0.0,
                ambient: 0.0,
                ..view.clone()
            },
        }
    }
}

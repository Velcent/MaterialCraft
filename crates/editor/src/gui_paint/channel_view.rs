//! [`ChannelView`]: what the views show (DS-03), the lit material, one channel or one baked mesh map, and how a
//! texture set's [`MaterialMaps`] become GPU maps for it. A channel or mesh map is shown as the base colour of a
//! plain rough surface, through the [`ChannelDisplay`] options; the views draw it unlit (as it is) or, with lit
//! channels on, lit by the scene.

use super::channel_display::{ChannelDisplay, Encoding};
use materialcraft::bake::MapKind;
use materialcraft::gpu3d::{self, GpuMaps, MapInputs};
use materialcraft::paint::bake_config::baked_kind;
use materialcraft::paint::{Channel, MaterialMaps, TextureSet};
use materialcraft::tex::Tex;

/// The views' display mode.
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
    /// The material and every channel with its label, in menu order.
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

    /// Every mesh map with its label, in menu order.
    pub const MESH_MAPS: [(ChannelView, &'static str); 7] = [
        (ChannelView::Baked(MapKind::Normal), "Normal (mesh map)"),
        (ChannelView::Baked(MapKind::WorldNormal), "World space normal"),
        (ChannelView::Baked(MapKind::Position), "Position"),
        (ChannelView::Baked(MapKind::AmbientOcclusion), "Ambient occlusion (mesh map)"),
        (ChannelView::Baked(MapKind::Curvature), "Curvature"),
        (ChannelView::Baked(MapKind::Thickness), "Thickness"),
        (ChannelView::Baked(MapKind::Id), "ID"),
    ];

    /// What the view shows, for its menu and its corner label (DS-05).
    pub fn label(self) -> &'static str {
        Self::ALL.iter().chain(Self::MESH_MAPS.iter()).find(|(v, _)| *v == self).map_or("?", |(_, l)| *l)
    }

    /// The next (`step` 1) or previous (−1) mode of the same kind: channels cycle through the material and the
    /// channels, mesh maps through the mesh maps (DS-04).
    pub fn cycled(self, step: i32) -> ChannelView {
        let list: &[(ChannelView, &str)] = if matches!(self, ChannelView::Baked(_)) { &Self::MESH_MAPS } else { &Self::ALL };
        let at = list.iter().position(|(v, _)| *v == self).unwrap_or(0) as i64;
        let next = (at + i64::from(step)).rem_euclid(list.len() as i64) as usize;
        list.get(next).map_or(self, |(v, _)| *v)
    }

    /// GPU maps of one texture set for this display mode, channels and mesh maps shown through `display`. Base
    /// colour and emissive are kept sRGB-encoded, which is what the viewer's sRGB textures expect (no second gamma).
    pub fn prepare(self, maps: &MaterialMaps, set: &TextureSet, display: &ChannelDisplay) -> GpuMaps {
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
                let encoding = if c == Channel::Normal { Encoding::Biased } else { Encoding::Plain };
                shown_maps(&display.apply(maps.map(c).unwrap_or(&maps.base_color), c.is_color(), encoding), height)
            }
            ChannelView::Baked(kind) => {
                let encoding = if matches!(kind, MapKind::Normal | MapKind::WorldNormal) { Encoding::Biased } else { Encoding::Plain };
                let color = matches!(kind, MapKind::Normal | MapKind::WorldNormal | MapKind::Position | MapKind::Id);
                match set.baked(&baked_kind(kind)) {
                    Some(image) => shown_maps(&display.apply(&image.to_tex(), color, encoding), height),
                    None => shown_maps(&Tex::solid(1, 1, [0.0, 0.0, 0.0, 1.0]), height),
                }
            }
        }
    }
}

/// A display mode with its channel display options: everything that decides the maps the views upload.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Shown {
    pub view: ChannelView,
    pub channel: ChannelDisplay,
}

impl Shown {
    pub fn prepare(&self, maps: &MaterialMaps, set: &TextureSet) -> GpuMaps {
        self.view.prepare(maps, set, &self.channel)
    }
}

/// `source`'s values as the base colour of a rough, flat, non-metal surface (the values are display values, as
/// the base colour's sRGB texture expects, so unlit shading shows them exactly).
fn shown_maps(source: &Tex, height: Option<&Tex>) -> GpuMaps {
    let opaque = source.map(|p| [p[0], p[1], p[2], 1.0]);
    let rough = Tex::solid(1, 1, [0.8, 0.8, 0.8, 1.0]);
    gpu3d::prepare(&MapInputs { base: Some(&opaque), normal: None, rough: Some(&rough), metal: None, ao: None, emissive: None, height })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_cycle_within_their_kind_and_name_themselves() {
        assert_eq!(ChannelView::Material.cycled(-1), ChannelView::Single(Channel::Emissive));
        assert_eq!(ChannelView::Material.cycled(1), ChannelView::Single(Channel::BaseColor));
        assert_eq!(ChannelView::Baked(MapKind::Id).cycled(1), ChannelView::Baked(MapKind::Normal));
        assert_eq!(ChannelView::Baked(MapKind::Curvature).label(), "Curvature");
        assert_eq!(ChannelView::Single(Channel::Height).label(), "Height");
    }
}

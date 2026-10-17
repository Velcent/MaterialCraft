//! [`TextureSet`]: the channels, layer stack and baked mesh maps painted onto one material slot.

use super::channel::{Channel, ChannelSpec};
use super::image::TiledImage;
use super::layer::{Layer, LayerKind, LayerWalk};

/// Kind of a map baked from the mesh.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BakedKind {
    AmbientOcclusion,
    Curvature,
    /// Tangent-space normal from a high-poly mesh (encoded, OpenGL +Y).
    Normal,
    WorldSpaceNormal,
    Position,
    Thickness,
    /// Colour / material ID map.
    Id,
    /// Anything else, by its id.
    Other(String),
}

impl BakedKind {
    /// Stable id used in files: `ambient_occlusion`, `curvature`, `normal`, `world_normal`,
    /// `position`, `thickness`, `id`, or the id of an `Other` map.
    pub fn id(&self) -> &str {
        match self {
            BakedKind::AmbientOcclusion => "ambient_occlusion",
            BakedKind::Curvature => "curvature",
            BakedKind::Normal => "normal",
            BakedKind::WorldSpaceNormal => "world_normal",
            BakedKind::Position => "position",
            BakedKind::Thickness => "thickness",
            BakedKind::Id => "id",
            BakedKind::Other(s) => s,
        }
    }

    /// Parses an id; unknown ids become `Other`.
    pub fn from_id(id: &str) -> BakedKind {
        match id {
            "ambient_occlusion" => BakedKind::AmbientOcclusion,
            "curvature" => BakedKind::Curvature,
            "normal" => BakedKind::Normal,
            "world_normal" => BakedKind::WorldSpaceNormal,
            "position" => BakedKind::Position,
            "thickness" => BakedKind::Thickness,
            "id" => BakedKind::Id,
            other => BakedKind::Other(other.to_string()),
        }
    }
}

/// A map baked from the mesh. Its resolution may differ from the texture set's.
#[derive(Debug, Clone, PartialEq)]
pub struct BakedMap {
    pub kind: BakedKind,
    pub image: TiledImage,
}

/// The paintable textures of one material slot.
#[derive(Debug, Clone, PartialEq)]
pub struct TextureSet {
    /// Unique within the document.
    pub id: String,
    pub name: String,
    /// Index into [`crate::mesh::Mesh::materials`].
    pub material_slot: usize,
    /// UV set the textures are painted on.
    pub uv_set: usize,
    /// Width and height of every layer image and mask.
    pub resolution: (u32, u32),
    /// Declared channels, in display order. Each channel at most once.
    pub channels: Vec<ChannelSpec>,
    /// Layer stack, bottom to top.
    pub layers: Vec<Layer>,
    pub baked: Vec<BakedMap>,
}

impl TextureSet {
    /// An empty texture set on material slot 0 and UV set 0, without channels or layers.
    pub fn new(id: impl Into<String>, name: impl Into<String>, resolution: (u32, u32)) -> TextureSet {
        TextureSet { id: id.into(), name: name.into(), material_slot: 0, uv_set: 0, resolution, channels: Vec::new(), layers: Vec::new(), baked: Vec::new() }
    }

    /// Declares `channel` with its default spec unless it is already declared.
    pub fn with_channel(mut self, channel: Channel) -> TextureSet {
        if self.channel_spec(channel).is_none() {
            self.channels.push(ChannelSpec::new(channel));
        }
        self
    }

    pub fn channel_spec(&self, channel: Channel) -> Option<&ChannelSpec> {
        self.channels.iter().find(|s| s.channel == channel)
    }

    pub fn has_channel(&self, channel: Channel) -> bool {
        self.channel_spec(channel).is_some()
    }

    /// Every layer, groups before their children, as `(depth, layer)`.
    pub fn walk_layers(&self) -> LayerWalk<'_> {
        LayerWalk::new(&self.layers)
    }

    /// The layer with `id`, searched through groups.
    pub fn layer(&self, id: &str) -> Option<&Layer> {
        self.walk_layers().map(|(_, l)| l).find(|l| l.id == id)
    }

    /// Mutable access to the layer with `id`, searched through groups.
    pub fn layer_mut(&mut self, id: &str) -> Option<&mut Layer> {
        find_mut(&mut self.layers, id)
    }

    /// The first baked map of `kind`.
    pub fn baked(&self, kind: &BakedKind) -> Option<&TiledImage> {
        self.baked.iter().find(|b| &b.kind == kind).map(|b| &b.image)
    }
}

fn find_mut<'a>(layers: &'a mut [Layer], id: &str) -> Option<&'a mut Layer> {
    for l in layers {
        if l.id == id {
            return Some(l);
        }
        if let LayerKind::Group(children) = &mut l.kind
            && let Some(found) = find_mut(children, id)
        {
            return Some(found);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::layer::Fill;

    #[test]
    fn lookups() {
        let mut set = TextureSet::new("s", "S", (64, 64)).with_channel(Channel::BaseColor).with_channel(Channel::BaseColor);
        assert_eq!(set.channels.len(), 1);
        let inner = Layer::new("inner", "Inner", LayerKind::Fill(Fill::uniform(vec![])));
        set.layers.push(Layer::new("g", "G", LayerKind::Group(vec![inner])));
        assert!(set.layer("inner").is_some());
        if let Some(l) = set.layer_mut("inner") {
            l.visible = false;
        }
        assert_eq!(set.layer("inner").map(|l| l.visible), Some(false));
        assert!(set.layer("nope").is_none());
        for k in [BakedKind::AmbientOcclusion, BakedKind::WorldSpaceNormal, BakedKind::Other("bent".into())] {
            assert_eq!(BakedKind::from_id(k.id()), k);
        }
    }
}

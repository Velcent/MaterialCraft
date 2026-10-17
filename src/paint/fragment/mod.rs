//! Fragments: the composite assets of the asset tray (AS-04, MK-15). A **smart material** is a layer-stack
//! fragment (layers with their masks, effects and pixels; dropped, it becomes a folder), a **smart mask** a mask
//! with its effect stack. Both are stored as `.mcfrag` files by [`crate::paint::format::fragment`] and declare
//! the baked maps they need (AS-05).
//!
//! - [`Fragment`]: the model, and the texture set it travels in ([`Fragment::carrier`]);
//! - [`needs`]: which baked maps a fragment's effects read, and which of them a texture set lacks;
//! - [`adapt`]: fitting a fragment's layers or mask to the texture set it is dropped into (resolution, channels).

pub mod adapt;
pub mod needs;

#[cfg(test)]
mod tests;

use super::channel::ChannelSpec;
use super::doc::Document;
use super::embedded::EmbeddedAsset;
use super::layer::{Layer, LayerKind, PaintContent};
use super::mask::Mask;
use super::texture_set::{BakedKind, TextureSet};
use super::validate::DocError;

/// Which kind of asset a fragment is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FragmentKind {
    SmartMaterial,
    SmartMask,
}

impl FragmentKind {
    /// Stable id used in files: `smart_material`, `smart_mask`.
    pub fn id(self) -> &'static str {
        match self {
            FragmentKind::SmartMaterial => "smart_material",
            FragmentKind::SmartMask => "smart_mask",
        }
    }

    pub fn from_id(id: &str) -> Option<FragmentKind> {
        [FragmentKind::SmartMaterial, FragmentKind::SmartMask].into_iter().find(|k| k.id() == id)
    }
}

/// What a fragment holds.
#[derive(Debug, Clone, PartialEq)]
pub enum FragmentContent {
    /// A smart material's layers, bottom to top.
    Layers(Vec<Layer>),
    /// A smart mask: the mask with its effects.
    Mask(Mask),
}

/// A smart material or smart mask.
#[derive(Debug, Clone, PartialEq)]
pub struct Fragment {
    /// Display name (the folder's name when dropped).
    pub name: String,
    /// The size of every paint, effect and mask image in it (they are resampled where it is dropped).
    pub resolution: (u32, u32),
    /// The channels its layers use, with their formats and colour spaces (a smart mask declares none).
    pub channels: Vec<ChannelSpec>,
    /// Baked maps its effects read (AS-05).
    pub needs: Vec<BakedKind>,
    pub content: FragmentContent,
    /// Library assets its layers use (material graphs), embedded so the fragment works on its own.
    pub assets: Vec<EmbeddedAsset>,
}

/// Id of the texture set a fragment travels in, see [`Fragment::carrier`].
pub const CARRIER_SET: &str = "fragment";
/// Id of the layer carrying a smart mask in [`Fragment::carrier`].
pub const CARRIER_LAYER: &str = "mask";

impl Fragment {
    /// A smart material made of `layers` of `set` (copies), with the set's channels and resolution.
    pub fn smart_material(name: impl Into<String>, set: &TextureSet, layers: Vec<Layer>) -> Fragment {
        Fragment::new(name, set, set.channels.clone(), FragmentContent::Layers(layers))
    }

    /// A smart mask made of `mask` (a copy) of a layer of `set`.
    pub fn smart_mask(name: impl Into<String>, set: &TextureSet, mask: Mask) -> Fragment {
        Fragment::new(name, set, Vec::new(), FragmentContent::Mask(mask))
    }

    fn new(name: impl Into<String>, set: &TextureSet, channels: Vec<ChannelSpec>, content: FragmentContent) -> Fragment {
        Fragment { name: name.into(), resolution: set.resolution, channels, needs: Vec::new(), content, assets: Vec::new() }
    }

    pub fn kind(&self) -> FragmentKind {
        match self.content {
            FragmentContent::Layers(_) => FragmentKind::SmartMaterial,
            FragmentContent::Mask(_) => FragmentKind::SmartMask,
        }
    }

    /// The fragment as a texture set (id [`CARRIER_SET`]): its layers, or for a smart mask one empty paint layer
    /// (id [`CARRIER_LAYER`]) holding the mask. The format writes and reads fragments through it, and validation
    /// uses the document rules.
    pub fn carrier(&self) -> TextureSet {
        let mut set = TextureSet::new(CARRIER_SET, self.name.clone(), self.resolution);
        set.channels = self.channels.clone();
        set.layers = match &self.content {
            FragmentContent::Layers(layers) => layers.clone(),
            FragmentContent::Mask(mask) => {
                let mut l = Layer::new(CARRIER_LAYER, "", LayerKind::Paint(PaintContent::default()));
                l.mask = Some(mask.clone());
                vec![l]
            }
        };
        set
    }

    /// The fragment back from its carrier set (the inverse of [`Self::carrier`]); `None` if a smart mask's set does
    /// not hold exactly the carrier layer with a mask.
    pub fn from_carrier(kind: FragmentKind, set: TextureSet, needs: Vec<BakedKind>, assets: Vec<EmbeddedAsset>) -> Option<Fragment> {
        let content = match kind {
            FragmentKind::SmartMaterial => FragmentContent::Layers(set.layers),
            FragmentKind::SmartMask => {
                let [layer] = <[Layer; 1]>::try_from(set.layers).ok()?;
                FragmentContent::Mask(layer.mask?)
            }
        };
        Some(Fragment { name: set.name, resolution: set.resolution, channels: set.channels, needs, content, assets })
    }

    /// Checks the fragment with the document rules (ids, channels, image sizes, anchors) on its carrier set.
    pub fn validate(&self) -> Result<(), DocError> {
        let doc = Document { texture_sets: vec![self.carrier()], ..Document::default() };
        doc.validate()
    }
}

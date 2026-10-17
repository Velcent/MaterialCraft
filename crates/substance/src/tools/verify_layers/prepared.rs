//! [`PreparedSet`]: one texture set of a project, ready to verify: its declared channels, resolution and the
//! texture-cache images of the channels being compared. Imports and composites the layer stack on request.

use crate::model::PainterTextureSet;
use crate::project::CacheEntryRef;
use crate::translate::cache_channels::CacheChannels;
use crate::translate::channel_table::{channel_for_bit, channel_spec};
use crate::translate::layers::cache_score::{Score, score};
use crate::translate::layers::{BlendTable, ImageSource, ImportedStack, LayerImport, SetChannels};
use crate::translate::set_layers::defaults_layer;
use materialcraft::paint::{Channel, Compositor, TextureSet, TiledImage};

pub struct PreparedSet<'a> {
    pub painter: &'a PainterTextureSet,
    pub channels: SetChannels,
    pub resolution: (u32, u32),
    entries: Vec<&'a CacheEntryRef>,
    cache: Vec<(Channel, TiledImage)>,
}

impl<'a> PreparedSet<'a> {
    /// `None` when the set has no texture cache.
    pub fn new(painter: &'a PainterTextureSet, cache: &'a CacheChannels<'a>) -> Option<PreparedSet<'a>> {
        let folder = painter.material_uid?.to_string();
        let entries = cache.entries_of(&folder);
        if entries.is_empty() {
            return None;
        }
        let mut specs = Vec::new();
        for decl in &painter.channels {
            if let Some(c) = channel_for_bit(decl.channel.bit())
                && !specs.iter().any(|s: &materialcraft::paint::ChannelSpec| s.channel == c)
            {
                specs.push(channel_spec(c, Some(decl.format)));
            }
        }
        for e in &entries {
            if let Some(c) = channel_for_bit(e.channel_bit).filter(|c| !specs.iter().any(|s| s.channel == *c)) {
                specs.push(channel_spec(c, None));
            }
        }
        let resolution = painter.resolution().map_or((2048, 2048), |[w, h]| (w, h));
        Some(PreparedSet {
            painter,
            channels: SetChannels::new(&specs),
            resolution,
            entries,
            cache: Vec::new(),
        })
    }

    pub fn import(&self, table: &BlendTable, images: &dyn ImageSource) -> ImportedStack {
        LayerImport { table, images }.import(self.painter, &self.channels, self.resolution)
    }

    /// Reads the cache image of `channel` (once); false if there is none or it does not decode.
    pub fn load(&mut self, channel: Channel, cache: &CacheChannels) -> bool {
        if self.cache.iter().any(|(c, _)| *c == channel) {
            return true;
        }
        let Some(entry) = self
            .entries
            .iter()
            .find(|e| channel_for_bit(e.channel_bit) == Some(channel))
        else {
            return false;
        };
        match cache.read(entry) {
            Ok(image) => {
                if image.image.size() != self.resolution {
                    self.resolution = image.image.size();
                }
                self.cache.push((channel, image.image));
                true
            }
            Err(_) => false,
        }
    }

    /// Our composite of `channel` (imported layers over Painter's defaults) against the loaded cache image.
    pub fn score(&self, stack: &ImportedStack, channel: Channel) -> Option<Score> {
        let cache = self.cache.iter().find(|(c, _)| *c == channel).map(|(_, i)| i)?;
        let spec = self.channels.spec(channel)?;
        let mut set = TextureSet::new("verify", self.painter.name.clone(), self.resolution);
        set.channels = self.channels.specs().to_vec();
        set.layers.push(defaults_layer(&set));
        set.layers.extend(stack.layers.iter().cloned());
        let (ours, _) = Compositor::new(&set).composite_tex(channel);
        Some(score(&ours, cache, spec))
    }
}

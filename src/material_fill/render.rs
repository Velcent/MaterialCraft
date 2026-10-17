//! [`RenderedMaterial`]: a material's graph evaluated once at one size, kept as one tileable texture per paint
//! channel with its prefiltered levels, ready to be sampled through a fill's projection.
//!
//! Graph outputs map to channels by their usage: `basecolor` (else `diffuse`) → base colour, `normal`,
//! `roughness` (else `1 - glossiness`), `metallic`, `height`, `ambient_occlusion`, `opacity`, `emissive`.
//! Values are taken as the channel's stored encoding (base colour and emissive sRGB-encoded, as the graph
//! editor shows them; normals OpenGL-encoded); other outputs are not used.

use super::material::{Material, MaterialError};
use crate::paint::Channel;
use crate::projection::{ImageLevels, LevelChain, Texels};
use crate::tex::Tex;
use std::collections::BTreeMap;

/// Largest render side (the graph engine's own limit).
pub const MAX_RENDER_SIZE: usize = 8192;

/// One channel of a render (outputs that are not square are left out): a square plane of grey or RGB values (coverage 1) and its level chain.
pub struct ChannelTexture {
    plane: Plane,
    levels: LevelChain,
}

impl ChannelTexture {
    fn new(plane: Plane) -> ChannelTexture {
        let levels = LevelChain::build(&plane);
        ChannelTexture { plane, levels }
    }

    /// The texture with its prefiltered levels, for [`ImageLevels::sample`].
    pub fn levels(&self) -> ImageLevels<'_> {
        ImageLevels::new(&self.plane, &self.levels)
    }

    /// Memory held (plane and levels), in bytes.
    pub fn bytes(&self) -> usize {
        let (w, h) = self.plane.size();
        let levels = (w as usize) * (h as usize) * 16 / 3;
        self.plane.data.len() * 4 + levels
    }
}

/// Grey (one value per texel) or RGB values, row by row.
struct Plane {
    side: usize,
    rgb: bool,
    data: Vec<f32>,
}

impl Plane {
    fn from_tex(t: &Tex, rgb: bool, map: impl Fn(f32) -> f32) -> Plane {
        let data = match rgb {
            true => t.px.iter().flat_map(|p| [map(p[0]), map(p[1]), map(p[2])]).collect(),
            false => t.px.iter().map(|p| map(p[0])).collect(),
        };
        Plane { side: t.w, rgb, data }
    }
}

impl Texels for Plane {
    fn size(&self) -> (u32, u32) {
        (self.side as u32, self.side as u32)
    }

    fn texel(&self, x: u32, y: u32) -> [f32; 4] {
        let i = y as usize * self.side + x as usize;
        match self.rgb {
            true => match self.data.get(i * 3..i * 3 + 3) {
                Some(&[r, g, b]) => [r, g, b, 1.0],
                _ => [0.0; 4],
            },
            false => self.data.get(i).map_or([0.0; 4], |&v| [v, v, v, 1.0]),
        }
    }
}

/// How a channel's texture is made from the graph's outputs: the first usage present wins.
struct ChannelSource {
    channel: Channel,
    usages: &'static [(&'static str, bool)],
}

/// (usage, inverted) per channel; inverted takes `1 - v` (glossiness → roughness).
const SOURCES: [ChannelSource; 8] = [
    ChannelSource { channel: Channel::BaseColor, usages: &[("basecolor", false), ("diffuse", false)] },
    ChannelSource { channel: Channel::Normal, usages: &[("normal", false)] },
    ChannelSource { channel: Channel::Roughness, usages: &[("roughness", false), ("glossiness", true)] },
    ChannelSource { channel: Channel::Metallic, usages: &[("metallic", false)] },
    ChannelSource { channel: Channel::Height, usages: &[("height", false)] },
    ChannelSource { channel: Channel::AmbientOcclusion, usages: &[("ambient_occlusion", false)] },
    ChannelSource { channel: Channel::Opacity, usages: &[("opacity", false)] },
    ChannelSource { channel: Channel::Emissive, usages: &[("emissive", false)] },
];

/// A material rendered at one size.
pub struct RenderedMaterial {
    size: usize,
    channels: Vec<(Channel, ChannelTexture)>,
}

impl RenderedMaterial {
    /// Evaluates `material` at `size` × `size` (1 … [`MAX_RENDER_SIZE`]) and builds every channel's levels.
    pub fn render(material: &Material, size: usize) -> Result<RenderedMaterial, MaterialError> {
        if size == 0 || size > MAX_RENDER_SIZE {
            return Err(MaterialError::Render(format!("size {size} is outside 1 … {MAX_RENDER_SIZE}")));
        }
        let graph = material.graph();
        let mut outputs = graph.evaluate(Some(size)).map_err(|e| MaterialError::Render(e.to_string()))?;
        let mut by_usage: BTreeMap<String, Tex> = BTreeMap::new();
        for (usage, ident, _) in graph.output_nodes() {
            if by_usage.contains_key(&usage) {
                continue;
            }
            if let Some(t) = outputs.remove(&ident) {
                by_usage.insert(usage, t);
            }
        }
        let planes: Vec<(Channel, &Tex, bool)> = SOURCES
            .iter()
            .filter_map(|s| s.usages.iter().find_map(|(u, inv)| by_usage.get(*u).map(|t| (s.channel, t, *inv))))
            .filter(|(_, t, _)| t.w > 0 && t.w == t.h && t.px.len() == t.w * t.h)
            .collect();
        let channels = std::thread::scope(|scope| {
            let jobs: Vec<_> = planes
                .iter()
                .map(|&(channel, tex, inverted)| {
                    scope.spawn(move || {
                        let plane = Plane::from_tex(tex, channel.is_color(), |v| if inverted { 1.0 - v } else { v });
                        (channel, ChannelTexture::new(plane))
                    })
                })
                .collect();
            jobs.into_iter().filter_map(|j| j.join().ok()).collect::<Vec<_>>()
        });
        Ok(RenderedMaterial { size, channels })
    }

    pub fn size(&self) -> usize {
        self.size
    }

    /// The texture of `channel`; `None` if the graph has no output for it.
    pub fn channel(&self, channel: Channel) -> Option<&ChannelTexture> {
        self.channels.iter().find(|(c, _)| *c == channel).map(|(_, t)| t)
    }

    /// The channels the material sets.
    pub fn channels(&self) -> impl Iterator<Item = Channel> + '_ {
        self.channels.iter().map(|(c, _)| *c)
    }

    /// Memory held, in bytes.
    pub fn bytes(&self) -> usize {
        self.channels.iter().map(|(_, t)| t.bytes()).sum()
    }
}

//! Builders for small Painter stacks and an import with a fixed channel set.

use crate::model::*;
use crate::translate::channel_table::channel_spec;
use crate::translate::layers::{BlendTable, ImageSource, ImportedStack, LayerImport, NoImages, SetChannels};
use materialcraft::paint::{Channel, Compositor, TextureSet};

pub const BASE: u64 = 1;
pub const HEIGHT: u64 = 2;
pub const ROUGH: u64 = 128;
pub const ALL: u64 = u64::MAX;
pub const SIZE: (u32, u32) = (8, 8);

pub fn channels() -> SetChannels {
    SetChannels::new(&[
        channel_spec(Channel::BaseColor, Some(0)),
        channel_spec(Channel::Height, Some(5)),
        channel_spec(Channel::Roughness, Some(1)),
    ])
}

pub fn uniform(bit: u64, v: [f32; 3]) -> PainterSource {
    PainterSource::Uniform(UniformSource {
        uid: None,
        channels: ChannelMask(bit),
        color: v,
        opacity: 1.0,
    })
}

pub fn header(channels: u64, blending: Vec<ChannelBlending>) -> EffectHeader {
    EffectHeader {
        uid: None,
        enabled: true,
        label: String::new(),
        channels: ChannelMask(channels),
        blending,
    }
}

pub fn fill(sources: Vec<PainterSource>) -> PainterEffect {
    PainterEffect::Fill(FillEffect {
        header: header(ALL, vec![]),
        sources,
        projection: 0,
        filtering: 2,
        placement: Default::default(),
        uv_transform: None,
    })
}

pub fn blend(channels: u64, mode: u64, opacity: f32) -> ChannelBlending {
    ChannelBlending {
        channels: ChannelMask(channels),
        mode: PainterBlendMode::from_raw(mode),
        opacity,
    }
}

pub fn layer(uid: u64, name: &str, content: Vec<PainterEffect>, blending: Vec<ChannelBlending>) -> PainterLayer {
    PainterLayer {
        uid: Some(uid),
        name: name.into(),
        kind: LayerKind::Fill,
        visible: true,
        blending,
        mask: None,
        content,
    }
}

pub fn folder(uid: u64, name: &str, children: Vec<PainterLayer>, blending: Vec<ChannelBlending>) -> PainterLayer {
    PainterLayer {
        kind: LayerKind::Group(children),
        content: vec![],
        ..layer(uid, name, vec![], blending)
    }
}

pub fn procedural(bits: u64, name: &str) -> PainterSource {
    PainterSource::Procedural(ProceduralSource {
        uid: None,
        channels: ChannelMask(bits),
        mode: 1,
        procedural: Some(Procedural {
            uid: None,
            url: format!("resource://shelf/{name}?version=1.sbsar"),
            tweaks: vec![],
            outputs: vec![],
            inputs: vec![],
        }),
    })
}

pub fn set(layers: Vec<PainterLayer>) -> PainterTextureSet {
    PainterTextureSet {
        uid: Some(1),
        material_uid: Some(2),
        name: "Set".into(),
        resolution_log2: Some([3, 3]),
        channels: vec![],
        layers,
    }
}

pub fn import_with(layers: Vec<PainterLayer>, images: &dyn ImageSource) -> ImportedStack {
    let table = BlendTable::painter();
    LayerImport {
        table: &table,
        images,
        plan: &crate::translate::replacements::ProceduralPlan::default(),
    }
    .import(&set(layers), &channels(), SIZE)
}

pub fn import(layers: Vec<PainterLayer>) -> ImportedStack {
    import_with(layers, &NoImages)
}

/// The composite of one channel at texel (x, y), stored encoding.
pub fn composite(stack: &ImportedStack, channel: Channel, x: usize, y: usize) -> [f32; 4] {
    let mut set = TextureSet::new("t", "T", SIZE);
    set.channels = channels().specs().to_vec();
    set.layers = stack.layers.clone();
    let (tex, _) = Compositor::new(&set).composite_tex(channel);
    tex.px[y * tex.w + x]
}

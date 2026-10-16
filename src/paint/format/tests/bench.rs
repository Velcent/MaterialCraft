//! Size and speed of a realistic document: `cargo test --release bench -- --ignored --nocapture`.

use super::fixtures::Rng;
use super::round_trip::{from_bytes, to_bytes};
use crate::paint::{
    Channel, ChannelSpec, Document, Layer, LayerKind, PaintContent, TextureSet, TiledImage,
};
use std::time::Instant;

/// A paint layer image: smooth gradients with a little grain, fully opaque.
fn painted(spec: &ChannelSpec, size: u32, seed: u32) -> TiledImage {
    let mut img = TiledImage::new(size, size, spec.layer_format()).unwrap();
    let all = img.all_tiles();
    let s = size as f32;
    img.update_tiles(&all, |r, px| {
        let mut rng = Rng::new(seed ^ (r.x * 7 + r.y));
        for (i, p) in px.iter_mut().enumerate() {
            let (x, y) = (
                (r.x + i as u32 % r.w) as f32 / s,
                (r.y + i as u32 / r.w) as f32 / s,
            );
            let grain = (rng.unit() - 0.5) * 0.02;
            let v = 0.5 + 0.3 * (x * 9.0 + seed as f32).sin() * (y * 7.0).cos() + grain;
            *p = [v, (v * 0.8 + x * 0.2), (1.0 - v), 1.0];
        }
    });
    img
}

#[test]
#[ignore]
fn bench_two_sets_seven_channels_2k() {
    let size = 2048;
    let channels = [
        Channel::BaseColor,
        Channel::Roughness,
        Channel::Metallic,
        Channel::Normal,
        Channel::Height,
        Channel::AmbientOcclusion,
        Channel::Emissive,
    ];
    let mut doc = Document::default();
    for s in 0..2 {
        let mut set = TextureSet::new(format!("set{s}"), "Set", (size, size));
        set.channels = channels.iter().map(|c| ChannelSpec::new(*c)).collect();
        let images = set
            .channels
            .iter()
            .enumerate()
            .map(|(i, spec)| (spec.channel, painted(spec, size, (s * 10 + i) as u32)))
            .collect();
        set.layers.push(Layer::new(
            "paint",
            "Paint",
            LayerKind::Paint(PaintContent { images }),
        ));
        doc.texture_sets.push(set);
    }
    let raw = doc.image_memory_bytes();
    let t = Instant::now();
    let bytes = to_bytes(&doc);
    let save = t.elapsed();
    let t = Instant::now();
    let back = from_bytes(&bytes).unwrap();
    let load = t.elapsed();
    assert!(back == doc);
    println!(
        "2 sets x 7 channels at {size}²: {} MiB in memory, file {} MiB, save {save:.2?}, load {load:.2?}",
        raw >> 20,
        bytes.len() >> 20
    );
}

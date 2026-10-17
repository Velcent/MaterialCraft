//! [`core_content`]: turns what a layer's content gives each channel into our layer kind: a uniform
//! [`Fill`] when every computable channel is a constant, a [`PaintContent`] with images as soon as one
//! channel is a bitmap (the constants become filled images next to it).

use super::channels::SetChannels;
use super::content::{ChannelContent, ChannelValue};
use crate::model::LayerKind as PainterKind;
use materialcraft::paint::{Fill, LayerKind, PaintContent, TiledImage};

/// Our layer kind for a non-folder layer's evaluated content.
pub fn core_content(
    content: &[ChannelContent],
    painter: &PainterKind,
    channels: &SetChannels,
    resolution: (u32, u32),
) -> LayerKind {
    let has_image = content.iter().any(|c| matches!(c.value, ChannelValue::Image { .. }));
    if has_image {
        let mut paint = PaintContent::default();
        for c in content {
            match &c.value {
                ChannelValue::Image { image, .. } => paint.set_image(c.channel, image.clone()),
                ChannelValue::Uniform { value, .. } => {
                    let Some(spec) = channels.spec(c.channel) else { continue };
                    if let Ok(mut image) = TiledImage::new(resolution.0, resolution.1, spec.layer_format()) {
                        image.fill([value[0], value[1], value[2], 1.0]);
                        paint.set_image(c.channel, image);
                    }
                }
                ChannelValue::Nothing => {}
            }
        }
        return LayerKind::Paint(paint);
    }
    let values: Vec<_> = content
        .iter()
        .filter_map(|c| match c.value {
            ChannelValue::Uniform { value, .. } => Some((c.channel, value)),
            _ => None,
        })
        .collect();
    if values.is_empty() && *painter != PainterKind::Fill {
        LayerKind::Paint(PaintContent::default())
    } else {
        LayerKind::Fill(Fill::uniform(values))
    }
}

/// False for a normal value that is not the flat normal (images count as not flat).
pub fn is_flat_normal(value: &ChannelValue) -> bool {
    match value {
        ChannelValue::Uniform { value: v, .. } => {
            (v[0] - 0.5).abs() < 1e-3 && (v[1] - 0.5).abs() < 1e-3 && (v[2] - 1.0).abs() < 1e-3
        }
        ChannelValue::Image { .. } => false,
        ChannelValue::Nothing => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::translate::channel_table::channel_spec;
    use crate::translate::layers::computability::Computability;
    use materialcraft::paint::Channel;

    fn uniform(channel: Channel, v: f32) -> ChannelContent {
        ChannelContent {
            channel,
            value: ChannelValue::Uniform {
                value: [v, v, v, 1.0],
                coverage: 1.0,
            },
            status: Computability::Exact,
        }
    }

    #[test]
    fn constants_make_fills_and_images_make_paint_layers() {
        let channels = SetChannels::new(&[
            channel_spec(Channel::BaseColor, Some(0)),
            channel_spec(Channel::Roughness, Some(1)),
        ]);
        let fill = core_content(
            &[uniform(Channel::Roughness, 0.3)],
            &PainterKind::Paint,
            &channels,
            (4, 4),
        );
        assert!(
            matches!(&fill, LayerKind::Fill(f) if f.uniform_value(Channel::Roughness) == Some([0.3, 0.3, 0.3, 1.0]))
        );
        let empty = core_content(&[], &PainterKind::Paint, &channels, (4, 4));
        assert_eq!(empty, LayerKind::Paint(PaintContent::default()));
        let spec = channels.spec(Channel::BaseColor).unwrap();
        let image = TiledImage::new(4, 4, spec.layer_format()).unwrap();
        let both = [
            ChannelContent {
                channel: Channel::BaseColor,
                value: ChannelValue::Image { image, coverage: 1.0 },
                status: Computability::Exact,
            },
            uniform(Channel::Roughness, 0.25),
        ];
        let LayerKind::Paint(p) = core_content(&both, &PainterKind::Fill, &channels, (4, 4)) else {
            panic!("paint layer expected")
        };
        assert_eq!(p.images.len(), 2);
        assert!((p.image(Channel::Roughness).unwrap().get(3, 3)[0] - 0.25).abs() < 0.01);
        assert!(!is_flat_normal(&uniform(Channel::Normal, 0.5).value));
    }
}

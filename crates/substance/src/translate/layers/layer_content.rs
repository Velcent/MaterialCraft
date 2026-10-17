//! [`core_content`]: turns what a layer's content gives each channel into our layer kind: a uniform
//! [`Fill`] when every computable channel is a constant, an image fill (bitmaps laid on by their projection,
//! constants next to them) when the bitmaps share one projection, and a [`PaintContent`] of resampled bitmaps
//! otherwise (the constants become filled images next to them).

use super::channels::SetChannels;
use super::content::{ChannelContent, ChannelValue};
use super::images::resample;
use crate::model::LayerKind as PainterKind;
use materialcraft::paint::{Fill, FillSource, ImageFill, LayerKind, PaintContent, TiledImage};
use materialcraft::projection::Projection;

/// Our layer kind for a non-folder layer's evaluated content.
pub fn core_content(
    content: &[ChannelContent],
    painter: &PainterKind,
    channels: &SetChannels,
    resolution: (u32, u32),
) -> LayerKind {
    let projections: Vec<&Projection> = content
        .iter()
        .filter_map(|c| match &c.value {
            ChannelValue::Image { projection, .. } => Some(projection),
            _ => None,
        })
        .collect();
    if let Some(first) = projections.first()
        && projections.iter().all(|p| p == first)
    {
        return LayerKind::Fill(image_fill(content, (*first).clone()));
    }
    if !projections.is_empty() {
        let mut paint = PaintContent::default();
        for c in content {
            match &c.value {
                ChannelValue::Image { image, projection, .. } => {
                    if let Some(image) = resample(image, projection, resolution) {
                        paint.set_image(c.channel, image);
                    }
                }
                ChannelValue::Uniform { value, .. } => {
                    let Some(spec) = channels.spec(c.channel) else { continue };
                    if let Ok(mut image) = TiledImage::new(resolution.0, resolution.1, spec.layer_format()) {
                        image.fill([value[0], value[1], value[2], 1.0]);
                        paint.set_image(c.channel, image);
                    }
                }
                ChannelValue::Composited | ChannelValue::Nothing => {}
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

/// One image fill: the bitmaps of the channels that have one, the constants of the others.
fn image_fill(content: &[ChannelContent], projection: Projection) -> Fill {
    let mut images = ImageFill::default();
    for c in content {
        match &c.value {
            ChannelValue::Image { image, .. } => images.set_image(c.channel, image.clone()),
            ChannelValue::Uniform { value, .. } => images.set_value(c.channel, [value[0], value[1], value[2], 1.0]),
            ChannelValue::Composited | ChannelValue::Nothing => {}
        }
    }
    Fill::new(FillSource::Image(images)).with_projection(projection)
}

/// False for a normal value that is not the flat normal (images count as not flat).
pub fn is_flat_normal(value: &ChannelValue) -> bool {
    match value {
        ChannelValue::Uniform { value: v, .. } => {
            (v[0] - 0.5).abs() < 1e-3 && (v[1] - 0.5).abs() < 1e-3 && (v[2] - 1.0).abs() < 1e-3
        }
        ChannelValue::Image { .. } | ChannelValue::Composited => false,
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
        let image = |w: u32, projection: Projection| ChannelValue::Image {
            image: TiledImage::new(w, w, spec.layer_format()).unwrap(),
            coverage: 1.0,
            projection,
        };
        let content = |channel: Channel, value: ChannelValue| ChannelContent {
            channel,
            value,
            status: Computability::Exact,
        };
        // a bitmap and a constant stay one image fill, the bitmap at its own size
        let both = [
            content(Channel::BaseColor, image(8, Projection::default())),
            uniform(Channel::Roughness, 0.25),
        ];
        let LayerKind::Fill(f) = core_content(&both, &PainterKind::Fill, &channels, (4, 4)) else {
            panic!("image fill expected")
        };
        let FillSource::Image(images) = &f.source else {
            panic!("image fill expected")
        };
        assert_eq!(images.image(Channel::BaseColor).unwrap().image().size(), (8, 8));
        assert_eq!(images.value(Channel::Roughness), Some([0.25, 0.25, 0.25, 1.0]));
        // bitmaps placed differently are resampled into a paint layer
        let tiled = Projection::uv(materialcraft::projection::ImagePlacement {
            tiling: [2.0, 2.0],
            ..Default::default()
        });
        let apart = [
            content(Channel::BaseColor, image(8, Projection::default())),
            content(Channel::Roughness, image(2, tiled)),
        ];
        let LayerKind::Paint(p) = core_content(&apart, &PainterKind::Fill, &channels, (4, 4)) else {
            panic!("paint layer expected")
        };
        assert_eq!(p.images.len(), 2);
        assert_eq!(p.image(Channel::Roughness).unwrap().size(), (4, 4));
        assert!(!is_flat_normal(&uniform(Channel::Normal, 0.5).value));
    }
}

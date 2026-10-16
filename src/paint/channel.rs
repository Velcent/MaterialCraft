//! Material channels of a texture set, their colour space, default value and storage format.

use super::pixel::{PixelFormat, PixelLayout, SampleType};

/// Number of user channels (`User(0)` to `User(7)`, as in Substance Painter).
pub const USER_CHANNELS: u8 = 8;

/// One material channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Channel {
    BaseColor,
    Height,
    Roughness,
    Metallic,
    /// Tangent-space normal, OpenGL convention (+Y up), encoded as `n * 0.5 + 0.5`.
    Normal,
    Opacity,
    Emissive,
    AmbientOcclusion,
    /// User channel 0..[`USER_CHANNELS`].
    User(u8),
}

impl Channel {
    /// The standard (non-user) channels.
    pub const STANDARD: [Channel; 8] = [
        Channel::BaseColor,
        Channel::Height,
        Channel::Roughness,
        Channel::Metallic,
        Channel::Normal,
        Channel::Opacity,
        Channel::Emissive,
        Channel::AmbientOcclusion,
    ];

    /// Stable id used in files: `base_color`, `height`, …, `user0`.
    pub fn id(self) -> String {
        match self {
            Channel::BaseColor => "base_color".into(),
            Channel::Height => "height".into(),
            Channel::Roughness => "roughness".into(),
            Channel::Metallic => "metallic".into(),
            Channel::Normal => "normal".into(),
            Channel::Opacity => "opacity".into(),
            Channel::Emissive => "emissive".into(),
            Channel::AmbientOcclusion => "ambient_occlusion".into(),
            Channel::User(n) => format!("user{n}"),
        }
    }

    pub fn from_id(id: &str) -> Option<Channel> {
        if let Some(n) = id.strip_prefix("user") {
            let n: u8 = n.parse().ok()?;
            return (n < USER_CHANNELS && n.to_string() == id[4..]).then_some(Channel::User(n));
        }
        Channel::STANDARD.into_iter().find(|c| c.id() == id)
    }

    /// The value a channel has where no layer covers it (normalised RGBA).
    pub fn default_value(self) -> [f32; 4] {
        match self {
            Channel::BaseColor => [0.5, 0.5, 0.5, 1.0],
            Channel::Roughness => [0.5, 0.5, 0.5, 1.0],
            Channel::Normal => [0.5, 0.5, 1.0, 1.0],
            Channel::Opacity | Channel::AmbientOcclusion => [1.0, 1.0, 1.0, 1.0],
            Channel::Height | Channel::Metallic | Channel::Emissive | Channel::User(_) => {
                [0.0, 0.0, 0.0, 1.0]
            }
        }
    }

    /// True for colour channels (three components), false for scalar ones.
    pub fn is_color(self) -> bool {
        matches!(
            self,
            Channel::BaseColor | Channel::Normal | Channel::Emissive
        )
    }

    /// Storage format suggested for a new channel. Height is signed (f16), normals use 16 bits to avoid banding.
    pub fn default_format(self) -> PixelFormat {
        let (layout, sample) = match self {
            Channel::BaseColor | Channel::Emissive => (PixelLayout::Rgb, SampleType::U8),
            Channel::Normal => (PixelLayout::Rgb, SampleType::U16),
            Channel::Height => (PixelLayout::Gray, SampleType::F16),
            _ => (PixelLayout::Gray, SampleType::U8),
        };
        PixelFormat::new(layout, sample)
    }

    pub fn default_color_space(self) -> ColorSpace {
        match self {
            Channel::BaseColor | Channel::Emissive => ColorSpace::Srgb,
            _ => ColorSpace::Linear,
        }
    }
}

/// How stored values are encoded. Values are composited in their stored encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorSpace {
    Srgb,
    Linear,
}

impl ColorSpace {
    pub fn id(self) -> &'static str {
        match self {
            ColorSpace::Srgb => "srgb",
            ColorSpace::Linear => "linear",
        }
    }

    pub fn from_id(id: &str) -> Option<ColorSpace> {
        match id {
            "srgb" => Some(ColorSpace::Srgb),
            "linear" => Some(ColorSpace::Linear),
            _ => None,
        }
    }
}

/// A channel declared by a texture set. `format` is the value format without alpha; paint layers
/// store [`ChannelSpec::layer_format`] (the same with an alpha component for coverage).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelSpec {
    pub channel: Channel,
    pub format: PixelFormat,
    pub color_space: ColorSpace,
}

impl ChannelSpec {
    /// The channel with its default format and colour space.
    pub fn new(channel: Channel) -> ChannelSpec {
        ChannelSpec {
            channel,
            format: channel.default_format(),
            color_space: channel.default_color_space(),
        }
    }

    /// Format for a paint layer's image of this channel: the value format plus alpha (coverage).
    pub fn layer_format(&self) -> PixelFormat {
        self.format.with_alpha()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip() {
        for c in Channel::STANDARD
            .into_iter()
            .chain((0..USER_CHANNELS).map(Channel::User))
        {
            assert_eq!(Channel::from_id(&c.id()), Some(c));
        }
        assert_eq!(Channel::BaseColor.id(), "base_color");
        assert_eq!(Channel::User(3).id(), "user3");
        for bad in ["user8", "user", "user03", "user-1", "Base_Color", ""] {
            assert_eq!(Channel::from_id(bad), None, "{bad}");
        }
        assert_eq!(
            ColorSpace::from_id(ColorSpace::Srgb.id()),
            Some(ColorSpace::Srgb)
        );
    }

    #[test]
    fn defaults() {
        assert_eq!(Channel::Normal.default_value(), [0.5, 0.5, 1.0, 1.0]);
        assert_eq!(Channel::AmbientOcclusion.default_value()[0], 1.0);
        let s = ChannelSpec::new(Channel::Roughness);
        assert_eq!(s.format.id(), "gray8");
        assert_eq!(s.layer_format().id(), "graya8");
        assert_eq!(
            ChannelSpec::new(Channel::BaseColor).color_space,
            ColorSpace::Srgb
        );
        assert!(Channel::Height.default_format().sample.is_float());
    }
}

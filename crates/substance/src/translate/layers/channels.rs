//! [`SetChannels`]: the channels a texture set declares, seen from both sides: which of our channels a
//! Painter channel bit or `channelTypes` mask means, and how Painter's stored values become ours.

use crate::model::{ChannelMask, PainterChannel};
use crate::translate::channel_table::{channel_for_bit, painter_channel_name};
use materialcraft::paint::working_space::linear_to_srgb;
use materialcraft::paint::{Channel, ChannelSpec, ColorSpace};

/// The declared channels of one set, in declaration order.
#[derive(Debug, Clone)]
pub struct SetChannels {
    specs: Vec<ChannelSpec>,
}

impl SetChannels {
    pub fn new(specs: &[ChannelSpec]) -> SetChannels {
        SetChannels { specs: specs.to_vec() }
    }

    pub fn specs(&self) -> &[ChannelSpec] {
        &self.specs
    }

    pub fn all(&self) -> impl Iterator<Item = Channel> + '_ {
        self.specs.iter().map(|s| s.channel)
    }

    pub fn spec(&self, channel: Channel) -> Option<&ChannelSpec> {
        self.specs.iter().find(|s| s.channel == channel)
    }

    /// Our channel for a Painter channel, if the set declares it.
    pub fn ours(&self, channel: PainterChannel) -> Option<Channel> {
        channel_for_bit(channel.bit()).filter(|c| self.spec(*c).is_some())
    }

    /// The Painter channel of one of ours (the bit the cache and `channelTypes` use).
    pub fn painter(&self, channel: Channel) -> PainterChannel {
        (0..64)
            .map(|i| 1u64 << i)
            .find(|bit| channel_for_bit(*bit) == Some(channel))
            .map_or(PainterChannel::Bit(0), PainterChannel::from_bit)
    }

    /// True if `mask` (an effect's `channelTypes`) includes `channel`; "all" includes every channel.
    pub fn mask_has(&self, mask: ChannelMask, channel: Channel) -> bool {
        mask.is_all() || mask.contains(self.painter(channel))
    }

    /// Painter's name of one of our channels, for the report.
    pub fn label(&self, channel: Channel) -> String {
        painter_channel_name(self.painter(channel).bit())
    }

    /// A uniform value as Painter stores it (linear; grey channels repeat the value) in our stored encoding
    /// for `channel`: sRGB channels are encoded, everything else is kept.
    pub fn stored_value(&self, channel: Channel, linear: [f32; 3]) -> [f32; 4] {
        let srgb = self.spec(channel).is_some_and(|s| s.color_space == ColorSpace::Srgb);
        let encode = |v: f32| if srgb { linear_to_srgb(v.clamp(0.0, 1.0)) } else { v };
        let v = if channel.is_color() {
            linear.map(encode)
        } else {
            [encode(linear[0]); 3]
        };
        [v[0], v[1], v[2], 1.0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::translate::channel_table::channel_spec;

    fn set() -> SetChannels {
        SetChannels::new(&[
            channel_spec(Channel::BaseColor, Some(0)),
            channel_spec(Channel::Roughness, Some(1)),
            channel_spec(Channel::Height, Some(5)),
        ])
    }

    #[test]
    fn maps_declared_channels_both_ways() {
        let s = set();
        assert_eq!(s.ours(PainterChannel::Roughness), Some(Channel::Roughness));
        assert_eq!(s.ours(PainterChannel::Metallic), None, "not declared");
        assert_eq!(s.ours(PainterChannel::Diffuse), None, "no counterpart");
        assert_eq!(s.painter(Channel::Height), PainterChannel::Height);
        assert!(s.mask_has(ChannelMask(u64::MAX), Channel::BaseColor));
        assert!(!s.mask_has(ChannelMask(2), Channel::BaseColor));
        assert_eq!(s.label(Channel::BaseColor), "Base Color");
    }

    #[test]
    fn colours_are_encoded_and_grey_values_repeat() {
        let s = set();
        let base = s.stored_value(Channel::BaseColor, [0.5, 0.0, 1.0]);
        assert!((base[0] - 0.735_357).abs() < 1e-4 && base[1] == 0.0 && (base[2] - 1.0).abs() < 1e-4);
        assert_eq!(s.stored_value(Channel::Height, [-0.25; 3]), [-0.25, -0.25, -0.25, 1.0]);
        assert_eq!(s.stored_value(Channel::Roughness, [0.3, 0.9, 0.9])[2], 0.3);
    }
}

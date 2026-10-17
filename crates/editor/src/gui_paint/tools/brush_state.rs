//! [`BrushTools`]: the settings the brush panel and the quick-access bar edit: one preset per brush tool, the value
//! painted into each channel (and whether the channel is painted at all, PT-02) and the grey value for masks.

use super::super::shell::tools::Tool;
use materialcraft::paint::brush::BrushPreset;
use materialcraft::paint::{Channel, TextureSet};
use materialcraft_brush::PaintValues;

/// The value painted into one channel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChannelValue {
    pub channel: Channel,
    /// Straight RGBA in the channel's stored encoding.
    pub value: [f32; 4],
    pub enabled: bool,
}

/// Brush settings of the session.
#[derive(Debug, Clone, PartialEq)]
pub struct BrushTools {
    pub paint: BrushPreset,
    pub eraser: BrushPreset,
    /// Values by channel; channels a set has but this list lacks paint their default value.
    pub values: Vec<ChannelValue>,
    /// Grey value painted into masks (0 black … 1 white).
    pub mask_value: f32,
    /// Test occlusion (paint only what the view shows).
    pub occlusion: bool,
}

impl Default for BrushTools {
    fn default() -> BrushTools {
        BrushTools { paint: BrushPreset::default(), eraser: BrushPreset::eraser(), values: Vec::new(), mask_value: 1.0, occlusion: true }
    }
}

impl BrushTools {
    /// The preset of `tool`, if it is a brush tool this step provides (paint and eraser).
    pub fn preset(&self, tool: Tool) -> Option<&BrushPreset> {
        match tool {
            Tool::Paint => Some(&self.paint),
            Tool::Eraser => Some(&self.eraser),
            _ => None,
        }
    }

    pub fn preset_mut(&mut self, tool: Tool) -> Option<&mut BrushPreset> {
        match tool {
            Tool::Paint => Some(&mut self.paint),
            Tool::Eraser => Some(&mut self.eraser),
            _ => None,
        }
    }

    /// The entry for `channel`, created with the channel's default value when missing.
    pub fn value_mut(&mut self, channel: Channel) -> &mut ChannelValue {
        let at = match self.values.iter().position(|v| v.channel == channel) {
            Some(i) => i,
            None => {
                self.values.push(ChannelValue { channel, value: channel.default_value(), enabled: true });
                self.values.len() - 1
            }
        };
        let len = self.values.len();
        // `at` is in range: found above or just pushed
        &mut self.values[at.min(len - 1)]
    }

    /// The values to paint into `set`'s channels.
    pub fn values_for(&self, set: &TextureSet) -> PaintValues {
        let channels = set
            .channels
            .iter()
            .map(|spec| match self.values.iter().find(|v| v.channel == spec.channel) {
                Some(v) => (v.channel, v.value, v.enabled),
                None => (spec.channel, spec.channel.default_value(), true),
            })
            .collect();
        PaintValues { channels, mask: self.mask_value.clamp(0.0, 1.0) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_follow_the_set_and_keep_edits() {
        let set = TextureSet::new("s", "S", (4, 4)).with_channel(Channel::BaseColor).with_channel(Channel::Roughness);
        let mut b = BrushTools::default();
        b.value_mut(Channel::Roughness).value = [0.2; 4];
        b.value_mut(Channel::BaseColor).enabled = false;
        b.value_mut(Channel::Height).value = [0.9; 4];
        let v = b.values_for(&set);
        assert_eq!(v.channels.len(), 2);
        assert_eq!(v.value(Channel::Roughness), Some([0.2; 4]));
        assert_eq!(v.value(Channel::BaseColor), None, "switched off");
        assert_eq!(v.value(Channel::Height), None, "the set has no height");
        assert!(b.preset(Tool::Smudge).is_none() && b.preset(Tool::Eraser).is_some());
    }
}

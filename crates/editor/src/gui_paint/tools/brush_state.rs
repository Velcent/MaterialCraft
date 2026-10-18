//! [`BrushTools`]: the settings the brush panel and the quick-access bar edit: one preset per brush tool, the value
//! painted into each channel (and whether the channel is painted at all, PT-02), the grey value for masks, the
//! stroke helpers (lazy mouse, symmetry) and the retouch tools' settings ([`RetouchSettings`]).

use super::super::shell::tools::Tool;
use super::helpers::StrokeHelpers;
use super::polygon_fill::PolygonFillSettings;
use materialcraft::paint::brush::BrushPreset;
use materialcraft::paint::stroke::StrokeTool;
use materialcraft::paint::{Channel, TextureSet};
use materialcraft_brush::{PaintValues, SourceMode};

/// The value painted into one channel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChannelValue {
    pub channel: Channel,
    /// Straight RGBA in the channel's stored encoding.
    pub value: [f32; 4],
    pub enabled: bool,
}

/// What the smudge tool does (ADR 0008 mc2-q-pt-06c: smudge drags and mixes, blur is a separate brush mode).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SmudgeMode {
    /// Drags and mixes paint along the stroke (PT-23).
    #[default]
    Smudge,
    /// Softens the paint under the stroke (PT-26).
    Blur,
}

/// The smudge and clone tools' own settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RetouchSettings {
    pub smudge_mode: SmudgeMode,
    /// Smudge: how much paint each stamp drags along; blur: how much it softens. 0 … 1.
    pub strength: f32,
    /// Clone: aligned (the offset stays) or fixed (each stroke starts at the source again) (PT-28).
    pub clone_mode: SourceMode,
}

impl Default for RetouchSettings {
    fn default() -> RetouchSettings {
        RetouchSettings { smudge_mode: SmudgeMode::Smudge, strength: 0.6, clone_mode: SourceMode::Aligned }
    }
}

/// Brush settings of the session.
#[derive(Debug, Clone, PartialEq)]
pub struct BrushTools {
    pub paint: BrushPreset,
    pub eraser: BrushPreset,
    pub smudge: BrushPreset,
    pub clone: BrushPreset,
    pub retouch: RetouchSettings,
    /// Values by channel; channels a set has but this list lacks paint their default value.
    pub values: Vec<ChannelValue>,
    /// Grey value painted into masks (0 black … 1 white).
    pub mask_value: f32,
    /// Test occlusion (paint only what the view shows).
    pub occlusion: bool,
    /// Lazy mouse and symmetry, shared by the brush tools.
    pub helpers: StrokeHelpers,
    /// The polygon fill tool's settings.
    pub polygon: PolygonFillSettings,
}

impl Default for BrushTools {
    fn default() -> BrushTools {
        BrushTools {
            paint: BrushPreset::default(),
            eraser: BrushPreset::eraser(),
            smudge: BrushPreset { name: "Smudge".into(), hardness: 0.3, spacing: 0.1, ..BrushPreset::default() },
            clone: BrushPreset { name: "Clone".into(), hardness: 0.8, ..BrushPreset::default() },
            retouch: RetouchSettings::default(),
            values: Vec::new(),
            mask_value: 1.0,
            occlusion: true,
            helpers: StrokeHelpers::default(),
            polygon: PolygonFillSettings::default(),
        }
    }
}

impl BrushTools {
    /// The preset of `tool`, if it is a brush tool (paint, eraser, smudge, clone).
    pub fn preset(&self, tool: Tool) -> Option<&BrushPreset> {
        match tool {
            Tool::Paint => Some(&self.paint),
            Tool::Eraser => Some(&self.eraser),
            Tool::Smudge => Some(&self.smudge),
            Tool::Clone => Some(&self.clone),
            _ => None,
        }
    }

    pub fn preset_mut(&mut self, tool: Tool) -> Option<&mut BrushPreset> {
        match tool {
            Tool::Paint => Some(&mut self.paint),
            Tool::Eraser => Some(&mut self.eraser),
            Tool::Smudge => Some(&mut self.smudge),
            Tool::Clone => Some(&mut self.clone),
            _ => None,
        }
    }

    /// The preset of `tool` and the stroke helpers together, for the quick-access bar.
    pub fn quick_mut(&mut self, tool: Tool) -> Option<(&mut BrushPreset, &mut StrokeHelpers)> {
        let BrushTools { paint, eraser, smudge, clone, helpers, .. } = self;
        let preset = match tool {
            Tool::Paint => paint,
            Tool::Eraser => eraser,
            Tool::Smudge => smudge,
            Tool::Clone => clone,
            _ => return None,
        };
        Some((preset, helpers))
    }

    /// The stroke tool `tool` paints with (the smudge tool's mode picks smudge or blur).
    pub fn stroke_tool(&self, tool: Tool) -> Option<StrokeTool> {
        match tool {
            Tool::Paint => Some(StrokeTool::Paint),
            Tool::Eraser => Some(StrokeTool::Eraser),
            Tool::Smudge => Some(match self.retouch.smudge_mode {
                SmudgeMode::Smudge => StrokeTool::Smudge,
                SmudgeMode::Blur => StrokeTool::Blur,
            }),
            Tool::Clone => Some(StrokeTool::Clone),
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
        assert!(b.preset(Tool::Projection).is_none() && b.preset(Tool::Smudge).is_some() && b.preset(Tool::Clone).is_some());
    }

    #[test]
    fn the_smudge_tool_paints_smudge_or_blur_strokes() {
        let mut b = BrushTools::default();
        assert_eq!(b.stroke_tool(Tool::Smudge), Some(StrokeTool::Smudge));
        b.retouch.smudge_mode = SmudgeMode::Blur;
        assert_eq!(b.stroke_tool(Tool::Smudge), Some(StrokeTool::Blur));
        assert_eq!(b.stroke_tool(Tool::Clone), Some(StrokeTool::Clone));
        assert_eq!(b.stroke_tool(Tool::PolygonFill), None);
    }
}

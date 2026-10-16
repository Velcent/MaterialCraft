//! [`ResolvedBlend`]: the blend mode and opacity one channel gets from a stored `perChannelBlending` list,
//! with Painter's default filled in where a sparse list leaves the channel out.

use super::blend_table::{default_mode, BlendMapping, BlendTable, Holder};
use super::computability::Computability;
use crate::model::{blending_for, ChannelBlending, PainterBlendMode, PainterChannel};
use materialcraft::paint::BlendMode;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedBlend {
    /// As stored (or Painter's default for the channel).
    pub painter: PainterBlendMode,
    pub mapping: BlendMapping,
    /// 0..1.
    pub opacity: f32,
}

impl ResolvedBlend {
    /// The blending of `channel` in `entries` (a layer's, folder's or effect's list).
    pub fn of(entries: &[ChannelBlending], channel: PainterChannel, holder: Holder, table: &BlendTable) -> Self {
        match blending_for(entries, channel) {
            Some(b) => ResolvedBlend::new(b.mode, b.opacity, table),
            None => ResolvedBlend::new(default_mode(channel, holder), 1.0, table),
        }
    }

    /// The grey mask entry of a mask effect: the entry without channel bits, else the first one.
    pub fn of_mask(entries: &[ChannelBlending], table: &BlendTable) -> Self {
        match entries.iter().find(|b| b.channels.is_empty()).or(entries.first()) {
            Some(b) => ResolvedBlend::new(b.mode, b.opacity, table),
            None => ResolvedBlend::new(PainterBlendMode::Normal, 1.0, table),
        }
    }

    pub fn new(painter: PainterBlendMode, opacity: f32, table: &BlendTable) -> Self {
        ResolvedBlend {
            painter,
            mapping: table.map(painter),
            opacity: unit(opacity),
        }
    }

    pub fn mode(&self) -> BlendMode {
        self.mapping.mode
    }

    /// False when the entry leaves the channel untouched (disabled or at zero opacity).
    pub fn contributes(&self) -> bool {
        self.mode() != BlendMode::Disable && self.opacity > 0.0
    }

    /// Exact unless the mode is a guess.
    pub fn status(&self) -> Computability {
        match self.mapping.confidence.is_exact() {
            true => Computability::Exact,
            false => Computability::approximated(format!(
                "Painter blend {} imported as {} (not identified yet)",
                self.painter,
                self.mode().id()
            )),
        }
    }
}

/// Clamps an opacity to 0..1 (non-finite values from a corrupt file count as 1).
pub fn unit(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ChannelMask;

    fn entry(channels: u64, mode: u64, opacity: f32) -> ChannelBlending {
        ChannelBlending {
            channels: ChannelMask(channels),
            mode: PainterBlendMode::from_raw(mode),
            opacity,
        }
    }

    #[test]
    fn stored_entries_win_and_missing_ones_take_defaults() {
        let t = BlendTable::painter();
        let entries = [entry(1 | 16777216, 4, 0.5)];
        let base = ResolvedBlend::of(&entries, PainterChannel::BaseColor, Holder::Layer, &t);
        assert_eq!((base.mode(), base.opacity), (BlendMode::Multiply, 0.5));
        let height = ResolvedBlend::of(&entries, PainterChannel::Height, Holder::Layer, &t);
        assert_eq!(height.mode(), BlendMode::Add);
        assert!(height.status().is_exact());
        let normal = ResolvedBlend::of(&[], PainterChannel::Normal, Holder::Layer, &t);
        assert!(!normal.status().is_exact());
    }

    #[test]
    fn masks_read_the_grey_entry_and_bad_opacity_is_clamped() {
        let t = BlendTable::painter();
        let m = ResolvedBlend::of_mask(&[entry(1, 2, 1.0), entry(0, 4, 2.0)], &t);
        assert_eq!((m.mode(), m.opacity), (BlendMode::Multiply, 1.0));
        assert_eq!(unit(f32::NAN), 1.0);
        assert!(!ResolvedBlend::new(PainterBlendMode::Disable, 1.0, &t).contributes());
        assert!(!ResolvedBlend::new(PainterBlendMode::Normal, 0.0, &t).contributes());
    }
}

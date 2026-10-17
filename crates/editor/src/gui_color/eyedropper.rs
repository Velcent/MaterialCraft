//! [`Eyedropper`]: the armed state of the viewport eyedropper (CP-06, CP-07). It turns a click on the view into a
//! [`SampleRequest`] for the screen that owns the view: by default the displayed colour, with Shift the channel's
//! stored value. The screen answers with a [`Sample`] (or an error) through the picker.

use eframe::egui::{Context, CursorIcon, Key, Order, Pos2, Rect};
use materialcraft::paint::{Channel, ColorSpace};

use super::link::FieldRequest;

/// Where a sample goes.
#[derive(Clone, Debug, PartialEq)]
pub enum PickInto {
    /// The picker's current colour.
    Picker,
    /// Straight into a colour field (its small eyedropper button).
    Field(FieldRequest),
}

/// What the eyedropper reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The colour as the view shows it (lit, tone-mapped), read back from the GPU.
    Displayed,
    /// The channel's composited value at the texel under the pointer, exactly.
    Stored,
}

/// A click on the view to sample.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SampleRequest {
    /// Screen position in UI points.
    pub at: Pos2,
    pub source: Source,
    /// The channel a stored-value sample reads.
    pub channel: Channel,
}

/// A sampled colour: RGB in `space`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    pub rgb: [f32; 3],
    pub space: ColorSpace,
}

impl Sample {
    /// A displayed pixel as read back (display-encoded bytes).
    pub fn displayed(rgba: [u8; 4]) -> Sample {
        Sample { rgb: [rgba[0], rgba[1], rgba[2]].map(|v| f32::from(v) / 255.0), space: ColorSpace::Srgb }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Armed {
    into: PickInto,
    channel: Channel,
    /// A click was turned into a request; the eyedropper waits for its answer.
    waiting: bool,
}

#[derive(Default)]
pub struct Eyedropper {
    armed: Option<Armed>,
}

impl Eyedropper {
    /// Waits for a click on the view; a stored-value sample reads `channel`.
    pub fn arm(&mut self, into: PickInto, channel: Channel) {
        self.armed = Some(Armed { into, channel, waiting: false });
    }

    pub fn cancel(&mut self) {
        self.armed = None;
    }

    pub fn is_armed(&self) -> bool {
        self.armed.is_some()
    }

    /// True while armed for the picker itself (its button shows pressed).
    pub fn is_armed_for_picker(&self) -> bool {
        self.armed.as_ref().is_some_and(|a| a.into == PickInto::Picker)
    }

    /// This frame's click inside `view` as a sample request; Escape or a secondary click cancels. Shows a crosshair
    /// over the view while armed. Clicks on windows above the view (the picker itself) are not samples.
    pub fn poll_click(&mut self, ctx: &Context, view: Rect) -> Option<SampleRequest> {
        let armed = self.armed.as_mut()?;
        let (pos, primary, secondary, shift, escape) =
            ctx.input(|i| (i.pointer.hover_pos(), i.pointer.primary_pressed(), i.pointer.secondary_pressed(), i.modifiers.shift, i.key_pressed(Key::Escape)));
        if escape {
            self.armed = None;
            return None;
        }
        let on_view = pos.filter(|p| view.contains(*p) && ctx.layer_id_at(*p).is_none_or(|l| l.order == Order::Background));
        let Some(pos) = on_view else { return None };
        ctx.set_cursor_icon(CursorIcon::Crosshair);
        if secondary {
            self.armed = None;
            return None;
        }
        if !primary || armed.waiting {
            return None;
        }
        armed.waiting = true;
        Some(SampleRequest { at: pos, source: if shift { Source::Stored } else { Source::Displayed }, channel: armed.channel })
    }

    /// Where the requested sample goes; disarms.
    pub fn finish(&mut self) -> Option<PickInto> {
        self.armed.take().map(|a| a.into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arming_finishing_and_cancelling() {
        let mut e = Eyedropper::default();
        assert!(!e.is_armed() && e.finish().is_none());
        e.arm(PickInto::Picker, Channel::Roughness);
        assert!(e.is_armed_for_picker());
        assert_eq!(e.finish(), Some(PickInto::Picker));
        assert!(!e.is_armed());
        e.arm(PickInto::Picker, Channel::BaseColor);
        e.cancel();
        assert!(!e.is_armed());
    }

    #[test]
    fn displayed_samples_are_display_encoded() {
        let s = Sample::displayed([255, 128, 0, 255]);
        assert_eq!(s.space, ColorSpace::Srgb);
        assert_eq!(s.rgb, [1.0, 128.0 / 255.0, 0.0]);
    }
}

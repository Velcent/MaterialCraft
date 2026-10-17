//! How colour fields and the one colour picker talk without holding references to each other: through the egui
//! context's temporary data, as egui's own popups do. A field posts a request ([`request_open`], [`request_pick`]),
//! the picker takes it at its next frame and delivers edits back by the field's id ([`deliver`]), which the field
//! takes when it is drawn ([`take_delivered`]). A field also stamps the frame it was last drawn in ([`mark_seen`]),
//! so the picker can close when its field is gone (CP-01).

use eframe::egui::{Context, Id};
use materialcraft::paint::Channel;

use super::color::FieldColor;

/// A colour field asking for the picker or the eyedropper.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldRequest {
    /// The field's id; edits come back under it.
    pub key: Id,
    /// The field's colour when asked.
    pub color: FieldColor,
    /// The channel a stored-value pick (Shift) reads.
    pub channel: Channel,
    /// Shown in the picker's title.
    pub label: String,
}

fn open_id() -> Id {
    Id::new("gui_color.open")
}

fn pick_id() -> Id {
    Id::new("gui_color.pick")
}

fn delivered_id(key: Id) -> Id {
    key.with("gui_color.delivered")
}

fn seen_id(key: Id) -> Id {
    key.with("gui_color.seen")
}

/// Removes and returns the value stored under `id`.
fn take<T: Clone + Send + Sync + 'static>(ctx: &Context, id: Id) -> Option<T> {
    ctx.data_mut(|d| {
        let v = d.get_temp::<T>(id);
        d.remove::<T>(id);
        v
    })
}

/// Asks the picker to open for a field (the last request of a frame wins).
pub fn request_open(ctx: &Context, req: FieldRequest) {
    ctx.data_mut(|d| d.insert_temp(open_id(), req));
}

pub fn take_open(ctx: &Context) -> Option<FieldRequest> {
    take(ctx, open_id())
}

/// Asks for an eyedropper pick straight into a field, without opening the picker (CP-07).
#[allow(dead_code)] // used by ColorField
pub fn request_pick(ctx: &Context, req: FieldRequest) {
    ctx.data_mut(|d| d.insert_temp(pick_id(), req));
}

pub fn take_pick(ctx: &Context) -> Option<FieldRequest> {
    take(ctx, pick_id())
}

/// Hands a new colour to the field `key`.
pub fn deliver(ctx: &Context, key: Id, color: FieldColor) {
    ctx.data_mut(|d| d.insert_temp(delivered_id(key), color));
}

/// The colour delivered to field `key` since it was last drawn.
#[allow(dead_code)] // used by ColorField
pub fn take_delivered(ctx: &Context, key: Id) -> Option<FieldColor> {
    take(ctx, delivered_id(key))
}

/// Stamps field `key` as drawn in this frame.
#[allow(dead_code)] // used by ColorField
pub fn mark_seen(ctx: &Context, key: Id) {
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| d.insert_temp(seen_id(key), pass));
}

/// The pass field `key` was last drawn in.
pub fn last_seen(ctx: &Context, key: Id) -> Option<u64> {
    ctx.data(|d| d.get_temp::<u64>(seen_id(key)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft::paint::ColorSpace;

    #[test]
    fn requests_and_deliveries_are_taken_once() {
        let ctx = Context::default();
        let key = Id::new("brush colour");
        let req = FieldRequest { key, color: FieldColor::new([0.1, 0.2, 0.3, 1.0], ColorSpace::Srgb), channel: Channel::BaseColor, label: "Brush".into() };
        request_open(&ctx, req.clone());
        assert_eq!(take_open(&ctx), Some(req.clone()));
        assert_eq!(take_open(&ctx), None);
        request_pick(&ctx, req.clone());
        assert_eq!(take_pick(&ctx), Some(req));
        let c = FieldColor::new([1.0, 0.0, 0.0, 1.0], ColorSpace::Linear);
        deliver(&ctx, key, c);
        assert_eq!(take_delivered(&ctx, Id::new("other")), None);
        assert_eq!(take_delivered(&ctx, key), Some(c));
        assert_eq!(take_delivered(&ctx, key), None);
        assert_eq!(last_seen(&ctx, key), None);
        mark_seen(&ctx, key);
        assert_eq!(last_seen(&ctx, key), Some(ctx.cumulative_pass_nr()));
    }
}

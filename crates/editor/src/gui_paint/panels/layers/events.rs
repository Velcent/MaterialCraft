//! [`LayerEvent`]: what the layer panel asks of the document. `PaintMode` hands these to the
//! [`crate::gui_paint::stack_editor`], which makes each one a history step through `paint::edit`.

use materialcraft::history::CoalesceKey;
use materialcraft::paint::edit::ops::{DropPayload, DropTarget, MaskStart, NewEffect, NewLayer};
use materialcraft::paint::edit::{LayerRef, LayerSlot, StackId};
use materialcraft::paint::{BlendMode, Channel};

#[derive(Clone, Debug, PartialEq)]
pub enum LayerEvent {
    /// A new layer directly above `above` (top of the stack without one).
    Create {
        set: String,
        what: NewLayer,
        above: Option<String>,
    },
    Duplicate {
        set: String,
        ids: Vec<String>,
    },
    Delete {
        set: String,
        ids: Vec<String>,
    },
    Group {
        set: String,
        ids: Vec<String>,
    },
    /// Move the layers to `to` (counted before the move), or with `copy` place copies there.
    Move {
        set: String,
        ids: Vec<String>,
        to: LayerSlot,
        copy: bool,
    },
    Rename(LayerRef, String),
    SetVisible(LayerRef, bool),
    /// The opacity of the layer's entry for `channel`; events with the same key are one step (a drag).
    SetOpacity {
        layer: LayerRef,
        channel: Channel,
        opacity: f32,
        key: CoalesceKey,
    },
    SetBlendMode {
        layer: LayerRef,
        channel: Channel,
        mode: BlendMode,
    },
    /// The end of a slider drag: the next change starts a new step.
    EndGesture,
    AddEffect {
        layer: LayerRef,
        stack: StackId,
        what: NewEffect,
    },
    RemoveEffect {
        layer: LayerRef,
        stack: StackId,
        index: usize,
    },
    SetEffectEnabled {
        layer: LayerRef,
        stack: StackId,
        index: usize,
        enabled: bool,
    },
    AddMask(LayerRef, MaskStart),
    RemoveMask(LayerRef),
    SetMaskEnabled(LayerRef, bool),
    InvertMask(LayerRef),
    /// Show only this layer's mask in the viewport (`None`: show the material again). View state only.
    Isolate(Option<LayerRef>),
    /// An asset dropped onto the stack; `replace` is Ctrl. Sent by the asset tray (step 15).
    #[allow(dead_code)]
    Drop {
        target: DropTarget,
        payload: DropPayload,
        replace: bool,
    },
}

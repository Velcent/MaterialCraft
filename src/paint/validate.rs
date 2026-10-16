//! Document validation: the invariants [`crate::paint::Document::validate`] checks, as [`DocError`]s.

use super::channel::Channel;
use super::doc::Document;
use super::image::MAX_DIMENSION;
use super::layer::{FillSource, Layer, LayerKind};
use super::texture_set::TextureSet;
use std::fmt;

/// Deepest allowed group nesting (top-level layers are depth 0).
pub const MAX_GROUP_DEPTH: usize = 32;

/// A broken document invariant. `set` and `layer` are ids.
#[derive(Debug, Clone, PartialEq)]
pub enum DocError {
    EmptyId {
        set: String,
    },
    DuplicateTextureSet {
        set: String,
    },
    BadResolution {
        set: String,
        resolution: (u32, u32),
    },
    MaterialSlotOutOfRange {
        set: String,
        slot: usize,
    },
    UvSetOutOfRange {
        set: String,
        uv_set: usize,
    },
    DuplicateChannel {
        set: String,
        channel: Channel,
    },
    DuplicateLayer {
        set: String,
        layer: String,
    },
    /// A layer uses a channel the texture set does not declare.
    UndeclaredChannel {
        set: String,
        layer: String,
        channel: Channel,
    },
    /// A layer has two blend entries, paint images or fill values for one channel.
    DuplicateLayerChannel {
        set: String,
        layer: String,
        channel: Channel,
    },
    /// Opacity not in 0..=1.
    BadOpacity {
        set: String,
        layer: String,
    },
    /// A paint image (`channel` = Some) or mask (`None`) does not match the texture set resolution.
    ImageSize {
        set: String,
        layer: String,
        channel: Option<Channel>,
        size: (u32, u32),
        expected: (u32, u32),
    },
    GroupTooDeep {
        set: String,
        layer: String,
    },
    DuplicateForeignFile {
        path: String,
    },
}

impl fmt::Display for DocError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use DocError::*;
        match self {
            EmptyId { set } => write!(f, "texture set {set:?}: empty texture set or layer id"),
            DuplicateTextureSet { set } => write!(f, "texture set id {set:?} is used twice"),
            BadResolution {
                set,
                resolution: (w, h),
            } => write!(
                f,
                "texture set {set:?}: resolution {w}x{h} must be 1..={MAX_DIMENSION}"
            ),
            MaterialSlotOutOfRange { set, slot } => {
                write!(f, "texture set {set:?}: mesh has no material slot {slot}")
            }
            UvSetOutOfRange { set, uv_set } => {
                write!(f, "texture set {set:?}: mesh part lacks UV set {uv_set}")
            }
            DuplicateChannel { set, channel } => write!(
                f,
                "texture set {set:?}: channel {} declared twice",
                channel.id()
            ),
            DuplicateLayer { set, layer } => {
                write!(f, "texture set {set:?}: layer id {layer:?} is used twice")
            }
            UndeclaredChannel {
                set,
                layer,
                channel,
            } => {
                write!(
                    f,
                    "texture set {set:?}, layer {layer:?}: channel {} is not declared",
                    channel.id()
                )
            }
            DuplicateLayerChannel {
                set,
                layer,
                channel,
            } => {
                write!(
                    f,
                    "texture set {set:?}, layer {layer:?}: channel {} appears twice",
                    channel.id()
                )
            }
            BadOpacity { set, layer } => write!(
                f,
                "texture set {set:?}, layer {layer:?}: opacity must be within 0..=1"
            ),
            ImageSize {
                set,
                layer,
                channel,
                size,
                expected,
            } => {
                let what = channel.map_or("mask".to_string(), |c| c.id());
                write!(
                    f,
                    "texture set {set:?}, layer {layer:?}: {what} image is {}x{}, expected {}x{}",
                    size.0, size.1, expected.0, expected.1
                )
            }
            GroupTooDeep { set, layer } => write!(
                f,
                "texture set {set:?}, layer {layer:?}: groups nested deeper than {MAX_GROUP_DEPTH}"
            ),
            DuplicateForeignFile { path } => write!(f, "foreign file {path:?} is stored twice"),
        }
    }
}

impl std::error::Error for DocError {}

/// Every problem of `doc`, in document order.
pub(crate) fn problems(doc: &Document) -> Vec<DocError> {
    let mut out = Vec::new();
    for (i, set) in doc.texture_sets.iter().enumerate() {
        if doc.texture_sets[..i].iter().any(|s| s.id == set.id) {
            out.push(DocError::DuplicateTextureSet {
                set: set.id.clone(),
            });
        }
        check_set(doc, set, &mut out);
    }
    for (i, f) in doc.foreign.iter().enumerate() {
        if doc.foreign[..i].iter().any(|g| g.path == f.path) {
            out.push(DocError::DuplicateForeignFile {
                path: f.path.clone(),
            });
        }
    }
    out
}

fn check_set(doc: &Document, set: &TextureSet, out: &mut Vec<DocError>) {
    let sid = || set.id.clone();
    if set.id.is_empty() {
        out.push(DocError::EmptyId { set: sid() });
    }
    let (w, h) = set.resolution;
    if w == 0 || h == 0 || w > MAX_DIMENSION || h > MAX_DIMENSION {
        out.push(DocError::BadResolution {
            set: sid(),
            resolution: set.resolution,
        });
    }
    if !doc.mesh.materials.is_empty() && set.material_slot >= doc.mesh.materials.len() {
        out.push(DocError::MaterialSlotOutOfRange {
            set: sid(),
            slot: set.material_slot,
        });
    }
    if doc
        .mesh
        .parts
        .iter()
        .any(|p| p.material == set.material_slot && set.uv_set >= p.uv_sets.len())
    {
        out.push(DocError::UvSetOutOfRange {
            set: sid(),
            uv_set: set.uv_set,
        });
    }
    for (i, spec) in set.channels.iter().enumerate() {
        if set.channels[..i].iter().any(|s| s.channel == spec.channel) {
            out.push(DocError::DuplicateChannel {
                set: sid(),
                channel: spec.channel,
            });
        }
    }
    let mut seen: Vec<&str> = Vec::new();
    for (depth, layer) in set.walk_layers() {
        if layer.id.is_empty() {
            out.push(DocError::EmptyId { set: sid() });
        } else if seen.contains(&layer.id.as_str()) {
            out.push(DocError::DuplicateLayer {
                set: sid(),
                layer: layer.id.clone(),
            });
        }
        seen.push(&layer.id);
        if depth > MAX_GROUP_DEPTH {
            out.push(DocError::GroupTooDeep {
                set: sid(),
                layer: layer.id.clone(),
            });
        }
        check_layer(set, layer, out);
    }
}

fn check_layer(set: &TextureSet, layer: &Layer, out: &mut Vec<DocError>) {
    let err_ctx = || (set.id.clone(), layer.id.clone());
    let unit = |v: f32| (0.0..=1.0).contains(&v);
    if !unit(layer.opacity) || layer.blend.iter().any(|b| !unit(b.opacity)) {
        let (set, layer) = err_ctx();
        out.push(DocError::BadOpacity { set, layer });
    }
    let mut channels: Vec<Vec<Channel>> = vec![layer.blend.iter().map(|b| b.channel).collect()];
    match &layer.kind {
        LayerKind::Paint(p) => channels.push(p.images.iter().map(|(c, _)| *c).collect()),
        LayerKind::Fill(f) => {
            if let FillSource::Uniform(v) = &f.source {
                channels.push(v.iter().map(|(c, _)| *c).collect());
            }
        }
        LayerKind::Group(_) => {}
    }
    for list in &channels {
        for (i, &c) in list.iter().enumerate() {
            let (s, l) = err_ctx();
            if !set.has_channel(c) {
                out.push(DocError::UndeclaredChannel {
                    set: s,
                    layer: l,
                    channel: c,
                });
            } else if list[..i].contains(&c) {
                out.push(DocError::DuplicateLayerChannel {
                    set: s,
                    layer: l,
                    channel: c,
                });
            }
        }
    }
    for (channel, img) in layer.images() {
        if img.size() != set.resolution {
            let (s, l) = err_ctx();
            out.push(DocError::ImageSize {
                set: s,
                layer: l,
                channel,
                size: img.size(),
                expected: set.resolution,
            });
        }
    }
}

#[cfg(test)]
#[path = "validate_tests.rs"]
mod tests;

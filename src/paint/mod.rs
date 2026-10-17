//! Paint documents: layered, per-channel textures on a mesh (Paint mode). See the internal docs,
//! `architecture/paint-mode.md`.
//!
//! - [`pixel`]: sample types and pixel formats.
//! - [`image`]: [`TiledImage`], lazily allocated tiles with dirty tracking.
//! - [`channel`]: material channels, colour spaces, [`ChannelSpec`].
//! - [`blend`] and [`normal`]: blend modes and normal-vector maths.
//! - [`layer`], [`effect`], [`mask`], [`geometry_mask`], [`texture_set`], [`doc`], [`report`]: the document
//!   model; [`validate`] its invariants.
//! - [`edit`]: [`edit::Edit`], every document change with its inverse (for the GUI and the history).
//! - [`stack`]: [`Compositor`], layer stack → channel.
//! - [`diag_entries`]: import summaries and compositing warnings as session log entries.
//! - [`working_space`]: the space layers blend in (linear light for sRGB channels).
//! - [`output`]: [`MaterialMaps`], final PBR maps for the viewer and export.

pub mod blend;
pub mod channel;
pub mod diag_entries;
pub mod doc;
pub mod edit;
pub mod effect;
pub mod format;
pub mod geometry_mask;
pub mod height_normal;
pub mod image;
pub mod layer;
pub mod mask;
pub mod normal;
pub mod normal_combine;
pub mod output;
mod par;
pub mod pixel;
pub mod report;
pub mod stack;
pub mod texture_set;
pub mod validate;
pub mod working_space;

pub use blend::BlendMode;
pub use channel::{Channel, ChannelSpec, ColorSpace};
pub use doc::{Document, ForeignFile};
pub use effect::{ContentEffect, Effect, EffectKind, EffectRegistry, GreyBlend, MaskEffect, MaskPixels};
pub use geometry_mask::GeometryMask;
pub use image::{ImageError, Rect, TILE_SIZE, Tile, TiledImage};
pub use layer::{ChannelBlend, Fill, FillSource, Layer, LayerImage, LayerKind, PaintContent};
pub use mask::Mask;
pub use output::{MaterialMaps, OutputOptions};
pub use pixel::{PixelFormat, PixelLayout, SampleType};
pub use report::{Fidelity, ImportReport, ReportEntry};
pub use stack::{CompositeWarning, Compositor};
pub use texture_set::{BakedKind, BakedMap, TextureSet};
pub use validate::DocError;

//! [`LibraryCandidate`]: one thing an imported project brings that can live in the user's asset library, and
//! [`LibraryCandidates`], the project's list.

use super::origin::Origin;
use materialcraft::assets::Usage;
use materialcraft::paint::brush::BrushPreset;

/// What a candidate becomes in the library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CandidateKind {
    /// A brush preset made from the brush settings of the project's strokes.
    Brush,
    /// A bitmap brush alpha.
    Alpha,
    /// A bitmap stencil (a texture in the library, tagged `stencil`).
    Stencil,
    /// A bitmap used as a fill, as paint colour or as a procedural's input.
    Texture,
    /// The viewer's environment map.
    Environment,
}

impl CandidateKind {
    pub const ALL: [CandidateKind; 5] = [
        CandidateKind::Brush,
        CandidateKind::Alpha,
        CandidateKind::Stencil,
        CandidateKind::Texture,
        CandidateKind::Environment,
    ];

    /// The library usage it is written as.
    pub fn usage(self) -> Usage {
        match self {
            CandidateKind::Brush => Usage::Brush,
            CandidateKind::Alpha => Usage::Alpha,
            CandidateKind::Stencil | CandidateKind::Texture => Usage::Texture,
            CandidateKind::Environment => Usage::Environment,
        }
    }

    /// Lowercase word for the review and the asset's tags.
    pub fn label(self) -> &'static str {
        match self {
            CandidateKind::Brush => "brush",
            CandidateKind::Alpha => "alpha",
            CandidateKind::Stencil => "stencil",
            CandidateKind::Texture => "texture",
            CandidateKind::Environment => "environment",
        }
    }
}

/// What a candidate is made from.
#[derive(Debug, Clone, PartialEq)]
pub enum CandidateSource {
    /// An image resource by its URL (stored in the project).
    Image { url: String },
    /// A brush preset; an alpha-image tip names the [`LibraryCandidate::key`] of its alpha, which is written with it.
    Brush { preset: BrushPreset, alpha: Option<String> },
}

/// One item the review offers ("Add to my library").
#[derive(Debug, Clone, PartialEq)]
pub struct LibraryCandidate {
    /// Unique within the project's list (the review's choices and the export refer to it).
    pub key: String,
    pub kind: CandidateKind,
    /// The display name it gets in the library.
    pub name: String,
    pub origin: Origin,
    /// How many effects use it.
    pub uses: usize,
    pub source: CandidateSource,
}

impl LibraryCandidate {
    /// False for vendor content: "from this project, not shareable".
    pub fn shareable(&self) -> bool {
        self.origin.shareable()
    }

    /// The key of the alpha a brush brings along, if any.
    pub fn brings(&self) -> Option<&str> {
        match &self.source {
            CandidateSource::Brush { alpha, .. } => alpha.as_deref(),
            CandidateSource::Image { .. } => None,
        }
    }
}

/// A project's library candidates.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LibraryCandidates {
    /// The project file, as its provenance records it.
    pub project: String,
    /// Brushes first, then alphas, stencils, textures and the environment; most used first within a kind.
    pub items: Vec<LibraryCandidate>,
    /// What was seen but not offered, and why (one line each, counted).
    pub left_out: Vec<String>,
}

impl LibraryCandidates {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn get(&self, key: &str) -> Option<&LibraryCandidate> {
        self.items.iter().find(|c| c.key == key)
    }

    /// The candidates of `kind`, in list order.
    pub fn of_kind(&self, kind: CandidateKind) -> impl Iterator<Item = &LibraryCandidate> {
        self.items.iter().filter(move |c| c.kind == kind)
    }

    /// The candidates writing `keys` needs: those keys plus the alphas their brushes bring, in list order (images
    /// before the brushes that use them, as the list keeps brushes first only for showing).
    pub fn closure(&self, keys: &[String]) -> Vec<&LibraryCandidate> {
        let wanted = |c: &LibraryCandidate| {
            keys.contains(&c.key)
                || self
                    .items
                    .iter()
                    .any(|b| keys.contains(&b.key) && b.brings() == Some(c.key.as_str()))
        };
        let (brushes, images): (Vec<&LibraryCandidate>, Vec<&LibraryCandidate>) = self
            .items
            .iter()
            .filter(|c| wanted(c))
            .partition(|c| c.kind == CandidateKind::Brush);
        images.into_iter().chain(brushes).collect()
    }
}

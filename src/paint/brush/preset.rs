//! [`BrushPreset`]: the settings of a brush tool in MaterialCraft's own terms (PT-04 … PT-11), with no engine type
//! in sight. The brush crate converts it to its engine's settings.

/// What the brush size is measured in (PT-10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeSpace {
    /// On the model: a fraction of the mesh's bounding radius, so a stamp keeps its size on the mesh when the
    /// camera moves (the default).
    Model,
    /// Screen units (UI points): constant on screen, whatever the camera does.
    Screen,
    /// Texels of the texture set (2D view strokes).
    Texture,
}

impl SizeSpace {
    pub const ALL: [SizeSpace; 3] = [SizeSpace::Model, SizeSpace::Screen, SizeSpace::Texture];

    pub fn id(self) -> &'static str {
        match self {
            SizeSpace::Model => "model",
            SizeSpace::Screen => "screen",
            SizeSpace::Texture => "texture",
        }
    }

    pub fn from_id(id: &str) -> Option<SizeSpace> {
        SizeSpace::ALL.into_iter().find(|s| s.id() == id)
    }
}

/// How a stamp is laid onto the surface (PT-08).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// The stamp is a screen-space shape projected along the view.
    FacingView,
    /// The stamp follows the surface around curvature (the default): each texel is placed by its offset from the
    /// stamp's centre in the tangent plane there, without foreshortening.
    Wrapped,
    /// As wrapped, but the stamp lies in the tangent plane at its centre and fades where the surface leaves it.
    Flat,
    /// The stamp is laid out in texture space around the texel under its centre.
    UvAligned,
}

impl Placement {
    pub const ALL: [Placement; 4] = [Placement::FacingView, Placement::Wrapped, Placement::Flat, Placement::UvAligned];

    pub fn id(self) -> &'static str {
        match self {
            Placement::FacingView => "view",
            Placement::Wrapped => "wrapped",
            Placement::Flat => "flat",
            Placement::UvAligned => "uv",
        }
    }

    pub fn from_id(id: &str) -> Option<Placement> {
        Placement::ALL.into_iter().find(|p| p.id() == id)
    }
}

/// The shape of every stamp (PT-11, AS-06).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Tip {
    /// A round tip whose edge softness is the preset's hardness.
    #[default]
    Round,
    /// A built-in procedural tip (chalk, spatter, bristles …) by its id: the brush crate renders it, so the preset
    /// needs no image and a stroke painted with it replays without the library.
    Procedural(String),
    /// A greyscale alpha image (white paints) by its library-relative path (`alphas/scratch.png`), looked up in the
    /// asset libraries like material references.
    Alpha(String),
}

impl Tip {
    /// The tip's kind as a preset writes it: `round`, `procedural` or `alpha`.
    pub fn kind(&self) -> &'static str {
        match self {
            Tip::Round => "round",
            Tip::Procedural(_) => "procedural",
            Tip::Alpha(_) => "alpha",
        }
    }

    /// The procedural tip's id or the alpha's reference; `None` for the round tip.
    pub fn reference(&self) -> Option<&str> {
        match self {
            Tip::Round => None,
            Tip::Procedural(r) | Tip::Alpha(r) => Some(r),
        }
    }

    /// The tip of `kind` with `reference`; `None` for an unknown kind, or a procedural or alpha tip without one.
    pub fn from_parts(kind: &str, reference: Option<&str>) -> Option<Tip> {
        let reference = reference.filter(|r| !r.trim().is_empty()).map(str::to_string);
        match kind {
            "round" => Some(Tip::Round),
            "procedural" => reference.map(Tip::Procedural),
            "alpha" => reference.map(Tip::Alpha),
            _ => None,
        }
    }
}

/// Per-stamp random variation, 0 (none) … 1 (full), seeded per stroke (PT-06).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Jitter {
    pub size: f32,
    pub flow: f32,
    pub angle: f32,
    /// Offset of each stamp from the path, in brush diameters.
    pub position: f32,
}

/// What pen pressure drives (PT-07); a mouse acts as full pressure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PressureUse {
    pub size: bool,
    pub flow: bool,
}

impl Default for PressureUse {
    fn default() -> PressureUse {
        PressureUse { size: true, flow: false }
    }
}

/// A brush tool's settings. Values outside their ranges are clamped where they are used ([`BrushPreset::clamped`]).
#[derive(Debug, Clone, PartialEq)]
pub struct BrushPreset {
    pub name: String,
    /// Stamp diameter in [`BrushPreset::size_space`] units: a fraction of the mesh radius for [`SizeSpace::Model`],
    /// screen units for [`SizeSpace::Screen`], texels for [`SizeSpace::Texture`].
    pub size: f32,
    pub size_space: SizeSpace,
    pub tip: Tip,
    /// Edge softness of a round tip, 0 (soft) … 1 (hard).
    pub hardness: f32,
    /// Coverage each stamp adds, 0 … 1 (PT-04).
    pub flow: f32,
    /// The most coverage the whole stroke reaches, 0 … 1; applied over the finished stroke, never driven by
    /// pressure (PT-04).
    pub opacity: f32,
    /// Distance between stamps as a fraction of the diameter (PT-04).
    pub spacing: f32,
    /// Stamp angle in degrees, counter-clockwise (PT-05).
    pub angle: f32,
    /// Turn each stamp along the stroke direction; a lone click then paints nothing (PT-05).
    pub follow_direction: bool,
    /// 1 = round, smaller = squashed.
    pub roundness: f32,
    pub jitter: Jitter,
    pub pressure: PressureUse,
    pub placement: Placement,
    /// Surfaces turned away from the view by more than this many degrees are not painted (PT-09).
    pub max_angle: f32,
}

impl Default for BrushPreset {
    fn default() -> BrushPreset {
        BrushPreset {
            name: "Basic hard".into(),
            size: 0.08,
            size_space: SizeSpace::Model,
            tip: Tip::Round,
            hardness: 0.8,
            flow: 1.0,
            opacity: 1.0,
            spacing: 0.1,
            angle: 0.0,
            follow_direction: false,
            roundness: 1.0,
            jitter: Jitter::default(),
            pressure: PressureUse::default(),
            placement: Placement::Wrapped,
            max_angle: 85.0,
        }
    }
}

impl BrushPreset {
    /// The eraser's default: the same tip, soft enough to blend out.
    pub fn eraser() -> BrushPreset {
        BrushPreset { name: "Eraser".into(), hardness: 0.5, ..BrushPreset::default() }
    }

    /// The preset with every value inside its range (non-finite values become the default's).
    pub fn clamped(&self) -> BrushPreset {
        let d = BrushPreset::default();
        let fix = |v: f32, def: f32, lo: f32, hi: f32| if v.is_finite() { v.clamp(lo, hi) } else { def };
        let unit = |v: f32, def: f32| fix(v, def, 0.0, 1.0);
        BrushPreset {
            name: self.name.clone(),
            size: fix(self.size, d.size, 1e-4, 10_000.0),
            size_space: self.size_space,
            tip: self.tip.clone(),
            hardness: unit(self.hardness, d.hardness),
            flow: unit(self.flow, d.flow),
            opacity: unit(self.opacity, d.opacity),
            spacing: fix(self.spacing, d.spacing, 0.01, 10.0),
            angle: fix(self.angle, 0.0, -360.0, 360.0),
            follow_direction: self.follow_direction,
            roundness: fix(self.roundness, 1.0, 0.01, 1.0),
            jitter: Jitter {
                size: unit(self.jitter.size, 0.0),
                flow: unit(self.jitter.flow, 0.0),
                angle: unit(self.jitter.angle, 0.0),
                position: fix(self.jitter.position, 0.0, 0.0, 10.0),
            },
            pressure: self.pressure,
            placement: self.placement,
            max_angle: fix(self.max_angle, d.max_angle, 0.0, 180.0),
        }
    }
}

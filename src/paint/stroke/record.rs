//! The stroke record's data: [`StrokeRecord`] → [`Stroke`]s → [`StrokeSample`]s, plus the camera, view kind,
//! tool, preset reference, seed and symmetry each stroke was made with.

/// A 4×4 matrix, column-major (OpenGL and glTF order: elements 12, 13, 14 are the translation).
pub type Mat4 = [f32; 16];

/// The identity matrix.
pub const IDENTITY: Mat4 = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];

/// The brush strokes behind a paint effect's pixels, in the order they were painted. The pixels stay what is
/// shown; the record makes replay, re-projection and resolution changes possible.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StrokeRecord {
    pub strokes: Vec<Stroke>,
}

impl StrokeRecord {
    /// Samples over all strokes.
    pub fn sample_count(&self) -> usize {
        self.strokes.iter().map(|s| s.samples.len()).sum()
    }
}

/// One stroke: pen down, the samples, pen up.
#[derive(Debug, Clone, PartialEq)]
pub struct Stroke {
    pub tool: StrokeTool,
    /// Which brush preset painted it: a reference resolved by the program (an asset id, or for imports a
    /// `<source>:` reference such as `substance:DataBrush/<uid>`). Empty if unknown.
    pub preset: String,
    pub view: ViewKind,
    /// The camera at the time of the stroke (meaningful for [`ViewKind::Mesh`]).
    pub camera: Camera,
    /// Seed of the brush's random variation (jitter, scattering), so a replay places the same stamps.
    pub seed: u32,
    pub symmetry: Symmetry,
    pub samples: Vec<StrokeSample>,
}

/// One input sample of a stroke.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrokeSample {
    /// Where the pen was, in the view's 2D space: texture coordinates for [`ViewKind::Uv`], normalised device
    /// coordinates (−1 … 1, y up) of [`Stroke::camera`] for [`ViewKind::Mesh`].
    pub position: [f32; 2],
    /// Pen pressure 0 … 1 (1 for a mouse).
    pub pressure: f32,
    /// Pen tilt in degrees, x and y (−90 … 90; 0 = upright or unknown).
    pub tilt: [f32; 2],
    /// Seconds since the stroke started.
    pub time: f32,
}

impl StrokeSample {
    /// A sample at `position` with full pressure, no tilt.
    pub fn at(position: [f32; 2], time: f32) -> StrokeSample {
        StrokeSample { position, pressure: 1.0, tilt: [0.0; 2], time }
    }
}

/// The tool a stroke was made with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrokeTool {
    /// Paints the brush's material.
    Paint,
    /// Removes paint.
    Eraser,
    /// Drags what is painted along the stroke.
    Smudge,
    /// Paints through a projected image or material (a stencil/projection tool).
    Projection,
}

impl StrokeTool {
    pub const ALL: [StrokeTool; 4] = [StrokeTool::Paint, StrokeTool::Eraser, StrokeTool::Smudge, StrokeTool::Projection];

    pub fn id(self) -> &'static str {
        match self {
            StrokeTool::Paint => "paint",
            StrokeTool::Eraser => "eraser",
            StrokeTool::Smudge => "smudge",
            StrokeTool::Projection => "projection",
        }
    }
}

/// Where a stroke was painted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewKind {
    /// In the 2D texture view: sample positions are texture coordinates.
    Uv,
    /// In the 3D view: sample positions are on screen and are projected onto the mesh with the camera.
    Mesh,
}

impl ViewKind {
    pub fn id(self) -> &'static str {
        match self {
            ViewKind::Uv => "uv",
            ViewKind::Mesh => "mesh",
        }
    }
}

/// A camera as plain matrices: world → view and view → clip space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub view: Mat4,
    pub projection: Mat4,
}

impl Default for Camera {
    fn default() -> Camera {
        Camera { view: IDENTITY, projection: IDENTITY }
    }
}

/// Symmetry copies of a stroke: copy `i` (1 … `copies`) is the stroke transformed `i` times by `transform`
/// (a mirror for one copy, a rotation step for radial symmetry). No copies: no symmetry. Odd copies flip their
/// stamps along `[u, v]` where `flip` says so (radial lettering).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Symmetry {
    pub copies: u32,
    pub transform: Mat4,
    pub flip: [bool; 2],
}

impl Default for Symmetry {
    fn default() -> Symmetry {
        Symmetry { copies: 0, transform: IDENTITY, flip: [false; 2] }
    }
}

/// The stroke record attached to paint pixels, as read: a record, or stored bytes that could not be read (a
/// newer or damaged record). Unreadable bytes are kept and written back unchanged, so nothing is lost.
#[derive(Debug, Clone, PartialEq)]
pub enum StrokeData {
    Record(StrokeRecord),
    Unreadable(UnreadableStrokes),
}

impl StrokeData {
    /// The record, if it could be read.
    pub fn record(&self) -> Option<&StrokeRecord> {
        match self {
            StrokeData::Record(r) => Some(r),
            StrokeData::Unreadable(_) => None,
        }
    }
}

impl From<StrokeRecord> for StrokeData {
    fn from(r: StrokeRecord) -> StrokeData {
        StrokeData::Record(r)
    }
}

/// A stroke record that could not be read, with why. `bytes` is empty when there was nothing to keep (the
/// entry was missing or could not be extracted); such a record is not written again.
#[derive(Debug, Clone, PartialEq)]
pub struct UnreadableStrokes {
    pub bytes: Vec<u8>,
    pub reason: String,
}

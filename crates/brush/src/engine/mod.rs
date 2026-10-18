//! Input points → dabs. [`DabEngine`] is the only way the rest of MaterialCraft reaches a brush engine; today it is
//! PhotoCraft's, behind the [`photocraft`] adapter (ADR 0007: one adapter file, pinned revision). PhotoCraft's
//! procedural tips, which the starter library ships as alphas, come through [`photocraft_tips`], and its response
//! curve (the editor's pen pressure curve) through [`photocraft_curve`], and its smudge, as the stamp stage's local
//! kernel ([`SmudgeKernel`]), through [`photocraft_smudge`].

mod photocraft;
mod photocraft_curve;
mod photocraft_smudge;
mod photocraft_tips;

pub use photocraft_curve::ResponseCurve;
pub use photocraft_smudge::SmudgeKernel;
pub use photocraft_tips::{PROCEDURAL_TIP_GENERATOR, PROCEDURAL_TIP_PX, PROCEDURAL_TIPS, TipInfo, procedural_tip, procedural_tip_alpha};

use materialcraft::paint::brush::BrushPreset;
use materialcraft::stamp::DabSample;

/// One input sample in the stroke's frame (screen units for 3D-view strokes, texels for 2D-view strokes).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InputPoint {
    pub pos: [f32; 2],
    /// Pen pressure 0 … 1 (1 for a mouse).
    pub pressure: f32,
    /// Pen tilt in degrees, x and y.
    pub tilt: [f32; 2],
    /// Pen barrel rotation in degrees (0 … 360, clockwise).
    pub rotation: f32,
    /// Seconds since the stroke started.
    pub time: f64,
    /// Dab size here relative to the stroke's start (model-sized brushes, ADR 0006 Q-PT-12); 1 otherwise.
    pub scale: f32,
}

impl InputPoint {
    /// A mouse sample: full pressure, no tilt or rotation, scale 1.
    pub fn at(pos: [f32; 2], time: f64) -> InputPoint {
        InputPoint { pos, pressure: 1.0, tilt: [0.0; 2], rotation: 0.0, time, scale: 1.0 }
    }
}

/// A sampled tip: `width × height` coverage values 0 … 1, row by row, top row first. The dab scales it so its
/// longer side is the dab's diameter.
#[derive(Debug, Clone, PartialEq)]
pub struct TipAlpha {
    pub width: u32,
    pub height: u32,
    pub values: Vec<f32>,
}

impl TipAlpha {
    /// A tip from `values`; `None` if the count does not match or a side is 0.
    pub fn new(width: u32, height: u32, values: Vec<f32>) -> Option<TipAlpha> {
        let n = (width as usize).checked_mul(height as usize)?;
        (n > 0 && n == values.len()).then_some(TipAlpha { width, height, values })
    }
}

/// What an engine takes beyond a [`BrushPreset`] (imported brushes, roadmap step 32): a sampled tip in place of
/// the preset's round one, and the share of the size and flow left at zero pen pressure.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EngineExtras {
    pub tip: Option<TipAlpha>,
    /// Size at zero pressure as a fraction of the full size (with pressure driving size).
    pub size_minimum: f32,
    /// Flow at zero pressure as a fraction of the full flow (with pressure driving flow).
    pub flow_minimum: f32,
}

/// The lazy mouse's smoother (PhotoCraft's pulled string), for [`crate::stroke::LazyMouse`].
pub(crate) use photocraft::{MAX_STRING, PulledString};

/// Makes one stroke's dabs. Deterministic: the same preset, size, seed and points give the same dabs, however the
/// points are split across [`DabEngine::push`] calls.
pub struct DabEngine {
    inner: photocraft::Engine,
}

impl DabEngine {
    /// An engine for `preset` with dabs `diameter` frame units across (at scale 1), its random variation seeded
    /// with `seed`.
    pub fn new(preset: &BrushPreset, diameter: f32, seed: u64) -> DabEngine {
        DabEngine::with_extras(preset, &EngineExtras::default(), diameter, seed)
    }

    /// [`DabEngine::new`] with what the preset can't say ([`EngineExtras`]).
    pub fn with_extras(preset: &BrushPreset, extras: &EngineExtras, diameter: f32, seed: u64) -> DabEngine {
        DabEngine { inner: photocraft::Engine::new(preset, extras, diameter, seed) }
    }

    /// Feeds points; appends the dabs they complete to `out`.
    pub fn push(&mut self, points: &[InputPoint], out: &mut Vec<DabSample>) {
        self.inner.push(points, out);
    }

    /// Ends the stroke; appends its last dabs to `out`.
    pub fn finish(&mut self, out: &mut Vec<DabSample>) {
        self.inner.finish(out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft::paint::brush::Jitter;

    fn line(n: usize) -> Vec<InputPoint> {
        (0..n).map(|i| InputPoint::at([10.0 + i as f32 * 3.0, 50.0], i as f64 / 60.0)).collect()
    }

    fn dabs(preset: &BrushPreset, points: &[InputPoint], chunks: usize) -> Vec<DabSample> {
        let mut e = DabEngine::new(preset, 20.0, 7);
        let mut out = Vec::new();
        for c in points.chunks(chunks.max(1)) {
            e.push(c, &mut out);
        }
        e.finish(&mut out);
        out
    }

    #[test]
    fn dabs_follow_spacing_and_do_not_depend_on_chunking() {
        let preset = BrushPreset { spacing: 0.25, jitter: Jitter { size: 0.5, angle: 0.3, flow: 0.2, position: 0.4 }, ..BrushPreset::default() };
        let pts = line(40);
        let a = dabs(&preset, &pts, 1);
        assert_eq!(a, dabs(&preset, &pts, 7));
        // 117 units of path at 5 units per dab
        assert!((22..=26).contains(&a.len()), "{}", a.len());
        assert!(a.iter().all(|d| !d.footprint.values.is_empty() && d.opacity == 1.0));
    }

    #[test]
    fn a_procedural_tip_paints_like_its_alpha_image_and_unlike_the_round_tip() {
        let chalk = BrushPreset { tip: materialcraft::paint::brush::Tip::Procedural("chalk".into()), ..BrushPreset::default() };
        let pts = line(30);
        let procedural = dabs(&chalk, &pts, 4);
        assert_ne!(procedural, dabs(&BrushPreset::default(), &pts, 4));
        let extras = EngineExtras { tip: procedural_tip_alpha("chalk"), ..EngineExtras::default() };
        let mut e = DabEngine::with_extras(&BrushPreset::default(), &extras, 20.0, 7);
        let mut image = Vec::new();
        e.push(&pts, &mut image);
        e.finish(&mut image);
        assert_eq!(procedural, image);
        let unknown = BrushPreset { tip: materialcraft::paint::brush::Tip::Procedural("nope".into()), ..BrushPreset::default() };
        assert_eq!(dabs(&unknown, &pts, 4), dabs(&BrushPreset::default(), &pts, 4), "an unknown tip paints round");
    }

    #[test]
    fn follow_direction_drops_the_first_dab_and_a_lone_click() {
        let preset = BrushPreset { follow_direction: true, ..BrushPreset::default() };
        assert!(dabs(&preset, &line(1), 1).is_empty());
        let plain = dabs(&BrushPreset::default(), &line(10), 1);
        assert_eq!(dabs(&preset, &line(10), 1).len() + 1, plain.len());
    }

    #[test]
    fn point_scale_sizes_dabs_and_the_ceiling_is_the_stroke_opacity() {
        let preset = BrushPreset { opacity: 0.4, ..BrushPreset::default() };
        let mut pts = line(20);
        pts.iter_mut().enumerate().for_each(|(i, p)| p.scale = 1.0 + i as f32 / 19.0);
        let d = dabs(&preset, &pts, 1);
        let (first, last) = (d.first().unwrap(), d.last().unwrap());
        assert!((first.radius - 10.0).abs() < 0.5 && (last.radius - 20.0).abs() < 1.0, "{} {}", first.radius, last.radius);
        assert!(d.iter().all(|d| (d.opacity - 0.4).abs() < 1e-6));
    }
}

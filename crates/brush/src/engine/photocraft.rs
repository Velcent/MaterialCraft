//! The adapter to PhotoCraft's brush engine (`photocraft-paint`, pinned; ADR 0007): the only file that names
//! `photocraft_paint`. Our presets become its `BrushSettings`, its `DabGenerator` turns input points into dabs and
//! `BrushContext::rasterize` turns each dab into a coverage grid, which leaves here as the core's
//! [`DabSample`]. PhotoCraft's own stroke buffer is not used: the core's stamp stage builds coverage up in texture
//! space. Its `Smoother` in pulled-string mode is the lazy mouse ([`PulledString`]); the generator's own smoothing
//! stays off, so recorded points are the points painted.
//!
//! Model-sized brushes (ADR 0006 Q-PT-12): PhotoCraft spaces and sizes dabs by one brush size. Each input point
//! carries a scale (the model's screen size there relative to the stroke's start); the adapter scales each dab's
//! radius by the scale interpolated along the segment it was placed on, before rasterising. Spacing stays that of
//! the stroke's start size.

use super::InputPoint;
use materialcraft::paint::brush::{BrushPreset, Tip};
use materialcraft::stamp::{DabSample, Footprint};
use photocraft_paint::dynamics::{DabGenerator, MAX_STRING_PX, Smoother};
use photocraft_paint::{BrushContext, BrushSettings, Control, Dab, Smoothing, StrokePoint, TipShape};

/// Dabs from points for one stroke.
pub(super) struct Engine {
    generator: DabGenerator,
    context: BrushContext,
    /// The stroke opacity ceiling, multiplied into every dab's own ceiling.
    opacity: f32,
    /// Drop the first dab: it has no direction yet (PT-05).
    skip_first: bool,
    /// The last point pushed: its position and scale.
    last: Option<([f32; 2], f32)>,
    primary: Vec<Dab>,
    dual: Vec<Dab>,
    coverage: Vec<f32>,
}

impl Engine {
    /// An engine for `preset` with dabs of `diameter` frame units at scale 1, jitter seeded with `seed`.
    pub fn new(preset: &BrushPreset, diameter: f32, seed: u64) -> Engine {
        let settings = settings(preset, diameter, seed);
        Engine {
            generator: DabGenerator::new(&settings, 1.0),
            context: BrushContext::new(&settings),
            opacity: preset.opacity.clamp(0.0, 1.0),
            skip_first: preset.follow_direction,
            last: None,
            primary: Vec::new(),
            dual: Vec::new(),
            coverage: Vec::new(),
        }
    }

    /// Feeds points; appends the dabs they complete.
    pub fn push(&mut self, points: &[InputPoint], out: &mut Vec<DabSample>) {
        for p in points {
            self.primary.clear();
            let sp = StrokePoint {
                x: f64::from(p.pos[0]),
                y: f64::from(p.pos[1]),
                pressure: p.pressure,
                tilt_x: p.tilt[0],
                tilt_y: p.tilt[1],
                time: p.time * 1000.0,
                ..StrokePoint::default()
            };
            self.generator.push(&[sp], &mut self.primary, &mut self.dual);
            let from = self.last.unwrap_or((p.pos, p.scale));
            self.emit(from, (p.pos, p.scale), out);
            self.last = Some((p.pos, p.scale));
        }
        self.dual.clear();
    }

    /// Ends the stroke; appends the remaining dabs.
    pub fn finish(&mut self, out: &mut Vec<DabSample>) {
        self.primary.clear();
        self.generator.finish(&mut self.primary, &mut self.dual);
        if let Some(last) = self.last {
            self.emit(last, last, out);
        }
        self.dual.clear();
    }

    /// Converts the generated dabs, which lie on the segment `from` → `to`, scaling each by the scale there.
    fn emit(&mut self, from: ([f32; 2], f32), to: ([f32; 2], f32), out: &mut Vec<DabSample>) {
        let dabs = std::mem::take(&mut self.primary);
        for mut d in dabs.iter().copied() {
            if self.skip_first && d.index == 0 {
                continue;
            }
            let c = [d.center.x as f32, d.center.y as f32];
            let scale = along(from, to, c);
            if scale.is_finite() && scale > 0.0 {
                d.radius *= scale;
            }
            let rect = self.context.dab_rect(&d, false);
            let (w, h) = ((rect.x1 - rect.x0).max(0) as usize, (rect.y1 - rect.y0).max(0) as usize);
            if w == 0 || h == 0 {
                continue;
            }
            self.context.rasterize(&d, false, rect, &mut self.coverage);
            let Some(footprint) = Footprint::new(rect.x0, rect.y0, w, h, std::mem::take(&mut self.coverage)) else { continue };
            out.push(DabSample { center: c, radius: d.radius, opacity: (d.opacity * self.opacity).clamp(0.0, 1.0), index: d.index, footprint });
        }
        self.primary = dabs;
    }
}

/// The longest lazy-mouse string, in frame units (PhotoCraft's pulled string at 100 %).
pub(crate) const MAX_STRING: f32 = MAX_STRING_PX as f32;

/// PhotoCraft's `Smoother` in pulled-string mode: the lazy mouse (PT-34). The brush follows the pointer only once
/// the pointer pulls a string of `radius` frame units taut; points keep the input's pressure, tilt, time and scale.
pub(crate) struct PulledString {
    inner: Smoother,
}

impl PulledString {
    pub fn new(radius: f32) -> PulledString {
        let amount = if radius.is_finite() { (radius / MAX_STRING).clamp(0.0, 1.0) } else { 0.0 };
        let cfg = Smoothing { enabled: true, amount, pulled_string: true, catch_up: false, catch_up_on_end: false, adjust_for_zoom: false };
        PulledString { inner: Smoother::new(&cfg, 1.0) }
    }

    /// Feeds one pointer position; appends the brush positions it pulls the brush to.
    pub fn push(&mut self, p: InputPoint, out: &mut Vec<InputPoint>) {
        let sp = StrokePoint { x: f64::from(p.pos[0]), y: f64::from(p.pos[1]), pressure: p.pressure, time: p.time * 1000.0, ..StrokePoint::default() };
        let mut moved = Vec::new();
        self.inner.push(sp, &mut moved);
        out.extend(moved.iter().map(|m| InputPoint { pos: [m.x as f32, m.y as f32], ..p }));
    }

    /// The string from the brush to the pointer while the brush lags behind.
    pub fn string(&self) -> Option<([f32; 2], [f32; 2])> {
        self.inner.string().map(|(a, b)| ([a[0] as f32, a[1] as f32], [b[0] as f32, b[1] as f32]))
    }
}

/// The scale at `p` on the segment `from` → `to` (linear in the projection of `p` onto it).
fn along(from: ([f32; 2], f32), to: ([f32; 2], f32), p: [f32; 2]) -> f32 {
    let (a, b) = (from.0, to.0);
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 1e-12 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) } else { 1.0 };
    from.1 + (to.1 - from.1) * t
}

/// Our preset as PhotoCraft's settings, with dabs `diameter` frame units across.
fn settings(preset: &BrushPreset, diameter: f32, seed: u64) -> BrushSettings {
    let p = preset.clamped();
    let mut s = BrushSettings {
        size: if diameter.is_finite() { diameter.clamp(0.5, photocraft_paint::MAX_BRUSH_SIZE) } else { 1.0 },
        hardness: p.hardness,
        spacing: p.spacing,
        spacing_enabled: true,
        // the stroke ceiling is applied by the stamp stage over the whole stroke; dabs build up to 1 here
        opacity: 1.0,
        flow: p.flow,
        pressure_size: p.pressure.size,
        pressure_opacity: false,
        angle: p.angle,
        roundness: p.roundness,
        seed,
        ..BrushSettings::default()
    };
    s.tip = match p.tip {
        Tip::Round => TipShape::Round,
    };
    s.smoothing.enabled = false;
    let sd = &mut s.shape_dynamics;
    sd.enabled = p.jitter.size > 0.0 || p.jitter.angle > 0.0 || p.follow_direction;
    sd.size.jitter = p.jitter.size;
    sd.angle.jitter = p.jitter.angle;
    if p.follow_direction {
        sd.angle.control = Control::Direction;
    }
    let tr = &mut s.transfer;
    tr.enabled = p.jitter.flow > 0.0 || p.pressure.flow;
    tr.flow.jitter = p.jitter.flow;
    if p.pressure.flow {
        tr.flow.control = Control::PenPressure;
    }
    let sc = &mut s.scattering;
    sc.enabled = p.jitter.position > 0.0;
    sc.both_axes = true;
    // PhotoCraft scatters by radii, the preset by diameters
    sc.scatter.jitter = p.jitter.position * 2.0;
    s
}

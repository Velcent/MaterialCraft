//! A brush's picture: one S-shaped stroke with a pen pressure that swells and fades, painted by the dab engine
//! into a grey coverage grid. The tray's brush thumbnails are made with it (saved presets, the starter library).

use crate::engine::{DabEngine, EngineExtras, InputPoint, TipAlpha};
use materialcraft::paint::brush::BrushPreset;

/// Input samples along the preview's path.
const SAMPLES: usize = 160;

/// The coverage (0 … 1, row by row, top row first) of a sample stroke of `preset` across a `width` × `height`
/// picture; `tip` is an alpha-image tip ([`crate::tips::alpha_for`]). The brush's size is set to fit the picture,
/// whatever it is on the model. Empty for a zero side.
pub fn stroke_preview(preset: &BrushPreset, tip: Option<TipAlpha>, width: u32, height: u32) -> Vec<f32> {
    let (w, h) = (width as usize, height as usize);
    let mut out = vec![0.0f32; w * h];
    if w == 0 || h == 0 {
        return out;
    }
    let (wf, hf) = (width as f32, height as f32);
    let diameter = (hf * 0.3).min(wf * 0.2).max(1.0);
    let extras = EngineExtras { tip, size_minimum: 0.25, flow_minimum: 0.25 };
    let mut engine = DabEngine::with_extras(preset, &extras, diameter, 1);
    let margin = diameter * 0.75;
    let points: Vec<InputPoint> = (0..SAMPLES)
        .map(|i| {
            let t = i as f32 / (SAMPLES - 1) as f32;
            let x = margin + t * (wf - 2.0 * margin);
            let y = hf * 0.5 - (t * std::f32::consts::TAU).sin() * (hf * 0.5 - margin) * 0.8;
            let pressure = 0.35 + 0.65 * (t * std::f32::consts::PI).sin();
            InputPoint { pressure, ..InputPoint::at([x, y], f64::from(t)) }
        })
        .collect();
    let mut dabs = Vec::new();
    engine.push(&points, &mut dabs);
    engine.finish(&mut dabs);
    for dab in &dabs {
        let [x0, y0, x1, y1] = dab.footprint.bounds();
        let (xa, ya) = (x0.floor().max(0.0) as usize, y0.floor().max(0.0) as usize);
        let (xb, yb) = ((x1.ceil().max(0.0) as usize).min(w), (y1.ceil().max(0.0) as usize).min(h));
        for y in ya..yb {
            for x in xa..xb {
                let c = dab.footprint.sample([x as f32 + 0.5, y as f32 + 0.5]);
                if let Some(v) = out.get_mut(y * w + x) {
                    // coverage builds up dab over dab to the stroke's ceiling, as the stamp stage does
                    *v = (*v + c * (1.0 - *v)).min(dab.opacity.max(*v));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft::paint::brush::Tip;

    #[test]
    fn previews_show_the_stroke_and_differ_by_tip() {
        let round = stroke_preview(&BrushPreset::default(), None, 96, 48);
        assert_eq!(round.len(), 96 * 48);
        let painted = round.iter().filter(|v| **v > 0.5).count();
        assert!(painted > 96 * 48 / 20, "{painted}");
        assert!(round.iter().all(|v| (0.0..=1.0).contains(v)));
        assert_eq!(round, stroke_preview(&BrushPreset::default(), None, 96, 48), "repeatable");
        let chalk = stroke_preview(&BrushPreset { tip: Tip::Procedural("chalk".into()), ..BrushPreset::default() }, None, 96, 48);
        assert_ne!(chalk, round);
        let faint = stroke_preview(&BrushPreset { opacity: 0.3, ..BrushPreset::default() }, None, 96, 48);
        assert!(faint.iter().all(|v| *v <= 0.3 + 1e-6));
        assert!(stroke_preview(&BrushPreset::default(), None, 0, 10).is_empty());
    }
}

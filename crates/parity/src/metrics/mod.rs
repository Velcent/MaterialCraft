//! [`Metrics`]: ours against Painter's, inside the UV islands, overall and per component: mean absolute error,
//! max, PSNR, share within 2/255 (the substance crate's [`Diff`]), SSIM, and a least-squares fit
//! `theirs ≈ gain × ours + offset` that shows range and inversion differences; and the texture statistics
//! ([`Distribution`]: histogram and power-spectrum distance) that judge look-alikes.

mod distribution;
mod ssim;

pub use distribution::Distribution;
pub use ssim::ssim;

use crate::subject::Component;
use materialcraft::tex::Tex;
use materialcraft_substance::tools::compare::{Diff, Fit};

#[derive(Debug, Clone, PartialEq)]
pub struct ComponentMetrics {
    pub label: &'static str,
    pub diff: Diff,
    pub ssim: Option<f64>,
    pub fit: Fit,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Metrics {
    /// Over all compared components.
    pub overall: Diff,
    /// Mean of the components' SSIM.
    pub ssim: Option<f64>,
    pub components: Vec<ComponentMetrics>,
    /// Histogram and spectrum distance of the first component.
    pub distribution: Distribution,
}

impl Metrics {
    /// `ours` and `theirs` must have the mask's size. Also returns the largest component error per texel
    /// (0 outside the mask), for the heatmap.
    pub fn measure(ours: &Tex, theirs: &Tex, components: &[Component], mask: &[bool]) -> (Metrics, Vec<f32>) {
        let indices: Vec<usize> = components.iter().map(|c| c.0).collect();
        let mut errors = Vec::new();
        let overall = Diff::measure(&ours.px, &theirs.px, &indices, mask, &mut errors);
        let mut scratch = Vec::new();
        let components: Vec<ComponentMetrics> = components
            .iter()
            .map(|&(c, label)| ComponentMetrics {
                label,
                diff: Diff::measure(&ours.px, &theirs.px, &[c], mask, &mut scratch),
                ssim: ssim(ours, theirs, c, mask),
                fit: Fit::of(&component(ours, c), &component(theirs, c), mask),
            })
            .collect();
        let ssims: Vec<f64> = components.iter().filter_map(|c| c.ssim).collect();
        let first = indices.first().copied().unwrap_or(0);
        let distribution = Distribution::measure(ours, theirs, first, mask);
        let metrics = Metrics {
            distribution,
            overall,
            ssim: (!ssims.is_empty()).then(|| ssims.iter().sum::<f64>() / ssims.len() as f64),
            components,
        };
        (metrics, errors)
    }

    /// Mean absolute error in 8-bit steps (1 = 1/255), the unit the docs use.
    pub fn mae_steps(&self) -> f64 {
        self.overall.mae * 255.0
    }
}

/// Component `c` moved into the first slot ([`Fit`] reads the first component).
fn component(tex: &Tex, c: usize) -> Vec<[f32; 4]> {
    tex.px.iter().map(|p| [p[c], 0.0, 0.0, 0.0]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subject::XYZ;

    #[test]
    fn measures_per_component_inside_the_mask() {
        let ours = Tex::solid(4, 4, [0.5, 0.5, 0.5, 1.0]);
        let mut theirs = ours.clone();
        theirs.px.iter_mut().for_each(|p| p[1] = 0.6);
        theirs.px[0] = [1.0; 4];
        let mut mask = vec![true; 16];
        mask[0] = false;
        let (m, errors) = Metrics::measure(&ours, &theirs, XYZ, &mask);
        assert_eq!(m.overall.texels, 15);
        assert!(m.components[0].diff.mae < 1e-9 && (m.components[1].diff.mae - 0.1).abs() < 1e-6);
        assert!((m.mae_steps() - 0.1 / 3.0 * 255.0).abs() < 1e-3);
        assert_eq!(errors[0], 0.0);
        assert_eq!(errors.len(), 16);
    }
}

//! [`Diff`]: error statistics between our map and an exported one, over the texels of a mask.

/// Values within this distance count as matching (2 steps of 8 bits).
pub const TOLERANCE: f32 = 2.0 / 255.0;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Diff {
    /// Mean absolute error per sample (0..1 scale).
    pub mae: f64,
    pub max: f64,
    /// Peak signal-to-noise ratio in dB for a peak of 1 (infinite when identical).
    pub psnr: f64,
    /// Share of texels whose largest component error is within [`TOLERANCE`], in percent.
    pub within: f64,
    pub texels: usize,
}

impl Diff {
    /// Compares components `comps` of `ours` and `theirs` where `mask` is true. `errors` receives the largest
    /// component error per texel (0 outside the mask).
    pub fn measure(
        ours: &[[f32; 4]],
        theirs: &[[f32; 4]],
        comps: &[usize],
        mask: &[bool],
        errors: &mut Vec<f32>,
    ) -> Diff {
        errors.clear();
        errors.resize(ours.len(), 0.0);
        let (mut sum, mut sq, mut max, mut within, mut texels) = (0f64, 0f64, 0f64, 0usize, 0usize);
        for (i, (a, b)) in ours.iter().zip(theirs).enumerate() {
            if !mask.get(i).copied().unwrap_or(false) {
                continue;
            }
            texels += 1;
            let mut worst = 0f32;
            for &c in comps {
                let e = (a[c] - b[c]).abs();
                let e = if e.is_nan() { 1.0 } else { e };
                sum += e as f64;
                sq += (e as f64) * (e as f64);
                worst = worst.max(e);
            }
            max = max.max(worst as f64);
            within += usize::from(worst <= TOLERANCE + 1e-6);
            errors[i] = worst;
        }
        let samples = (texels * comps.len()).max(1) as f64;
        let mse = sq / samples;
        Diff {
            mae: sum / samples,
            max,
            psnr: if mse > 0.0 { -10.0 * mse.log10() } else { f64::INFINITY },
            within: 100.0 * within as f64 / texels.max(1) as f64,
            texels,
        }
    }
}

/// Least-squares line `theirs ≈ gain × ours + offset` over the first component of masked texels; shows
/// range and scale differences (e.g. a height export that is offset or normalised).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Fit {
    pub gain: f64,
    pub offset: f64,
}

impl Fit {
    pub fn of(ours: &[[f32; 4]], theirs: &[[f32; 4]], mask: &[bool]) -> Fit {
        let (mut n, mut sx, mut sy, mut sxx, mut sxy) = (0f64, 0f64, 0f64, 0f64, 0f64);
        for (i, (a, b)) in ours.iter().zip(theirs).enumerate() {
            if mask.get(i).copied().unwrap_or(false) && a[0].is_finite() && b[0].is_finite() {
                let (x, y) = (a[0] as f64, b[0] as f64);
                n += 1.0;
                sx += x;
                sy += y;
                sxx += x * x;
                sxy += x * y;
            }
        }
        let var = n * sxx - sx * sx;
        if n == 0.0 || var.abs() < 1e-12 {
            return Fit {
                gain: 0.0,
                offset: if n > 0.0 { sy / n } else { 0.0 },
            };
        }
        let gain = (n * sxy - sx * sy) / var;
        Fit {
            gain,
            offset: (sy - gain * sx) / n,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_inside_the_mask_only() {
        let ours = [[0.5; 4], [0.0; 4], [1.0; 4]];
        let theirs = [[0.5; 4], [0.1; 4], [0.0; 4]];
        let mut errors = Vec::new();
        let d = Diff::measure(&ours, &theirs, &[0], &[true, true, false], &mut errors);
        assert_eq!(d.texels, 2);
        assert!((d.mae - 0.05).abs() < 1e-6 && (d.max - 0.1).abs() < 1e-6);
        assert_eq!(d.within, 50.0);
        assert_eq!(errors[2], 0.0);
        let same = Diff::measure(&ours, &ours, &[0, 1, 2], &[true; 3], &mut errors);
        assert!(same.psnr.is_infinite());
        let fit = Fit::of(&ours, &[[0.75; 4], [0.5; 4], [1.0; 4]], &[true; 3]);
        assert!((fit.gain - 0.5).abs() < 1e-6 && (fit.offset - 0.5).abs() < 1e-6);
    }
}

//! [`Distribution`]: how alike two images are as *textures*, not pixel for pixel: the distance between their
//! value histograms and between their radially averaged power spectra, inside a mask. Noise and grunge
//! look-alikes are never pixel-equal (their MAE says nothing), but a good one has a similar histogram (the same
//! amount of dark and light) and a similar spectrum (features of the same sizes).

use materialcraft::tex::Tex;

/// Histogram bins over 0..1.
const BINS: usize = 64;
/// Side of the square the spectrum is computed on (a power of two).
const SPECTRUM_SIZE: usize = 128;
/// Radial frequency bands (octaves split in two), from 1 cycle per image to the Nyquist frequency.
const BANDS: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Distribution {
    /// Earth mover's distance between the value histograms, in 8-bit steps: how far values have to move on
    /// average to turn one histogram into the other.
    pub histogram: f64,
    /// Mean absolute difference of the log10 band powers of the normalised power spectra (0 = the same
    /// distribution of feature sizes; 1 = a factor of ten per band); `None` when the mask holds too few texels.
    pub spectrum: Option<f64>,
}

impl Distribution {
    /// Component `c` of `ours` and `theirs` inside `mask` (all three the same size).
    pub fn measure(ours: &Tex, theirs: &Tex, c: usize, mask: &[bool]) -> Distribution {
        Distribution {
            histogram: histogram_distance(ours, theirs, c, mask) * 255.0,
            spectrum: spectrum_distance(ours, theirs, c, mask),
        }
    }
}

fn values(t: &Tex, c: usize, mask: &[bool]) -> Vec<f32> {
    t.px.iter()
        .zip(mask)
        .filter(|(_, m)| **m)
        .map(|(p, _)| p.get(c).copied().unwrap_or(0.0).clamp(0.0, 1.0))
        .collect()
}

fn histogram(v: &[f32]) -> [f64; BINS] {
    let mut h = [0.0f64; BINS];
    for x in v {
        let b = ((x * BINS as f32) as usize).min(BINS - 1);
        if let Some(slot) = h.get_mut(b) {
            *slot += 1.0;
        }
    }
    let n = v.len().max(1) as f64;
    h.iter_mut().for_each(|x| *x /= n);
    h
}

/// Earth mover's distance between the two histograms, in value units (0..1).
fn histogram_distance(ours: &Tex, theirs: &Tex, c: usize, mask: &[bool]) -> f64 {
    let (a, b) = (histogram(&values(ours, c, mask)), histogram(&values(theirs, c, mask)));
    let (mut ca, mut cb, mut sum) = (0.0f64, 0.0f64, 0.0f64);
    for (x, y) in a.iter().zip(&b) {
        ca += x;
        cb += y;
        sum += (ca - cb).abs();
    }
    sum / BINS as f64
}

/// Component `c` resampled (nearest) to `n × n`, mean-free inside the mask and 0 outside.
fn centred(t: &Tex, c: usize, mask: &[bool], n: usize) -> Option<Vec<[f64; 2]>> {
    let (w, h) = (t.w.max(1), t.h.max(1));
    let at = |x: usize, y: usize| {
        let (sx, sy) = (x * w / n, y * h / n);
        let i = sy * w + sx;
        match mask.get(i).copied().unwrap_or(false) {
            true => t.px.get(i).and_then(|p| p.get(c)).map(|v| f64::from(*v)),
            false => None,
        }
    };
    let samples: Vec<Option<f64>> = (0..n * n).map(|i| at(i % n, i / n)).collect();
    let inside: Vec<f64> = samples.iter().flatten().copied().collect();
    if inside.len() < n * n / 16 {
        return None;
    }
    let mean = inside.iter().sum::<f64>() / inside.len() as f64;
    Some(samples.iter().map(|s| [s.map_or(0.0, |v| v - mean), 0.0]).collect())
}

/// In-place radix-2 FFT of a power-of-two length.
fn fft(a: &mut [[f64; 2]]) {
    let n = a.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            a.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * std::f64::consts::PI / len as f64;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (s, co) = (ang * k as f64).sin_cos();
                let (Some(&u), Some(&v)) = (a.get(start + k), a.get(start + k + len / 2)) else {
                    continue;
                };
                let t = [v[0] * co - v[1] * s, v[0] * s + v[1] * co];
                if let Some(x) = a.get_mut(start + k) {
                    *x = [u[0] + t[0], u[1] + t[1]];
                }
                if let Some(x) = a.get_mut(start + k + len / 2) {
                    *x = [u[0] - t[0], u[1] - t[1]];
                }
            }
        }
        len <<= 1;
    }
}

/// Normalised power per radial band of an `n × n` mean-free image.
fn band_powers(mut img: Vec<[f64; 2]>, n: usize) -> [f64; BANDS] {
    for row in img.chunks_mut(n) {
        fft(row);
    }
    let mut col = vec![[0.0f64; 2]; n];
    for x in 0..n {
        for (y, c) in col.iter_mut().enumerate() {
            *c = img.get(y * n + x).copied().unwrap_or_default();
        }
        fft(&mut col);
        for (y, c) in col.iter().enumerate() {
            if let Some(p) = img.get_mut(y * n + x) {
                *p = *c;
            }
        }
    }
    let mut bands = [0.0f64; BANDS];
    let nyquist = (n / 2) as f64;
    for (i, v) in img.iter().enumerate() {
        let (x, y) = (i % n, i / n);
        let fx = if x > n / 2 { x as f64 - n as f64 } else { x as f64 };
        let fy = if y > n / 2 { y as f64 - n as f64 } else { y as f64 };
        let r = (fx * fx + fy * fy).sqrt();
        if !(1.0..=nyquist).contains(&r) {
            continue;
        }
        // bands evenly spaced in log frequency between 1 and the Nyquist frequency
        let b = ((r.ln() / nyquist.ln()) * BANDS as f64) as usize;
        if let Some(slot) = bands.get_mut(b.min(BANDS - 1)) {
            *slot += v[0] * v[0] + v[1] * v[1];
        }
    }
    let total: f64 = bands.iter().sum::<f64>().max(1e-30);
    bands.iter_mut().for_each(|b| *b /= total);
    bands
}

fn spectrum_distance(ours: &Tex, theirs: &Tex, c: usize, mask: &[bool]) -> Option<f64> {
    let n = SPECTRUM_SIZE;
    let a = band_powers(centred(ours, c, mask, n)?, n);
    let b = band_powers(centred(theirs, c, mask, n)?, n);
    let floor = 1e-6;
    Some(
        a.iter()
            .zip(&b)
            .map(|(x, y)| (x.max(floor).log10() - y.max(floor).log10()).abs())
            .sum::<f64>()
            / BANDS as f64,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tex(n: usize, f: impl Fn(usize, usize) -> f32) -> Tex {
        Tex {
            w: n,
            h: n,
            px: (0..n * n).map(|i| f(i % n, i / n)).map(|v| [v, v, v, 1.0]).collect(),
        }
    }

    #[test]
    fn same_statistics_measure_close_and_different_ones_far() {
        let n = 128;
        let mask = vec![true; n * n];
        let stripes = |k: f32| {
            tex(n, move |x, _| {
                0.5 + 0.5 * (x as f32 * k * std::f32::consts::TAU / n as f32).sin()
            })
        };
        let shifted = tex(n, |x, _| {
            0.5 + 0.5 * ((x as f32 + 5.0) * 8.0 * std::f32::consts::TAU / n as f32).sin()
        });
        let same = Distribution::measure(&stripes(8.0), &shifted, 0, &mask);
        assert!(same.histogram < 1.0, "{same:?}");
        assert!(same.spectrum.is_some_and(|s| s < 0.05), "{same:?}");
        let finer = Distribution::measure(&stripes(8.0), &stripes(40.0), 0, &mask);
        assert!(finer.spectrum.is_some_and(|s| s > 0.5), "{finer:?}");
        let darker = Distribution::measure(&stripes(8.0), &tex(n, |_, _| 0.1), 0, &mask);
        assert!(darker.histogram > 50.0, "{darker:?}");
        assert!(
            Distribution::measure(&shifted, &shifted, 0, &vec![false; n * n])
                .spectrum
                .is_none()
        );
    }
}

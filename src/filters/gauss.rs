//! Gaussian blur of a padded [`Plane`]: separable, edges clamped (the plane is padded, so nothing wraps around
//! as in the designer's tileable `ops::blur`), single-threaded (the compositor already runs tiles in
//! parallel). Small sigmas use the exact kernel, larger ones three box passes.

use super::plane::Plane;

/// How far (texels) a blur of `sigma` reads.
pub fn reach(sigma: f32) -> u32 {
    if sigma.is_finite() && sigma > 0.0 { (sigma * 3.0).ceil().min(super::kernel::MAX_RADIUS as f32) as u32 + 1 } else { 0 }
}

/// The plane blurred with a gaussian of `sigma` texels. Colour is weighted by coverage (alpha), so texels
/// without coverage add nothing to their neighbours' colour.
pub fn blur(p: &Plane, sigma: f32) -> Vec<[f32; 4]> {
    if !(sigma.is_finite() && sigma > 0.05) || p.w == 0 || p.h == 0 {
        return p.px.clone();
    }
    let mut px: Vec<[f32; 4]> = p.px.iter().map(|v| [v[0] * v[3], v[1] * v[3], v[2] * v[3], v[3]]).collect();
    if sigma < 3.0 {
        let k = kernel(sigma);
        px = pass(&px, p.w, p.h, true, |line, out| convolve(line, &k, out));
        px = pass(&px, p.w, p.h, false, |line, out| convolve(line, &k, out));
    } else {
        for r in box_radii(sigma) {
            px = pass(&px, p.w, p.h, true, |line, out| box_line(line, r, out));
            px = pass(&px, p.w, p.h, false, |line, out| box_line(line, r, out));
        }
    }
    px.into_iter().map(|v| if v[3] > 1e-6 { [v[0] / v[3], v[1] / v[3], v[2] / v[3], v[3]] } else { [0.0, 0.0, 0.0, 0.0] }).collect()
}

/// Normalised gaussian weights for offsets -n..=n.
fn kernel(sigma: f32) -> Vec<f32> {
    let n = (sigma * 3.0).ceil().max(1.0) as i64;
    let k: Vec<f32> = (-n..=n).map(|i| (-((i * i) as f32) / (2.0 * sigma * sigma)).exp()).collect();
    let sum: f32 = k.iter().sum();
    k.into_iter().map(|v| v / sum).collect()
}

/// Radii of three box blurs that together approximate a gaussian of `sigma`.
fn box_radii(sigma: f32) -> [usize; 3] {
    let ideal = (12.0 * sigma * sigma / 3.0 + 1.0).sqrt();
    let mut lower = ideal.floor() as i64;
    if lower % 2 == 0 {
        lower -= 1;
    }
    let upper = lower + 2;
    let m = ((12.0 * sigma * sigma - 3.0 * (lower * lower) as f32 - 12.0 * lower as f32 - 9.0) / (-4.0 * lower as f32 - 4.0)).round() as i64;
    let width = |i: i64| if i < m { lower } else { upper };
    [width(0), width(1), width(2)].map(|w| (w.max(1) / 2) as usize)
}

/// Runs `f` over every row (`rows`) or every column, line by line.
fn pass(px: &[[f32; 4]], w: usize, h: usize, rows: bool, f: impl Fn(&[[f32; 4]], &mut [[f32; 4]])) -> Vec<[f32; 4]> {
    let mut out = vec![[0.0; 4]; w * h];
    if rows {
        for (src, dst) in px.chunks_exact(w).zip(out.chunks_exact_mut(w)) {
            f(src, dst);
        }
        return out;
    }
    let mut line = vec![[0.0; 4]; h];
    let mut done = vec![[0.0; 4]; h];
    for x in 0..w {
        for (y, l) in line.iter_mut().enumerate() {
            *l = px.get(y * w + x).copied().unwrap_or([0.0; 4]);
        }
        f(&line, &mut done);
        for (y, d) in done.iter().enumerate() {
            if let Some(o) = out.get_mut(y * w + x) {
                *o = *d;
            }
        }
    }
    out
}

fn clamped(line: &[[f32; 4]], i: i64) -> [f32; 4] {
    let n = line.len() as i64;
    line.get(i.clamp(0, (n - 1).max(0)) as usize).copied().unwrap_or([0.0; 4])
}

fn convolve(line: &[[f32; 4]], k: &[f32], out: &mut [[f32; 4]]) {
    let n = (k.len() / 2) as i64;
    for (i, o) in out.iter_mut().enumerate() {
        let mut a = [0.0; 4];
        for (j, w) in k.iter().enumerate() {
            let v = clamped(line, i as i64 + j as i64 - n);
            for c in 0..4 {
                if let (Some(a), Some(v)) = (a.get_mut(c), v.get(c)) {
                    *a += v * w;
                }
            }
        }
        *o = a;
    }
}

/// A running-sum box blur of radius `r` along one line, edges clamped.
fn box_line(line: &[[f32; 4]], r: usize, out: &mut [[f32; 4]]) {
    let r = r as i64;
    let inv = 1.0 / (2 * r + 1) as f32;
    let mut acc = [0.0f32; 4];
    for k in -r..=r {
        let v = clamped(line, k);
        acc = [acc[0] + v[0], acc[1] + v[1], acc[2] + v[2], acc[3] + v[3]];
    }
    for (i, o) in out.iter_mut().enumerate() {
        *o = acc.map(|a| a * inv);
        let (add, sub) = (clamped(line, i as i64 + r + 1), clamped(line, i as i64 - r));
        acc = [acc[0] + add[0] - sub[0], acc[1] + add[1] - sub[1], acc[2] + add[2] - sub[2], acc[3] + add[3] - sub[3]];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spike(w: usize) -> Plane {
        let mut p = Plane::filled(0, 0, w, 1, [0.0, 0.0, 0.0, 1.0]);
        p.px[w / 2] = [1.0, 1.0, 1.0, 1.0];
        p
    }

    #[test]
    fn keeps_the_total_and_spreads_by_sigma() {
        for sigma in [1.0f32, 6.0] {
            let p = spike(101);
            let b = blur(&p, sigma);
            let sum: f32 = b.iter().map(|v| v[0]).sum();
            assert!((sum - 1.0).abs() < 1e-3, "{sigma}: {sum}");
            let var: f32 = b.iter().enumerate().map(|(i, v)| v[0] * (i as f32 - 50.0).powi(2)).sum();
            assert!((var.sqrt() - sigma).abs() < 0.15 * sigma, "{sigma}: {}", var.sqrt());
            assert!(b.iter().enumerate().all(|(i, v)| (i as i64 - 50).unsigned_abs() as u32 <= reach(sigma) || v[0].abs() < 1e-6));
        }
    }

    #[test]
    fn uncovered_texels_lend_no_colour() {
        let p = Plane { x0: 0, y0: 0, w: 2, h: 1, px: vec![[1.0, 0.0, 0.0, 1.0], [0.0, 1.0, 0.0, 0.0]] };
        let b = blur(&p, 1.0);
        assert!((b[1][0] - 1.0).abs() < 1e-5 && b[1][1] == 0.0 && b[1][3] > 0.0);
    }
}

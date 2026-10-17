//! From per-sample values to the output texture: averaging sub-samples into texels and dilating past the islands'
//! edges so filtering and mip levels don't pull in the background.

/// Averages `sub` x `sub` blocks of a `width`² sample grid into `(width / sub)`² texels, over the covered samples
/// only. Returns the texels and which of them are covered.
pub(crate) fn downsample(values: &[[f32; 4]], covered: &[bool], width: usize, sub: usize) -> (Vec<[f32; 4]>, Vec<bool>) {
    let sub = sub.max(1);
    let size = width / sub;
    let mut px = vec![[0.0f32; 4]; size * size];
    let mut cover = vec![false; size * size];
    for (i, (out, c)) in px.iter_mut().zip(cover.iter_mut()).enumerate() {
        let (x, y) = (i % size.max(1), i / size.max(1));
        let (mut sum, mut n, mut background) = ([0.0f32; 4], 0u32, None);
        for sy in 0..sub {
            for sx in 0..sub {
                let j = (y * sub + sy) * width + x * sub + sx;
                let (Some(v), Some(&inside)) = (values.get(j), covered.get(j)) else {
                    continue;
                };
                if inside {
                    sum.iter_mut().zip(v).for_each(|(s, v)| *s += v);
                    n += 1;
                } else {
                    background.get_or_insert(*v);
                }
            }
        }
        if n > 0 {
            *out = sum.map(|s| s / n as f32);
            *c = true;
        } else if let Some(b) = background {
            *out = b;
        }
    }
    (px, cover)
}

/// Grows the covered area by `rings` texels: each new texel takes the average of its covered 8-neighbours.
pub(crate) fn dilate(px: &mut [[f32; 4]], covered: &[bool], size: usize, rings: usize) {
    let mut filled = covered.to_vec();
    filled.resize(px.len(), false);
    let is_filled = |filled: &[bool], i: usize| filled.get(i).copied().unwrap_or(false);
    let mut frontier: Vec<usize> = (0..filled.len()).filter(|&i| is_filled(&filled, i)).collect();
    for _ in 0..rings {
        let mut next: Vec<usize> = frontier.iter().flat_map(|&i| neighbours(i, size)).filter(|&j| !is_filled(&filled, j)).collect();
        next.sort_unstable();
        next.dedup();
        if next.is_empty() {
            break;
        }
        let values: Vec<[f32; 4]> = next
            .iter()
            .map(|&j| {
                let (mut sum, mut n) = ([0.0f32; 4], 0.0f32);
                for v in neighbours(j, size).filter(|&k| is_filled(&filled, k)).filter_map(|k| px.get(k)) {
                    sum.iter_mut().zip(v).for_each(|(s, v)| *s += v);
                    n += 1.0;
                }
                sum.map(|s| s / n.max(1.0))
            })
            .collect();
        for (&j, v) in next.iter().zip(values) {
            if let (Some(p), Some(f)) = (px.get_mut(j), filled.get_mut(j)) {
                *p = v;
                *f = true;
            }
        }
        frontier = next;
    }
}

fn neighbours(i: usize, size: usize) -> impl Iterator<Item = usize> {
    let (x, y) = ((i % size) as i64, (i / size) as i64);
    let s = size as i64;
    (-1i64..=1).flat_map(move |dy| (-1i64..=1).map(move |dx| (x + dx, y + dy))).filter_map(move |(nx, ny)| {
        let ok = (nx, ny) != (x, y) && nx >= 0 && ny >= 0 && nx < s && ny < s;
        ok.then(|| (ny * s + nx) as usize)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn averages_covered_samples_only() {
        let values = [[1.0; 4], [0.0; 4], [3.0; 4], [9.0; 4]];
        let (px, cover) = downsample(&values, &[true, false, true, false], 2, 2);
        assert_eq!((px, cover), (vec![[2.0; 4]], vec![true]));
        let (px, cover) = downsample(&values, &[false; 4], 2, 2);
        assert_eq!((px, cover), (vec![[1.0; 4]], vec![false]));
    }

    #[test]
    fn dilates_rings() {
        let mut px = vec![[0.0f32; 4]; 25];
        let mut covered = vec![false; 25];
        px[12] = [1.0; 4];
        covered[12] = true;
        dilate(&mut px, &covered, 5, 1);
        assert_eq!(px.iter().filter(|p| p[0] == 1.0).count(), 9);
        assert_eq!(px[0], [0.0; 4]);
        dilate(&mut px, &covered, 5, 4);
        assert!(px.iter().all(|p| p[0] == 1.0));
    }
}

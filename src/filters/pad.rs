//! Padding: every texel outside a kept set takes the value of the nearest kept texel (a breadth-first flood
//! outwards from the kept texels, eight neighbours), so a neighbourhood kernel run over the result reads only
//! values of the kept set.

use std::collections::VecDeque;

/// Replaces the texels of `px` (`w` x `h`, row by row) where `keep` is false with the value of the nearest
/// texel where it is true. Returns false (and changes nothing) when no texel is kept.
pub fn pad(px: &mut [[f32; 4]], keep: &[bool], w: usize, h: usize) -> bool {
    let n = w.saturating_mul(h).min(px.len()).min(keep.len());
    let mut done: Vec<bool> = keep.iter().take(n).copied().collect();
    let mut queue: VecDeque<usize> = (0..n).filter(|&i| done.get(i).copied().unwrap_or(false)).collect();
    if queue.is_empty() {
        return false;
    }
    if queue.len() == n {
        return true;
    }
    while let Some(i) = queue.pop_front() {
        let Some(value) = px.get(i).copied() else { continue };
        let (x, y) = ((i % w) as i64, (i / w) as i64);
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)] {
            let (nx, ny) = (x + dx, y + dy);
            if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                continue;
            }
            let j = ny as usize * w + nx as usize;
            if let Some(d) = done.get_mut(j).filter(|d| !**d) {
                *d = true;
                if let Some(p) = px.get_mut(j) {
                    *p = value;
                }
                queue.push_back(j);
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_from_the_nearest_kept_texel() {
        let mut px = vec![[0.0; 4], [1.0; 4], [9.0; 4], [9.0; 4], [2.0; 4]];
        let keep = [true, true, false, false, true];
        assert!(pad(&mut px, &keep, 5, 1));
        assert_eq!(px.iter().map(|p| p[0]).collect::<Vec<_>>(), [0.0, 1.0, 1.0, 2.0, 2.0]);
        let mut none = vec![[5.0; 4]; 2];
        assert!(!pad(&mut none, &[false, false], 2, 1));
        assert_eq!(none[0][0], 5.0);
    }
}

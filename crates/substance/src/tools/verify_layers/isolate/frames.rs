//! [`FrameGrid`]: full-resolution composites shrunk to the compared size by averaging only the texels on the
//! mesh (Painter's cache holds padding elsewhere, which no layer makes). A compared texel counts when at least
//! half of its block lies on the mesh. [`Independence`]: which texels do not change across nuisance states.

use super::inverse::STEP;
use crate::translate::layers::island_texels::IslandTexels;
use materialcraft::tex::Tex;

/// One composite at the compared size: the first `components` of each texel.
pub type Frame = Vec<[f32; 3]>;

#[derive(Debug, Clone)]
pub struct FrameGrid {
    pub full: (usize, usize),
    pub size: (usize, usize),
    /// Per compared texel: the island texels averaged into it.
    counts: Vec<u32>,
    /// Per full texel: its compared texel, or `usize::MAX` off the mesh.
    target: Vec<usize>,
}

impl FrameGrid {
    /// A grid from `full` down to at most `max_size` on the longest side; `islands` of the full size (else
    /// every texel counts).
    pub fn new(full: (usize, usize), max_size: usize, islands: Option<&IslandTexels>) -> FrameGrid {
        let longest = full.0.max(full.1).max(1);
        let scale = |v: usize| {
            if longest > max_size {
                (v * max_size / longest).max(1)
            } else {
                v.max(1)
            }
        };
        let size = (scale(full.0), scale(full.1));
        let islands = islands.filter(|m| m.size() == full);
        let mut counts = vec![0u32; size.0 * size.1];
        let mut target = vec![usize::MAX; full.0 * full.1];
        for y in 0..full.1 {
            for x in 0..full.0 {
                if islands.is_some_and(|m| !m.contains(x, y)) {
                    continue;
                }
                let t = (y * size.1 / full.1) * size.0 + x * size.0 / full.0;
                counts[t] += 1;
                target[y * full.0 + x] = t;
            }
        }
        FrameGrid {
            full,
            size,
            counts,
            target,
        }
    }

    /// Compared texels with at least half their block on the mesh.
    pub fn valid(&self) -> Vec<bool> {
        let block = (self.full.0 * self.full.1) as f32 / (self.size.0 * self.size.1) as f32;
        self.counts.iter().map(|c| *c as f32 >= 0.5 * block && *c > 0).collect()
    }

    /// `tex` (the full size) averaged onto the grid.
    pub fn frame(&self, tex: &Tex) -> Frame {
        let mut sums = vec![[0.0f64; 3]; self.counts.len()];
        if tex.w == self.full.0 && tex.h == self.full.1 {
            for (p, t) in tex.px.iter().zip(&self.target) {
                if let Some(s) = sums.get_mut(*t) {
                    for k in 0..3 {
                        s[k] += f64::from(p[k]);
                    }
                }
            }
        }
        sums.iter()
            .zip(&self.counts)
            .map(|(s, c)| match *c {
                0 => [0.0; 3],
                c => [0, 1, 2].map(|k| (s[k] / f64::from(c)) as f32),
            })
            .collect()
    }
}

/// Texels whose composite stays the same, within half a step, whatever state the nuisances are in.
#[derive(Debug, Clone)]
pub struct Independence {
    /// Per probe value: the frame with every nuisance in its first state.
    reference: Vec<Frame>,
    components: usize,
    dirty: Vec<bool>,
}

impl Independence {
    pub fn new(reference: Vec<Frame>, components: usize) -> Independence {
        let n = reference.first().map_or(0, Vec::len);
        Independence {
            reference,
            components: components.clamp(1, 3),
            dirty: vec![false; n],
        }
    }

    /// Another state's frame for probe value number `probe`.
    pub fn compare(&mut self, probe: usize, frame: &[[f32; 3]]) {
        let Some(reference) = self.reference.get(probe) else {
            return;
        };
        for ((d, a), b) in self.dirty.iter_mut().zip(reference).zip(frame) {
            if (0..self.components).any(|k| (a[k] - b[k]).abs() > 0.5 * STEP || !b[k].is_finite()) {
                *d = true;
            }
        }
    }

    pub fn clean(&self) -> Vec<bool> {
        self.dirty.iter().map(|d| !d).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_average_blocks_and_independence_marks_changes() {
        let grid = FrameGrid::new((4, 4), 2, None);
        assert_eq!(grid.size, (2, 2));
        let mut tex = Tex::solid(4, 4, [0.0, 0.0, 0.0, 1.0]);
        tex.px[0] = [1.0, 0.5, 0.0, 1.0];
        let f = grid.frame(&tex);
        assert!((f[0][0] - 0.25).abs() < 1e-6 && (f[0][1] - 0.125).abs() < 1e-6 && f[3] == [0.0; 3]);
        assert!(grid.valid().iter().all(|v| *v));
        let mut ind = Independence::new(vec![f.clone()], 1);
        let mut other = f.clone();
        other[3][0] += 0.1;
        other[2][1] += 0.1; // a component that is not compared
        ind.compare(0, &other);
        assert_eq!(ind.clean(), vec![true, true, true, false]);
        assert!(
            grid.frame(&Tex::solid(2, 2, [1.0; 4])).iter().all(|p| *p == [0.0; 3]),
            "wrong size"
        );
    }
}

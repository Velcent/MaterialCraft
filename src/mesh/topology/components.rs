//! Grouping triangles into connected components (UV islands, connected pieces).

/// A numbering of triangles into groups `0..count`. Groups are numbered in the order of their first triangle, so
/// the numbering only depends on the mesh. Triangles that could not be read (bad indices) belong to no group.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Components {
    of_triangle: Vec<u32>,
    count: u32,
}

const NONE: u32 = u32::MAX;

impl Components {
    /// The group of triangle `tri`, or `None` when it is out of range or belongs to no group.
    pub fn of(&self, tri: usize) -> Option<u32> {
        self.of_triangle.get(tri).copied().filter(|&c| c != NONE)
    }

    /// How many groups there are.
    pub fn count(&self) -> u32 {
        self.count
    }

    /// The triangles of group `group`, in triangle order.
    pub fn triangles(&self, group: u32) -> impl Iterator<Item = usize> + '_ {
        self.of_triangle.iter().enumerate().filter(move |&(_, &c)| c == group && c != NONE).map(|(t, _)| t)
    }
}

/// Union-find over triangle indices.
pub(super) struct Joiner {
    parent: Vec<u32>,
    valid: Vec<bool>,
}

impl Joiner {
    /// `valid[t]` says whether triangle `t` takes part; others end up in no group.
    pub(super) fn new(valid: Vec<bool>) -> Joiner {
        Joiner { parent: (0..valid.len() as u32).collect(), valid }
    }

    fn root(&mut self, mut t: u32) -> u32 {
        // path halving; every parent is a valid index by construction
        while let Some(&p) = self.parent.get(t as usize) {
            if p == t {
                break;
            }
            let grand = self.parent.get(p as usize).copied().unwrap_or(p);
            if let Some(slot) = self.parent.get_mut(t as usize) {
                *slot = grand;
            }
            t = grand;
        }
        t
    }

    pub(super) fn join(&mut self, a: u32, b: u32) {
        let (ra, rb) = (self.root(a), self.root(b));
        if ra == rb {
            return;
        }
        // the smaller root wins, which keeps the result independent of join order
        let (keep, drop) = if ra < rb { (ra, rb) } else { (rb, ra) };
        if let Some(slot) = self.parent.get_mut(drop as usize) {
            *slot = keep;
        }
    }

    pub(super) fn finish(mut self) -> Components {
        let n = self.parent.len();
        let mut number_of_root = vec![NONE; n];
        let mut of_triangle = vec![NONE; n];
        let mut count = 0u32;
        for t in 0..n {
            if !self.valid.get(t).copied().unwrap_or(false) {
                continue;
            }
            let r = self.root(t as u32) as usize;
            let (Some(number), Some(out)) = (number_of_root.get_mut(r), of_triangle.get_mut(t)) else {
                continue;
            };
            if *number == NONE {
                *number = count;
                count += 1;
            }
            *out = *number;
        }
        Components { of_triangle, count }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_and_numbers_in_triangle_order() {
        let mut j = Joiner::new(vec![true, true, true, false, true]);
        j.join(4, 1);
        j.join(2, 4);
        let c = j.finish();
        assert_eq!(c.count(), 2);
        assert_eq!((c.of(0), c.of(1), c.of(2), c.of(3), c.of(4)), (Some(0), Some(1), Some(1), None, Some(1)));
        assert_eq!(c.triangles(1).collect::<Vec<_>>(), vec![1, 2, 4]);
        assert_eq!(c.of(99), None);
    }
}

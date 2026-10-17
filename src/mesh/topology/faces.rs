//! Face groups: which triangles came from one source polygon (an OBJ n-gon, a quad in Painter's mesh).
//!
//! Importers fan-triangulate polygons in order, so every face is a run of consecutive triangles. Polygon fill picks
//! by face use this to select the whole quad or n-gon the user sees in their modelling tool.

use std::ops::Range;

/// Consecutive runs of triangles that form one source face each.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FaceGroups {
    /// First triangle of every face, increasing; the last face runs to `triangle_count`.
    starts: Vec<u32>,
    triangle_count: u32,
}

impl FaceGroups {
    /// Every triangle is its own face (the source had triangles only, or its polygons are unknown).
    pub fn per_triangle(triangle_count: usize) -> FaceGroups {
        let n = clamp_u32(triangle_count);
        FaceGroups { starts: (0..n).collect(), triangle_count: n }
    }

    /// Faces from the corner counts of the source polygons, in order: a polygon with `k >= 3` corners covers `k - 2`
    /// triangles; points and lines cover none. Triangles left over after the last polygon become one face each;
    /// polygons beyond `triangle_count` are cut off.
    pub fn from_polygon_sizes(corner_counts: impl IntoIterator<Item = usize>, triangle_count: usize) -> FaceGroups {
        let n = clamp_u32(triangle_count);
        let mut starts = Vec::new();
        let mut next = 0u32;
        for k in corner_counts {
            if next >= n {
                break;
            }
            if k < 3 {
                continue;
            }
            starts.push(next);
            next = next.saturating_add(clamp_u32(k - 2));
        }
        starts.extend(next..n);
        FaceGroups { starts, triangle_count: n }
    }

    /// Faces from a per-triangle flag that is true on the first triangle of each polygon (as Painter's mesh records
    /// it). A run that does not begin with a flag still starts a face at triangle 0.
    pub fn from_polygon_starts(starts_polygon: impl IntoIterator<Item = bool>) -> FaceGroups {
        let mut starts = Vec::new();
        let mut n = 0u32;
        for (t, first) in starts_polygon.into_iter().enumerate() {
            if first || t == 0 {
                starts.push(t as u32);
            }
            n = n.saturating_add(1);
        }
        FaceGroups { starts, triangle_count: n }
    }

    /// Faces from runs of `(face count, triangles per face)`, in order (the `.mcpaint` storage, see [`Self::runs`]).
    /// Runs past `u32::MAX` triangles are cut off.
    pub fn from_runs(runs: impl IntoIterator<Item = (usize, usize)>) -> FaceGroups {
        let mut starts = Vec::new();
        let mut next = 0u32;
        for (count, size) in runs {
            let size = clamp_u32(size);
            if size == 0 {
                continue;
            }
            for _ in 0..count {
                let Some(after) = next.checked_add(size) else { return FaceGroups { starts, triangle_count: next } };
                starts.push(next);
                next = after;
            }
        }
        FaceGroups { starts, triangle_count: next }
    }

    /// The faces as runs of `(face count, triangles per face)`: a quad mesh is one run.
    pub fn runs(&self) -> Vec<(usize, usize)> {
        let mut runs: Vec<(usize, usize)> = Vec::new();
        for f in 0..self.count() {
            let size = self.triangles(f).len();
            match runs.last_mut() {
                Some((n, s)) if *s == size => *n += 1,
                _ => runs.push((1, size)),
            }
        }
        runs
    }

    /// True when every triangle is its own face (nothing to keep beyond the triangles).
    pub fn is_per_triangle(&self) -> bool {
        self.starts.len() == self.triangle_count as usize
    }

    /// How many faces there are.
    pub fn count(&self) -> usize {
        self.starts.len()
    }

    /// How many triangles the faces cover.
    pub fn triangle_count(&self) -> usize {
        self.triangle_count as usize
    }

    /// The face that triangle `tri` belongs to.
    pub fn face_of(&self, tri: usize) -> Option<usize> {
        if tri >= self.triangle_count as usize {
            return None;
        }
        // the last start that is <= tri
        Some(self.starts.partition_point(|&s| s as usize <= tri).saturating_sub(1))
    }

    /// The triangles of face `face`.
    pub fn triangles(&self, face: usize) -> Range<usize> {
        let start = self.starts.get(face).map_or(self.triangle_count, |&s| s) as usize;
        let end = self.starts.get(face + 1).map_or(self.triangle_count, |&s| s) as usize;
        start..end.max(start)
    }
}

impl crate::mesh::MeshPart {
    /// The triangles of the source face that triangle `tri` belongs to: its polygon when the part's
    /// [`FaceGroups`] fit its triangles, else `tri` alone (empty for a triangle the part does not have).
    pub fn face_triangles(&self, tri: usize) -> Range<usize> {
        if tri >= self.triangles.len() {
            return tri..tri;
        }
        if self.faces.triangle_count() == self.triangles.len()
            && let Some(f) = self.faces.face_of(tri)
        {
            return self.faces.triangles(f);
        }
        tri..tri + 1
    }
}

fn clamp_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polygon_sizes() {
        // quad, line (skipped), pentagon, triangle; one triangle more than the polygons cover
        let f = FaceGroups::from_polygon_sizes([4, 2, 5, 3], 7);
        assert_eq!(f.count(), 4);
        assert_eq!(f.triangles(0), 0..2);
        assert_eq!(f.triangles(1), 2..5);
        assert_eq!(f.triangles(2), 5..6);
        assert_eq!(f.triangles(3), 6..7);
        assert_eq!(f.face_of(4), Some(1));
        assert_eq!(f.face_of(7), None);
        assert_eq!(f.triangles(9), 7..7);
    }

    #[test]
    fn polygons_past_the_end_are_cut() {
        let f = FaceGroups::from_polygon_sizes([6, 4], 3);
        assert_eq!(f.count(), 1);
        assert_eq!(f.triangles(0), 0..3);
    }

    #[test]
    fn polygon_starts() {
        let f = FaceGroups::from_polygon_starts([false, false, true, true, false]);
        assert_eq!(f.count(), 3);
        assert_eq!(f.triangles(0), 0..2);
        assert_eq!(f.face_of(4), Some(2));
        assert_eq!(FaceGroups::per_triangle(3).face_of(2), Some(2));
    }

    #[test]
    fn runs_round_trip() {
        let f = FaceGroups::from_polygon_sizes([4, 4, 4, 5, 3, 3], 11);
        assert_eq!(f.runs(), vec![(3, 2), (1, 3), (2, 1)]);
        assert_eq!(FaceGroups::from_runs(f.runs()), f);
        assert!(FaceGroups::per_triangle(4).is_per_triangle() && !f.is_per_triangle());
        assert_eq!(FaceGroups::from_runs([(2, 0), (1, 2)]).triangle_count(), 2, "empty faces are skipped");
    }

    #[test]
    fn parts_answer_face_triangles_with_a_fallback() {
        let mut part = crate::mesh::MeshPart { triangles: vec![[0, 1, 2]; 3], ..Default::default() };
        assert_eq!(part.face_triangles(1), 1..2, "unknown faces: the triangle alone");
        part.faces = FaceGroups::from_polygon_sizes([4, 3], 3);
        assert_eq!((part.face_triangles(1), part.face_triangles(2), part.face_triangles(3)), (0..2, 2..3, 3..3));
        part.triangles.push([0, 1, 2]);
        assert_eq!(part.face_triangles(1), 1..2, "faces that no longer fit are ignored");
    }
}

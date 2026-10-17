//! [`PickRegion`] and the 2D overlap tests behind it.

/// What the user marked, in one flat space (screen units for the 3D view, UV units for the 2D view).
#[derive(Debug, Clone, PartialEq)]
pub enum PickRegion {
    /// A click: the one triangle under the point.
    Point([f32; 2]),
    /// A dragged rectangle between two corners: every triangle it touches.
    Rect([f32; 2], [f32; 2]),
    /// A closed lasso outline (at least three points): every triangle it touches.
    Lasso(Vec<[f32; 2]>),
}

impl PickRegion {
    /// The outline of an area region (rectangle or lasso), `None` for a point or a lasso of fewer than three
    /// finite points.
    pub(super) fn outline(&self) -> Option<Vec<[f32; 2]>> {
        let pts = match self {
            PickRegion::Point(_) => return None,
            PickRegion::Rect(a, b) => vec![*a, [b[0], a[1]], *b, [a[0], b[1]]],
            PickRegion::Lasso(p) => p.iter().copied().filter(|q| q.iter().all(|c| c.is_finite())).collect(),
        };
        (pts.len() >= 3).then_some(pts)
    }
}

/// The axis-aligned bounds of `pts`.
pub(super) fn bounds(pts: &[[f32; 2]]) -> ([f32; 2], [f32; 2]) {
    let mut lo = [f32::INFINITY; 2];
    let mut hi = [f32::NEG_INFINITY; 2];
    for p in pts {
        lo = [lo[0].min(p[0]), lo[1].min(p[1])];
        hi = [hi[0].max(p[0]), hi[1].max(p[1])];
    }
    (lo, hi)
}

/// True if the triangle `t` and the closed polygon `outline` (any winding, may self-intersect; even-odd inside) share
/// a point. `outline_bounds` are [`bounds`] of the outline.
pub(super) fn touches(t: [[f32; 2]; 3], outline: &[[f32; 2]], outline_bounds: ([f32; 2], [f32; 2])) -> bool {
    let (lo, hi) = outline_bounds;
    let outside = |below: &dyn Fn(&[f32; 2]) -> bool| t.iter().all(below);
    if outside(&|c| c[0] < lo[0]) || outside(&|c| c[0] > hi[0]) || outside(&|c| c[1] < lo[1]) || outside(&|c| c[1] > hi[1]) {
        return false;
    }
    if t.iter().any(|c| inside_polygon(outline, *c)) {
        return true;
    }
    if outline.first().is_some_and(|p| in_triangle(t, *p)) {
        return true;
    }
    let n = outline.len();
    for (i, a) in outline.iter().enumerate() {
        let Some(b) = outline.get((i + 1) % n) else { continue };
        for e in 0..3 {
            if let (Some(p), Some(q)) = (t.get(e), t.get((e + 1) % 3))
                && segments_cross(*a, *b, *p, *q)
            {
                return true;
            }
        }
    }
    false
}

/// Even-odd point-in-polygon test.
pub(super) fn inside_polygon(poly: &[[f32; 2]], q: [f32; 2]) -> bool {
    let mut inside = false;
    let n = poly.len();
    for (i, a) in poly.iter().enumerate() {
        let Some(b) = poly.get((i + n - 1) % n) else { continue };
        if (a[1] > q[1]) != (b[1] > q[1]) {
            let x = a[0] + (q[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
            if q[0] < x {
                inside = !inside;
            }
        }
    }
    inside
}

/// True if `q` lies in the triangle (either winding, edges included).
pub(super) fn in_triangle(t: [[f32; 2]; 3], q: [f32; 2]) -> bool {
    let side = |a: [f32; 2], b: [f32; 2]| (b[0] - a[0]) * (q[1] - a[1]) - (b[1] - a[1]) * (q[0] - a[0]);
    let s = [side(t[0], t[1]), side(t[1], t[2]), side(t[2], t[0])];
    s.iter().all(|v| *v >= 0.0) || s.iter().all(|v| *v <= 0.0)
}

/// True if segments `ab` and `pq` intersect (touching counts).
fn segments_cross(a: [f32; 2], b: [f32; 2], p: [f32; 2], q: [f32; 2]) -> bool {
    let orient = |o: [f32; 2], u: [f32; 2], v: [f32; 2]| (u[0] - o[0]) * (v[1] - o[1]) - (u[1] - o[1]) * (v[0] - o[0]);
    let (d1, d2) = (orient(p, q, a), orient(p, q, b));
    let (d3, d4) = (orient(a, b, p), orient(a, b, q));
    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0)) && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0)) {
        return true;
    }
    let on = |o: [f32; 2], u: [f32; 2], v: [f32; 2], d: f32| {
        d == 0.0 && v[0] >= o[0].min(u[0]) && v[0] <= o[0].max(u[0]) && v[1] >= o[1].min(u[1]) && v[1] <= o[1].max(u[1])
    };
    on(p, q, a, d1) || on(p, q, b, d2) || on(a, b, p, d3) || on(a, b, q, d4)
}

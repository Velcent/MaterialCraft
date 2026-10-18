//! The 2D placement handle of UV mode (PJ-25) in UV coordinates: where one repetition of the image lies in UV
//! space ([`UvFrame`]), its handles, and [`UvDrag`]: drag inside to move (optionally locked to one axis), outside
//! to turn about a pivot (the frame's centre unless given), the edge handles to scale one axis and the corners to
//! scale both keeping their ratio, all symmetric about the centre.
//!
//! The pointer is given in UV coordinates (the GUI picks the mesh under it); conventions are those of
//! [`super::super::UvPlacement`]: the image's centre sits at the UV centre plus the offset, a positive rotation
//! turns the image from +u towards +v, and the frame of one repetition is `1 / tiling` wide.

use super::super::data::ImagePlacement;
use super::handles::{Shape, ShapeKind};
use super::maths::snap;

/// One grabbable part of the UV handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UvHandle {
    Move,
    Rotate,
    /// Scale along the image's u (0) or v (1) axis.
    ScaleAxis(usize),
    /// Scale both axes, keeping their ratio.
    ScaleUniform,
}

/// A part of the UV handle in UV coordinates.
pub type UvShape = Shape<UvHandle, [f32; 2]>;

/// One repetition of the image in UV space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UvFrame {
    pub centre: [f32; 2],
    /// Unit image axes u and v in UV space.
    pub axes: [[f32; 2]; 2],
    /// Half the frame's width and height in UV units (signed: mirrored axes are negative).
    pub half: [f32; 2],
}

impl UvFrame {
    pub fn of(p: &ImagePlacement) -> UvFrame {
        let finite = |v: f32, d: f32| if v.is_finite() { v } else { d };
        let (s, c) = finite(p.rotation, 0.0).to_radians().sin_cos();
        let tiling = p.tiling.map(|t| {
            let t = finite(t, 1.0);
            if t.abs() < 1e-6 { 1e-6f32.copysign(t) } else { t }
        });
        UvFrame { centre: [0.5 + finite(p.offset[0], 0.0), 0.5 + finite(p.offset[1], 0.0)], axes: [[c, s], [-s, c]], half: [0.5 / tiling[0], 0.5 / tiling[1]] }
    }

    /// The UV point at frame coordinates `f` (−1 … 1 from the centre to the edges along the image axes).
    pub fn point(&self, f: [f32; 2]) -> [f32; 2] {
        let (a, b) = (f[0] * self.half[0], f[1] * self.half[1]);
        [self.centre[0] + a * self.axes[0][0] + b * self.axes[1][0], self.centre[1] + a * self.axes[0][1] + b * self.axes[1][1]]
    }

    /// The UV point `uv` in UV units along the image axes from the centre.
    pub fn local(&self, uv: [f32; 2]) -> [f32; 2] {
        let d = [uv[0] - self.centre[0], uv[1] - self.centre[1]];
        [d[0] * self.axes[0][0] + d[1] * self.axes[0][1], d[0] * self.axes[1][0] + d[1] * self.axes[1][1]]
    }

    /// True if `uv` lies inside the frame.
    pub fn contains(&self, uv: [f32; 2]) -> bool {
        let l = self.local(uv);
        l[0].abs() <= self.half[0].abs() && l[1].abs() <= self.half[1].abs()
    }

    /// The outline (closed) with `per_edge` points on each edge, for drawing it over the mesh.
    pub fn outline(&self, per_edge: usize) -> Vec<[f32; 2]> {
        let n = per_edge.max(1);
        let corners = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0], [-1.0, -1.0]];
        let mut out = Vec::with_capacity(4 * n + 1);
        for w in corners.windows(2) {
            let (Some(a), Some(b)) = (w.first(), w.get(1)) else { continue };
            for i in 0..n {
                let t = i as f32 / n as f32;
                out.push(self.point([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]));
            }
        }
        out.push(self.point([-1.0, -1.0]));
        out
    }

    /// The handles: corners scale both axes, edge midpoints one, the centre and the inside move. Turning is
    /// what a drag outside them does (see [`UvHandle::Rotate`]).
    pub fn shapes(&self) -> Vec<UvShape> {
        let knob = |handle, f: [f32; 2]| Shape { handle, kind: ShapeKind::Knob, points: vec![self.point(f)] };
        let mut out = vec![Shape {
            handle: UvHandle::Move,
            kind: ShapeKind::Area,
            points: [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]].map(|f| self.point(f)).to_vec(),
        }];
        out.extend([[1.0, 0.0], [-1.0, 0.0]].map(|f| knob(UvHandle::ScaleAxis(0), f)));
        out.extend([[0.0, 1.0], [0.0, -1.0]].map(|f| knob(UvHandle::ScaleAxis(1), f)));
        out.extend([[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]].map(|f| knob(UvHandle::ScaleUniform, f)));
        out.push(knob(UvHandle::Move, [0.0, 0.0]));
        out
    }
}

/// One drag of the UV handle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UvDrag {
    pub handle: UvHandle,
    start: ImagePlacement,
    frame: UvFrame,
    from: [f32; 2],
    pivot: [f32; 2],
}

impl UvDrag {
    /// Starts dragging `handle` of `placement`'s frame with the pointer at UV `from`; turns go about `pivot` (the
    /// frame's centre without one).
    pub fn begin(handle: UvHandle, placement: &ImagePlacement, from: [f32; 2], pivot: Option<[f32; 2]>) -> UvDrag {
        let frame = UvFrame::of(placement);
        UvDrag { handle, start: *placement, frame, from, pivot: pivot.unwrap_or(frame.centre) }
    }

    /// The placement with the pointer at UV `to`. `lock_axis` keeps a move on the axis (u or v) it mostly
    /// follows; `angle_step` turns to whole multiples of that many degrees.
    pub fn update(&self, to: [f32; 2], lock_axis: bool, angle_step: Option<f32>) -> ImagePlacement {
        let mut p = self.start;
        match self.handle {
            UvHandle::Move => {
                let mut d = [to[0] - self.from[0], to[1] - self.from[1]];
                if lock_axis {
                    if d[0].abs() >= d[1].abs() {
                        d[1] = 0.0;
                    } else {
                        d[0] = 0.0;
                    }
                }
                p.offset = [p.offset[0] + d[0], p.offset[1] + d[1]];
            }
            UvHandle::Rotate => {
                let angle = |q: [f32; 2]| {
                    let (x, y) = (q[0] - self.pivot[0], q[1] - self.pivot[1]);
                    (x * x + y * y > 1e-12).then(|| y.atan2(x).to_degrees())
                };
                let (Some(a0), Some(a1)) = (angle(self.from), angle(to)) else { return p };
                let mut delta = (a1 - a0 + 180.0).rem_euclid(360.0) - 180.0;
                if let Some(step) = angle_step {
                    delta = snap(self.start.rotation + delta, step) - self.start.rotation;
                }
                p.rotation = self.start.rotation + delta;
                // the centre turns about the pivot with the image
                let (s, c) = delta.to_radians().sin_cos();
                let (x, y) = (self.frame.centre[0] - self.pivot[0], self.frame.centre[1] - self.pivot[1]);
                let centre = [self.pivot[0] + c * x - s * y, self.pivot[1] + s * x + c * y];
                p.offset = [centre[0] - 0.5, centre[1] - 0.5];
            }
            UvHandle::ScaleAxis(k) => {
                let (l0, l1) = (self.frame.local(self.from), self.frame.local(to));
                let (a, b) = (l0.get(k).copied().unwrap_or(0.0), l1.get(k).copied().unwrap_or(0.0));
                if a.abs() > 1e-9
                    && let Some(t) = p.tiling.get_mut(k)
                {
                    *t /= (b / a).max(1e-3);
                }
            }
            UvHandle::ScaleUniform => {
                let (l0, l1) = (self.frame.local(self.from), self.frame.local(to));
                let (r0, r1) = (l0[0].hypot(l0[1]), l1[0].hypot(l1[1]));
                if r0 > 1e-9 {
                    let f = (r1 / r0).max(1e-3);
                    p.tiling = p.tiling.map(|t| t / f);
                }
            }
        }
        p
    }
}

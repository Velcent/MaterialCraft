//! The 3D gizmo's parts (PJ-23): which [`Handle`]s each [`GizmoMode`] shows, and their geometry in object space
//! ([`Gizmo::shapes`]) for drawing and hit tests alike.

use super::maths::{column, unit_axis};
use super::placed::PlacedVolume;
use crate::mesh::Vec3;
use crate::v3::{add, mul};

/// Segments of a rotation ring.
const RING_SEGMENTS: usize = 64;

/// What the gizmo shows: one transform gizmo with three modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GizmoMode {
    #[default]
    Move,
    Rotate,
    Scale,
}

impl GizmoMode {
    pub const ALL: [GizmoMode; 3] = [GizmoMode::Move, GizmoMode::Rotate, GizmoMode::Scale];

    pub fn label(self) -> &'static str {
        match self {
            GizmoMode::Move => "Move",
            GizmoMode::Rotate => "Rotate",
            GizmoMode::Scale => "Scale",
        }
    }
}

/// Which axes moves and turns follow: the volume's own or the object's. Scaling always follows the volume's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Space {
    #[default]
    Local,
    World,
}

/// One grabbable part of the gizmo. Axes are 0 = x, 1 = y, 2 = z of the gizmo's [`Space`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handle {
    /// Move along one axis.
    MoveAxis(usize),
    /// Move in the plane across one axis.
    MovePlane(usize),
    /// Move in the plane facing the camera.
    MoveView,
    /// Turn about one axis.
    RotateAxis(usize),
    /// Turn freely (a trackball around the centre).
    RotateFree,
    /// Scale along one of the volume's axes.
    ScaleAxis(usize),
    /// Scale all three axes together.
    ScaleUniform,
}

impl Handle {
    /// The axis the handle belongs to (for its colour), `None` for the centre handles.
    pub fn axis(self) -> Option<usize> {
        match self {
            Handle::MoveAxis(k) | Handle::MovePlane(k) | Handle::RotateAxis(k) | Handle::ScaleAxis(k) => Some(k),
            Handle::MoveView | Handle::RotateFree | Handle::ScaleUniform => None,
        }
    }
}

/// How a part is drawn and hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeKind {
    /// An open polyline.
    Line,
    /// A filled polygon.
    Area,
    /// A round knob at the first point.
    Knob,
}

/// One grabbable part: handle `H`, drawn as `kind` through points of type `P`.
#[derive(Debug, Clone, PartialEq)]
pub struct Shape<H, P> {
    pub handle: H,
    pub kind: ShapeKind,
    pub points: Vec<P>,
}

/// A part of the 3D gizmo in object space.
pub type HandleShape = Shape<Handle, Vec3>;

/// The gizmo of one volume: its centre and axes in object space and the length of its arms (object units; the
/// GUI picks it so the gizmo keeps its size on screen, see [`super::length_for_pixels`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gizmo {
    pub centre: Vec3,
    /// Unit axes x, y and z of the gizmo's space.
    pub axes: [Vec3; 3],
    pub length: f32,
    pub mode: GizmoMode,
}

impl Gizmo {
    /// The gizmo of `volume` in `space` (scaling always shows the volume's own axes).
    pub fn new(volume: &PlacedVolume, space: Space, mode: GizmoMode, length: f32) -> Gizmo {
        Gizmo { centre: volume.centre, axes: axes(volume, if mode == GizmoMode::Scale { Space::Local } else { space }), length, mode }
    }

    /// Axis `k` (z past it).
    pub fn axis(&self, k: usize) -> Vec3 {
        self.axes.get(k).copied().unwrap_or(self.axes[2])
    }

    /// Every part of the gizmo, the centre handles last (they are on top when drawn and win ties when hit).
    pub fn shapes(&self) -> Vec<HandleShape> {
        let (c, l) = (self.centre, self.length);
        let at = |k: usize, s: f32| add(c, mul(self.axis(k), s * l));
        let mut out = Vec::new();
        match self.mode {
            GizmoMode::Move => {
                for k in 0..3 {
                    out.push(HandleShape { handle: Handle::MoveAxis(k), kind: ShapeKind::Line, points: vec![at(k, 0.2), at(k, 1.0)] });
                    out.push(HandleShape { handle: Handle::MoveAxis(k), kind: ShapeKind::Knob, points: vec![at(k, 1.0)] });
                }
                for k in 0..3 {
                    let (u, v) = (self.axis((k + 1) % 3), self.axis((k + 2) % 3));
                    let p = |a: f32, b: f32| add(c, add(mul(u, a * l), mul(v, b * l)));
                    out.push(HandleShape {
                        handle: Handle::MovePlane(k),
                        kind: ShapeKind::Area,
                        points: vec![p(0.3, 0.3), p(0.5, 0.3), p(0.5, 0.5), p(0.3, 0.5)],
                    });
                }
                out.push(HandleShape { handle: Handle::MoveView, kind: ShapeKind::Knob, points: vec![c] });
            }
            GizmoMode::Rotate => {
                for k in 0..3 {
                    let (u, v) = (self.axis((k + 1) % 3), self.axis((k + 2) % 3));
                    let points = (0..=RING_SEGMENTS)
                        .map(|i| {
                            let a = i as f32 / RING_SEGMENTS as f32 * std::f32::consts::TAU;
                            add(c, add(mul(u, a.cos() * l), mul(v, a.sin() * l)))
                        })
                        .collect();
                    out.push(HandleShape { handle: Handle::RotateAxis(k), kind: ShapeKind::Line, points });
                }
                out.push(HandleShape { handle: Handle::RotateFree, kind: ShapeKind::Knob, points: vec![c] });
            }
            GizmoMode::Scale => {
                for k in 0..3 {
                    out.push(HandleShape { handle: Handle::ScaleAxis(k), kind: ShapeKind::Line, points: vec![at(k, 0.2), at(k, 0.9)] });
                    out.push(HandleShape { handle: Handle::ScaleAxis(k), kind: ShapeKind::Knob, points: vec![at(k, 0.9)] });
                }
                out.push(HandleShape { handle: Handle::ScaleUniform, kind: ShapeKind::Knob, points: vec![c] });
            }
        }
        out
    }
}

/// The unit axes of `space` for `volume`.
pub fn axes(volume: &PlacedVolume, space: Space) -> [Vec3; 3] {
    match space {
        Space::Local => [0, 1, 2].map(|k| column(&volume.rotation, k)),
        Space::World => [0, 1, 2].map(unit_axis),
    }
}

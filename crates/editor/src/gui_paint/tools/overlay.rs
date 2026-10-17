//! The brush tools' visual aids over the 3D view: the lazy mouse's ring and string (PT-35), the straight line's
//! dotted guide (PT-31), and symmetry's plane or axis, where it meets the mesh, and an extra cursor at each
//! symmetric place (PT-39). [`Overlay`] is plain view-space geometry built from the tools' state; drawing it is a
//! few egui shapes.

use super::super::document_state::OpenDocument;
use super::super::shell::tools::Tool;
use super::super::view_pick::ViewPicking;
use super::super::viewport::Hover;
use super::Painting;
use eframe::egui::{self, Color32, Pos2, Stroke};
use materialcraft::camera::Camera;
use materialcraft::mesh::{Mesh, Vec3};
use materialcraft::paint::brush::SizeSpace;
use materialcraft::projection::ViewProjector;
use materialcraft::stamp::screen_scale_at;
use materialcraft::symmetry::SymmetrySpec;
use materialcraft::symmetry::guides::{Guide, Meeting, guide, meeting};
use materialcraft_brush::stroke::line::SNAP_DEGREES;
use materialcraft_brush::stroke::snap_angle;

/// Most mesh cut segments drawn (a dense mesh's cut is still a line at this count).
const MAX_SEGMENTS: usize = 20_000;

/// What a part of the overlay shows (its colour).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aid {
    Lazy,
    Line,
    Plane,
    Meeting,
    Cursor,
}

impl Aid {
    fn color(self) -> Color32 {
        match self {
            Aid::Lazy => Color32::from_gray(150),
            Aid::Line => Color32::from_gray(225),
            Aid::Plane => Color32::from_rgba_unmultiplied(110, 190, 255, 170),
            Aid::Meeting => Color32::from_rgb(255, 205, 90),
            Aid::Cursor => Color32::from_gray(200),
        }
    }
}

/// View-space geometry of the aids (view coordinates from the view's top-left corner).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Overlay {
    pub rings: Vec<([f32; 2], f32, Aid)>,
    /// Segments; dotted ones are guides.
    pub lines: Vec<([f32; 2], [f32; 2], Aid, bool)>,
    pub dots: Vec<([f32; 2], Aid)>,
}

impl Overlay {
    /// Draws the overlay with `painter`, the view's top-left corner at `origin`.
    pub fn draw(&self, painter: &egui::Painter, origin: Pos2) {
        let at = |p: [f32; 2]| Pos2::new(origin.x + p[0], origin.y + p[1]);
        for (a, b, aid, dotted) in &self.lines {
            let stroke = Stroke::new(if *aid == Aid::Meeting { 1.5 } else { 1.0 }, aid.color());
            if *dotted {
                painter.extend(egui::Shape::dashed_line(&[at(*a), at(*b)], stroke, 4.0, 4.0));
            } else {
                painter.line_segment([at(*a), at(*b)], stroke);
            }
        }
        for (c, r, aid) in &self.rings {
            painter.circle_stroke(at(*c), *r, Stroke::new(1.0, aid.color()));
        }
        for (c, aid) in &self.dots {
            painter.circle_filled(at(*c), 3.0, aid.color());
        }
    }

    fn segment(&mut self, view: &dyn ViewProjector, a: Vec3, b: Vec3, aid: Aid) {
        if let (Some(p), Some(q)) = (view.to_screen(a), view.to_screen(b)) {
            self.lines.push((p.pos, q.pos, aid, false));
        }
    }
}

/// Where the current symmetry meets the current mesh, kept until either changes.
#[derive(Default)]
pub struct GuideCache {
    key: Option<(SymmetrySpec, usize, usize)>,
    meeting: Meeting,
}

impl GuideCache {
    fn get(&mut self, spec: SymmetrySpec, mesh: &Mesh) -> &Meeting {
        let id = mesh.parts.first().map_or(0, |p| p.positions.as_ptr() as usize);
        let count = mesh.parts.iter().map(|p| p.triangles.len()).sum();
        let key = Some((spec, id, count));
        if self.key != key {
            self.meeting = meeting(spec, mesh);
            self.key = key;
        }
        &self.meeting
    }
}

impl Painting {
    /// The aids to draw this frame with the pointer at `hover` and brush tool `tool`.
    pub fn overlay(&mut self, doc: &OpenDocument, picking: &ViewPicking, camera: Option<Camera>, hover: Option<Hover>, tool: Tool) -> Overlay {
        let mut out = Overlay::default();
        let Some(preset) = self.brush.preset(tool).cloned() else { return out };
        let mesh = &doc.doc().mesh;
        let projector = camera.and_then(|c| picking.projector_on(mesh, c));
        let view = projector.as_ref().map(|p| p as &dyn ViewProjector);
        let helpers = self.brush.helpers;
        let stroking = self.active.is_some();
        // lazy mouse: the ring around the pointer and the string to the brush
        if helpers.lazy.on
            && let Some(h) = hover
        {
            out.rings.push((h.pos, helpers.lazy.radius.min(materialcraft_brush::LazyMouse::MAX_RADIUS), Aid::Lazy));
        }
        if let Some((brush, pointer)) = self.active.as_ref().and_then(|a| a.string()) {
            out.lines.push((brush, pointer, Aid::Lazy, false));
        }
        // the straight line from the last stroke's end
        if let (Some(h), Some(snap), false, Some(anchor)) = (hover, hover.and_then(|h| h.line), stroking, self.anchor)
            && let Some(from) = anchor.start(view)
        {
            let to = if snap { snap_angle(from, h.pos, SNAP_DEGREES) } else { h.pos };
            out.lines.push((from, to, Aid::Line, true));
        }
        let spec = helpers.symmetry.spec();
        let (Some(view), true) = (view, spec.is_active()) else { return out };
        let aids = helpers.symmetry.aids;
        if aids.plane
            && let Some(bounds) = mesh.bounds()
        {
            match guide(spec, bounds) {
                Some(Guide::Plane(c)) => {
                    for (i, a) in c.iter().enumerate() {
                        out.segment(view, *a, c[(i + 1) % 4], Aid::Plane);
                    }
                }
                Some(Guide::Axis([a, b])) => out.segment(view, a, b, Aid::Plane),
                None => {}
            }
        }
        if aids.meeting {
            let m = self.guides.get(spec, mesh);
            for [a, b] in m.lines.iter().take(MAX_SEGMENTS) {
                out.segment(view, *a, *b, Aid::Meeting);
            }
            for p in &m.points {
                if let Some(s) = view.to_screen(*p) {
                    out.dots.push((s.pos, Aid::Meeting));
                }
            }
        }
        // an extra cursor at each symmetric place of the surface under the pointer
        let hide = stroking && aids.hide_cursors_while_painting;
        if aids.cursors
            && !hide
            && let Some(hit) = hover.and_then(|h| view.pick(h.pos))
        {
            let model = mesh.bounding_sphere().map_or(1.0, |s| s.radius);
            for copy in spec.copies().iter().skip(1) {
                let (p, n) = (copy.point(hit.position), copy.normal(hit.normal));
                let Some(s) = view.to_screen(p) else { continue };
                if view.facing(p, n) <= 0.0 {
                    continue;
                }
                let radius = match preset.size_space {
                    SizeSpace::Model => screen_scale_at(view, p).map_or(0.0, |k| preset.size * model * k) * 0.5,
                    SizeSpace::Screen | SizeSpace::Texture => preset.size * 0.5,
                };
                out.rings.push((s.pos, radius.max(2.0), Aid::Cursor));
            }
        }
        out
    }
}

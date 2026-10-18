//! [`GizmoView`]: the projection gizmo of the selected fill as it lies on screen this frame, built from the
//! document, the view projector and (for UV mode) the texture set's surface map. Drawing and hit tests use the same
//! shapes ([`materialcraft::projection::gizmo::hit`]).

use super::GizmoSettings;
use materialcraft::mesh::{SurfaceMap, TexelKind, Vec3};
use materialcraft::paint::edit::ops::FillRef;
use materialcraft::paint::{Document, Fill};
use materialcraft::projection::gizmo::{self, Gizmo, Handle, PlacedVolume, ScreenShape, UvFrame, UvHandle, length_for_pixels};
use materialcraft::projection::{ProjectionMode, SceneFrame, ViewProjector};

/// Points per edge of the UV frame's outline drawn over the mesh.
const OUTLINE_POINTS: usize = 24;

/// Pointer distance (screen units) that still grabs a line, and the radius of a knob.
pub const LINE_TOLERANCE: f32 = 7.0;
pub const KNOB_RADIUS: f32 = 8.0;

/// The 3D gizmo of a volume mode on screen.
pub struct VolumeView {
    pub gizmo: Gizmo,
    pub shapes: Vec<ScreenShape<Handle>>,
    /// The volume's box (screen segments).
    pub outline: Vec<[[f32; 2]; 2]>,
    /// Where a planar projection looks from (the +z face's centre to the box's centre).
    pub direction: Option<[[f32; 2]; 2]>,
    /// The mode can be snapped onto the surface.
    pub snaps: bool,
}

/// The UV handle on screen: the frame of one image repetition laid over the mesh.
pub struct UvView {
    pub shapes: Vec<ScreenShape<UvHandle>>,
    /// The outline, broken where it leaves the mesh or is hidden behind it.
    pub outline: Vec<Vec<[f32; 2]>>,
    /// The texture set's material slot (picks on other sets do not move the handle).
    pub material: usize,
    /// The frame in UV space (a press inside it moves, outside turns).
    pub frame: UvFrame,
}

pub enum GizmoView {
    Volume(VolumeView),
    Uv(UvView),
}

/// What the pointer is over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GizmoHit {
    Volume(Handle),
    Uv(UvHandle),
}

impl GizmoView {
    /// The gizmo of `target`'s fill in `doc` seen through `view`, or `None` when the fill's mode has none (texture
    /// space, an unknown mode) or the gizmo is hidden. `surface` is the set's surface map, which UV mode needs to
    /// find its frame on the mesh.
    pub fn build(
        doc: &Document,
        target: &FillRef,
        view: &dyn ViewProjector,
        frame: Option<&SceneFrame>,
        surface: Option<&SurfaceMap>,
        settings: &GizmoSettings,
    ) -> Option<GizmoView> {
        if !settings.shown {
            return None;
        }
        let fill = target.fill(doc)?;
        match &fill.projection.mode {
            ProjectionMode::Uv => {
                let set = doc.texture_set(&target.layer.set)?;
                Some(GizmoView::Uv(uv_view(fill, view, surface?, set.material_slot)))
            }
            mode => {
                let volume = mode.volume()?;
                let placed = PlacedVolume::of(volume, frame?);
                let length = length_for_pixels(view, placed.centre, settings.size)?;
                let g = Gizmo::new(&placed, settings.space, settings.mode, length);
                let at = |p: Vec3| view.to_screen(p).map(|s| s.pos);
                let shapes = gizmo::to_screen(&g.shapes(), at);
                let outline = placed.edges().iter().filter_map(|[a, b]| Some([at(*a)?, at(*b)?])).collect();
                let direction = matches!(mode, ProjectionMode::Planar(_)).then(|| Some([at(placed.front())?, at(placed.centre)?])).flatten();
                Some(GizmoView::Volume(VolumeView { gizmo: g, shapes, outline, direction, snaps: mode.snaps_to_surface() }))
            }
        }
    }

    /// The part under `at`. In UV mode a press on the set's surface away from the knobs moves the image inside its
    /// frame and turns it outside.
    pub fn hit(&self, at: [f32; 2], view: &dyn ViewProjector, doc: &Document) -> Option<GizmoHit> {
        match self {
            GizmoView::Volume(v) => gizmo::pick(&v.shapes, at, LINE_TOLERANCE, KNOB_RADIUS).map(GizmoHit::Volume),
            GizmoView::Uv(v) => match gizmo::pick(&v.shapes, at, LINE_TOLERANCE, KNOB_RADIUS) {
                Some(h) => Some(GizmoHit::Uv(h)),
                None => uv_under(view, doc, v.material, at).map(|uv| GizmoHit::Uv(if v.frame.contains(uv) { UvHandle::Move } else { UvHandle::Rotate })),
            },
        }
    }
}

/// The UV coordinate of the set with material slot `material` under screen position `at`.
pub fn uv_under(view: &dyn ViewProjector, doc: &Document, material: usize, at: [f32; 2]) -> Option<[f32; 2]> {
    let hit = view.pick(at)?;
    (doc.mesh.parts.get(hit.part)?.material == material).then_some(hit.uv)
}

fn uv_view(fill: &Fill, view: &dyn ViewProjector, surface: &SurfaceMap, material: usize) -> UvView {
    let frame = UvFrame::of(&fill.projection.image);
    let on_screen = |uv: [f32; 2]| {
        let p = surface_point(surface, uv)?;
        if view.occluded(p) {
            return None;
        }
        view.to_screen(p).map(|s| s.pos)
    };
    let shapes = gizmo::to_screen(&frame.shapes(), on_screen);
    let mut outline: Vec<Vec<[f32; 2]>> = Vec::new();
    let mut run: Vec<[f32; 2]> = Vec::new();
    for uv in frame.outline(OUTLINE_POINTS) {
        match on_screen(uv) {
            Some(p) => run.push(p),
            None if run.len() > 1 => outline.push(std::mem::take(&mut run)),
            None => run.clear(),
        }
    }
    if run.len() > 1 {
        outline.push(run);
    }
    UvView { shapes, outline, material, frame }
}

/// The mesh point at UV `uv` of the set `surface` is the map of (covered texels only: the gutter is not on the
/// mesh).
fn surface_point(surface: &SurfaceMap, uv: [f32; 2]) -> Option<Vec3> {
    let (w, h) = (surface.width() as f32, surface.height() as f32);
    let (x, y) = (uv[0] * w, uv[1] * h);
    // the frame's edges at UV 0 and 1 count as the outermost texels
    if !(x >= 0.0 && y >= 0.0 && x <= w && y <= h) {
        return None;
    }
    let s = surface.sample((x as usize).min(surface.width().saturating_sub(1)), (y as usize).min(surface.height().saturating_sub(1)))?;
    (s.kind == TexelKind::Covered).then_some(s.position)
}

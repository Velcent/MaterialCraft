//! [`StampStroke`]: one stroke's stamps applied to texture-space images by pulling texels into each dab's frame.
//!
//! Per dab: find the cells that may lie under it ([`TexelIndex`]), map each of their texels into the dab's frame
//! and sample the coverage there, scale it by the texel's visibility and the stroke's factors, and build up the
//! stroke's coverage with the dab's flow towards its opacity ceiling (`c ← c + v · (ceiling − c)`, the rule of
//! PhotoCraft's stroke buffer). Each changed tile of each target image is then rewritten from its pixels before the
//! stroke and the stroke coverage, so the opacity ceiling holds over the whole stroke (PT-04).
//!
//! The camera does not move during a stroke, so each texel's screen position and visibility are computed once per
//! stroke (lazily, per cell) and reused by every later dab. Tiles are processed in parallel; each texel's result
//! depends only on the dabs in order, so a stroke gives the same bits however it is split up.

use super::dab::DabSample;
use super::factors::TexelFactor;
use super::frame::{DabFrame, screen_scale_at};
use super::index::{CELL, CELLS_PER_TILE, TexelIndex};
use super::occlusion::DepthMap;
use super::symmetry::{CopyTransform, SymmetrySpec};
use super::write::{Blend, ChannelTarget};
use crate::camera::Camera;
use crate::mesh::{SurfaceMap, Vec3};
use crate::paint::brush::Placement;
use crate::paint::{TILE_SIZE, TiledImage};
use crate::projection::ViewProjector;
use crate::v3::{len, sub};
use std::collections::BTreeMap;
use std::fmt;

const T: usize = TILE_SIZE as usize;

/// The view a 3D-view stroke is painted through.
#[derive(Clone, Copy)]
pub struct ViewSpace<'a> {
    pub projector: &'a dyn ViewProjector,
    /// The view's depth for the occlusion test; `None` paints hidden surfaces too.
    pub depth: Option<&'a DepthMap>,
    pub placement: Placement,
    /// Surfaces turned away from the view by more than this many degrees are not painted (PT-09).
    pub max_angle: f32,
}

/// The frame dabs are made in.
#[derive(Clone, Copy)]
pub enum StrokeSpace<'a> {
    /// 2D-view strokes: dab frames are texel coordinates of the texture set.
    Texture,
    /// 3D-view strokes: dab frames are screen positions of the view.
    View(ViewSpace<'a>),
}

/// Why a stamp could not be applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StampError {
    /// The targets differ from the ones the stroke started with (count or size).
    TargetsChanged,
    /// The surface map, its index and the images are not all the same size.
    SizeMismatch,
}

impl fmt::Display for StampError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            StampError::TargetsChanged => f.write_str("the images being painted changed during the stroke"),
            StampError::SizeMismatch => f.write_str("the surface map and the images being painted differ in size"),
        }
    }
}

impl std::error::Error for StampError {}

/// A texel as a 3D-view stroke sees it: computed once per stroke.
#[derive(Debug, Clone, Copy, Default)]
struct ViewTexel {
    pos: Vec3,
    screen: [f32; 2],
    /// Visibility × facing fade, 0 where the texel can't be painted from this view.
    weight: f32,
}

#[derive(Default)]
struct TileState {
    /// Stroke coverage per texel of the tile (row stride [`T`]).
    acc: Vec<f32>,
    /// Per symmetry copy and cell of the tile, the cell's texels as the view sees them (filled when first needed).
    views: Vec<Option<Box<[ViewTexel]>>>,
}

/// What a stroke's stamps are applied through, borrowed per call so a stroke can live across frames: the surface
/// map and its index, the frame dabs are made in, and the factors.
#[derive(Clone, Copy)]
pub struct StampContext<'a> {
    pub surface: &'a SurfaceMap,
    pub index: &'a TexelIndex,
    pub space: StrokeSpace<'a>,
    pub factors: &'a [&'a dyn TexelFactor],
}

/// What the per-stroke view caches were computed with; a call with anything else drops them.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ViewKey {
    camera: Camera,
    max_angle: f32,
    occlusion: bool,
}

/// One stroke into a fixed set of target images. See the module docs.
pub struct StampStroke {
    size: (usize, usize),
    copies: Vec<CopyTransform>,
    /// The targets' pixels before the stroke (sharing their tiles) and how each blends.
    before: Vec<(TiledImage, Blend)>,
    tiles: BTreeMap<usize, TileState>,
    tiles_x: usize,
    view_key: Option<ViewKey>,
}

impl StampStroke {
    /// A stroke into `targets` (all of one texture set's size) with the copies `symmetry` asks for (3D-view
    /// strokes only).
    pub fn new(symmetry: SymmetrySpec, targets: &[ChannelTarget]) -> Result<StampStroke, StampError> {
        let size = targets.first().map_or((0, 0), |t| t.image.size());
        if targets.iter().any(|t| t.image.size() != size) {
            return Err(StampError::SizeMismatch);
        }
        let (w, h) = (size.0 as usize, size.1 as usize);
        Ok(StampStroke {
            size: (w, h),
            copies: symmetry.copies(),
            before: targets.iter().map(|t| (t.image.clone(), Blend::new(t))).collect(),
            tiles: BTreeMap::new(),
            tiles_x: w.div_ceil(T),
            view_key: None,
        })
    }

    /// Every image tile the stroke has changed so far, as `(tx, ty)`.
    pub fn touched(&self) -> Vec<(u32, u32)> {
        let painted = self.tiles.iter().filter(|(_, s)| s.acc.iter().any(|&c| c > 0.0));
        painted.map(|(&t, _)| ((t % self.tiles_x) as u32, (t / self.tiles_x) as u32)).collect()
    }

    /// The stroke's coverage at texel `(x, y)` so far.
    pub fn coverage(&self, x: usize, y: usize) -> f32 {
        let t = (y / T) * self.tiles_x + x / T;
        self.tiles.get(&t).and_then(|s| s.acc.get((y % T) * T + x % T)).copied().unwrap_or(0.0)
    }

    /// Applies one dab through `ctx` to `targets` (the stroke's targets, in the same order); returns the image
    /// tiles it changed.
    pub fn apply(&mut self, ctx: &StampContext, dab: &DabSample, targets: &mut [ChannelTarget]) -> Result<Vec<(u32, u32)>, StampError> {
        if targets.len() != self.before.len() || targets.iter().zip(&self.before).any(|(t, (b, _))| t.image.size() != b.size()) {
            return Err(StampError::TargetsChanged);
        }
        if (ctx.surface.width(), ctx.surface.height()) != self.size || !ctx.index.fits(ctx.surface) {
            return Err(StampError::SizeMismatch);
        }
        let ceiling = if dab.opacity.is_finite() { dab.opacity.clamp(0.0, 1.0) } else { 0.0 };
        if ceiling <= 0.0 || dab.footprint.values.is_empty() {
            return Ok(Vec::new());
        }
        let limit = [self.size.0 as f32, self.size.1 as f32];
        let changed = match ctx.space {
            StrokeSpace::Texture => self.apply_texel_rect(ctx, dab, ceiling, TexelFrame { center: dab.center, anchor: dab.center, scale: 1.0, limit }),
            StrokeSpace::View(view) if view.placement == Placement::UvAligned => match uv_frame(ctx.surface, view.projector, dab) {
                Some(frame) => self.apply_texel_rect(ctx, dab, ceiling, frame),
                None => Vec::new(),
            },
            StrokeSpace::View(view) => self.apply_view(ctx, view, dab, ceiling),
        };
        let coords: Vec<(u32, u32)> = changed.iter().map(|&t| ((t % self.tiles_x) as u32, (t / self.tiles_x) as u32)).collect();
        if coords.is_empty() {
            return Ok(coords);
        }
        let tiles = &self.tiles;
        let tiles_x = self.tiles_x;
        for (target, (before, blend)) in targets.iter_mut().zip(&self.before) {
            target.image.update_tiles(&coords, |rect, px| {
                let ti = (rect.y as usize / T) * tiles_x + rect.x as usize / T;
                let Some(acc) = tiles.get(&ti).map(|s| &s.acc) else { return };
                let mut orig = vec![[0.0f32; 4]; rect.area()];
                before.read_region(rect, &mut orig);
                let w = rect.w as usize;
                for (i, (p, o)) in px.iter_mut().zip(&orig).enumerate() {
                    let c = acc.get((i / w) * T + i % w).copied().unwrap_or(0.0);
                    *p = blend.apply(*o, c);
                }
            });
        }
        Ok(coords)
    }

    /// Texture-space dabs: texels map into the frame by `frame` (an offset and a scale), no view involved.
    fn apply_texel_rect(&mut self, ctx: &StampContext, dab: &DabSample, ceiling: f32, frame: TexelFrame) -> Vec<usize> {
        let b = dab.footprint.bounds();
        // the texel rectangle the footprint covers
        let lo = frame.to_texel([b[0], b[1]]);
        let hi = frame.to_texel([b[2], b[3]]);
        let (x0, y0) = (lo[0].floor().max(0.0), lo[1].floor().max(0.0));
        let (x1, y1) = (hi[0].ceil().min(frame.limit[0]), hi[1].ceil().min(frame.limit[1]));
        if !(x0 < x1 && y0 < y1) {
            return Vec::new();
        }
        let (x0, y0, x1, y1) = (x0 as usize, y0 as usize, x1 as usize, y1 as usize);
        let tiles_x = self.tiles_x;
        let wanted: Vec<usize> = (y0 / T..=(y1 - 1) / T).flat_map(|ty| (x0 / T..=(x1 - 1) / T).map(move |tx| ty * tiles_x + tx)).collect();
        let factors = ctx.factors;
        for_tiles(&mut self.tiles, self.tiles_x, &wanted, |ti, state, tiles_x| {
            let (tx, ty) = ((ti % tiles_x) * T, (ti / tiles_x) * T);
            let mut changed = false;
            for y in y0.max(ty)..y1.min(ty + T) {
                for x in x0.max(tx)..x1.min(tx + T) {
                    let p = frame.to_frame([x as f32 + 0.5, y as f32 + 0.5]);
                    let mut v = dab.footprint.sample(p);
                    if v <= 0.0 {
                        continue;
                    }
                    v *= factor(factors, x, y, None);
                    changed |= build_up(state, (y - ty) * T + (x - tx), v, ceiling);
                }
            }
            changed
        })
    }

    /// 3D-view dabs: texels near the dab are found through the index and mapped by the dab's frame.
    fn apply_view(&mut self, ctx: &StampContext, view: ViewSpace, dab: &DabSample, ceiling: f32) -> Vec<usize> {
        let max_angle = if view.max_angle.is_finite() { view.max_angle.clamp(0.0, 180.0) } else { 180.0 };
        let key = ViewKey { camera: *view.projector.camera(), max_angle, occlusion: view.depth.is_some() };
        if self.view_key != Some(key) {
            // another view than the cached texels were seen from: see them again
            self.tiles.values_mut().for_each(|t| t.views.clear());
            self.view_key = Some(key);
        }
        let min_facing = max_angle.to_radians().cos();
        let b = dab.footprint.bounds();
        let reach = ((b[2] - b[0]).max(b[3] - b[1])) * 0.5 * std::f32::consts::SQRT_2;
        let frame = DabFrame::new(view.projector, view.placement, dab.center, reach);
        // cells per tile, per copy
        let mut work: BTreeMap<usize, Vec<(usize, usize)>> = BTreeMap::new();
        for (ci, copy) in self.copies.iter().enumerate() {
            let cells = ctx.index.cull(|aabb| {
                let moved = transform_box(*copy, aabb);
                frame.may_touch(view.projector, &moved, b)
            });
            for (tile, cell) in cells {
                work.entry(tile).or_default().push((ci, cell));
            }
        }
        if work.is_empty() {
            return Vec::new();
        }
        let wanted: Vec<usize> = work.keys().copied().collect();
        let (surface, copies, factors) = (ctx.surface, &self.copies, ctx.factors);
        let cells_x = ctx.index.cells_x();
        for_tiles(&mut self.tiles, self.tiles_x, &wanted, |ti, state, tiles_x| {
            let Some(cells) = work.get(&ti) else { return false };
            if state.views.is_empty() {
                state.views = vec![None; copies.len() * CELLS_PER_TILE * CELLS_PER_TILE];
            }
            let (tx, ty) = ((ti % tiles_x) * T, (ti / tiles_x) * T);
            let mut changed = false;
            for &(copy_i, cell) in cells {
                let (cx, cy) = ((cell % cells_x) * CELL, (cell / cells_x) * CELL);
                let slot = copy_i * CELLS_PER_TILE * CELLS_PER_TILE + ((cy - ty) / CELL) * CELLS_PER_TILE + (cx - tx) / CELL;
                let Some(copy) = copies.get(copy_i) else { continue };
                if state.views.get(slot).is_some_and(Option::is_none) {
                    let texels = view_cell(surface, view, *copy, cx, cy, min_facing);
                    if let Some(s) = state.views.get_mut(slot) {
                        *s = Some(texels);
                    }
                }
                let Some(Some(texels)) = state.views.get(slot) else { continue };
                for (k, t) in texels.iter().enumerate() {
                    if t.weight <= 0.0 {
                        continue;
                    }
                    let Some((p, fade)) = frame.map(t.pos, t.screen) else { continue };
                    let mut v = dab.footprint.sample(p);
                    if v <= 0.0 {
                        continue;
                    }
                    let (x, y) = (cx + k % CELL, cy + k / CELL);
                    v *= t.weight * fade * factor(factors, x, y, Some(t.screen));
                    changed |= build_up_at(&mut state.acc, (y - ty) * T + (x - tx), v, ceiling);
                }
            }
            changed
        })
    }
}

/// Runs `f(tile, state, tiles_x)` for each of `wanted` (sorted) in parallel; returns the tiles for which it reported
/// a change. Tile states are created as needed; ones left with nothing in them are dropped again.
fn for_tiles(tiles: &mut BTreeMap<usize, TileState>, tiles_x: usize, wanted: &[usize], f: impl Fn(usize, &mut TileState, usize) -> bool + Sync) -> Vec<usize> {
    for &t in wanted {
        tiles.entry(t).or_insert_with(|| TileState { acc: vec![0.0; T * T], views: Vec::new() });
    }
    let mut jobs: Vec<(usize, &mut TileState, bool)> = tiles.iter_mut().filter(|(k, _)| wanted.binary_search(k).is_ok()).map(|(k, s)| (*k, s, false)).collect();
    crate::paint::par::for_each_mut(&mut jobs, |_, (t, s, changed)| *changed = f(*t, s, tiles_x));
    let changed: Vec<usize> = jobs.iter().filter(|j| j.2).map(|j| j.0).collect();
    // keep states that hold coverage or view data; others were only touched by the cull
    tiles.retain(|_, s| !s.views.is_empty() || s.acc.iter().any(|&c| c > 0.0));
    changed
}

/// The UV-aligned frame of a dab in the 3D view: the texel under its centre and the screen size of a texel there.
fn uv_frame(surface: &SurfaceMap, projector: &dyn ViewProjector, dab: &DabSample) -> Option<TexelFrame> {
    let hit = projector.pick(dab.center)?;
    let (w, h) = (surface.width(), surface.height());
    let anchor = [hit.uv[0] * w as f32, hit.uv[1] * h as f32];
    let (ax, ay) = (anchor[0].floor().max(0.0) as usize, anchor[1].floor().max(0.0) as usize);
    let p0 = surface.point_index(ay * w + ax)?.0;
    let p1 = surface.point_index(ay * w + ax + 1).or_else(|| surface.point_index((ay + 1) * w + ax))?.0;
    let per_texel = len(sub(p1, p0));
    let scale = screen_scale_at(projector, hit.position)? * per_texel;
    (scale.is_finite() && scale > 0.0).then_some(TexelFrame { center: dab.center, anchor, scale, limit: [w as f32, h as f32] })
}

/// Maps texel positions into a texture-space dab frame: `frame = center + (texel − anchor) · scale`.
#[derive(Debug, Clone, Copy)]
struct TexelFrame {
    center: [f32; 2],
    anchor: [f32; 2],
    scale: f32,
    /// The texture's size in texels.
    limit: [f32; 2],
}

impl TexelFrame {
    fn to_frame(self, t: [f32; 2]) -> [f32; 2] {
        [self.center[0] + (t[0] - self.anchor[0]) * self.scale, self.center[1] + (t[1] - self.anchor[1]) * self.scale]
    }

    fn to_texel(self, f: [f32; 2]) -> [f32; 2] {
        [self.anchor[0] + (f[0] - self.center[0]) / self.scale, self.anchor[1] + (f[1] - self.center[1]) / self.scale]
    }
}

/// The texels of the cell at `(cx, cy)` as the view sees them through symmetry copy `copy`.
fn view_cell(surface: &SurfaceMap, view: ViewSpace, copy: CopyTransform, cx: usize, cy: usize, min_facing: f32) -> Box<[ViewTexel]> {
    let (w, h) = (surface.width(), surface.height());
    let mut out = vec![ViewTexel::default(); CELL * CELL];
    let camera = view.projector.camera();
    for (k, t) in out.iter_mut().enumerate() {
        let (x, y) = (cx + k % CELL, cy + k / CELL);
        if x >= w || y >= h {
            continue;
        }
        let Some((p, n, _)) = surface.point_index(y * w + x) else { continue };
        let (p, n) = (copy.point(p), copy.normal(n));
        let Some(s) = view.projector.to_screen(p) else { continue };
        let facing = view.projector.facing(p, n);
        if facing < min_facing {
            continue;
        }
        // a short fade above the limit avoids a hard rim
        let fade = ((facing - min_facing) / 0.05).clamp(0.0, 1.0);
        if let Some(depth) = view.depth {
            // two screen units of depth, more on steep surfaces, whose depth changes fast within one unit
            let unit = 1.0 / camera.screen_scale(s.view_depth).max(1e-12);
            let tolerance = 2.0 * unit / facing.max(0.15);
            if !depth.visible(s.pos, s.view_depth, tolerance) {
                continue;
            }
        }
        *t = ViewTexel { pos: p, screen: s.pos, weight: fade };
    }
    out.into_boxed_slice()
}

/// The box around `b`'s corners moved by `copy`.
fn transform_box(copy: CopyTransform, b: &super::index::Aabb) -> super::index::Aabb {
    match copy {
        CopyTransform::Identity => *b,
        _ => {
            let [first, rest @ ..] = b.corners().map(|p| copy.point(p));
            let mut out = super::index::Aabb::point(first);
            rest.into_iter().for_each(|p| out.include(p));
            out
        }
    }
}

fn factor(factors: &[&dyn TexelFactor], x: usize, y: usize, screen: Option<[f32; 2]>) -> f32 {
    factors.iter().fold(1.0, |f, t| f * t.factor(x as u32, y as u32, screen).clamp(0.0, 1.0))
}

fn build_up(state: &mut TileState, i: usize, v: f32, ceiling: f32) -> bool {
    build_up_at(&mut state.acc, i, v, ceiling)
}

/// Builds the stroke coverage at `i` up by `v` towards `ceiling`; true if it changed.
#[inline]
fn build_up_at(acc: &mut [f32], i: usize, v: f32, ceiling: f32) -> bool {
    let Some(c) = acc.get_mut(i) else { return false };
    let v = v.min(1.0);
    if ceiling <= *c || v.is_nan() || v <= 0.0 {
        return false;
    }
    let next = *c + v * (ceiling - *c);
    if next > *c {
        *c = next;
        true
    } else {
        false
    }
}

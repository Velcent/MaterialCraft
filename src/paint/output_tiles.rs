//! [`TileMaps`]: a texture set's final maps over some of its tiles, with exactly the values [`MaterialMaps`] has
//! there, for updating what a brush stroke changed without building the whole maps again.
//!
//! Each tile is composited as the whole build composites it (the same rectangles, so the same values, whatever the
//! stack holds). Which tiles a change reaches ([`MaterialMaps::tiles_reached`]): the changed tiles grown by how far
//! the stack spreads a change of the changed layers ([`Compositor::reach`]: filters reading them), and, as the
//! normal derived from the height reads one texel around each texel, their neighbours, whose heights are composited
//! for the tiles asked for. Baked normal and ambient occlusion maps of another resolution are
//! resampled whole, as the whole build does, once (the stack cache keeps them).

use super::channel::Channel;
use super::effect::EffectRegistry;
use super::height_normal::Slope;
use super::image::{Rect, TILE_SIZE};
use super::normal;
use super::normal_combine;
use super::output::{MaterialMaps, OutputOptions, resample};
use super::par::for_each_mut;
use super::stack::{CacheSession, Compositor, FillContext};
use super::texture_set::{BakedKind, TextureSet};
use crate::paint::TiledImage;
use crate::tex::Tex;
use std::collections::HashMap;
use std::sync::Arc;

/// One tile of a texture set's final maps, row by row over `rect` (see [`MaterialMaps`] for each map).
#[derive(Debug, Clone, PartialEq)]
pub struct TileMaps {
    pub rect: Rect,
    pub base_color: Vec<[f32; 4]>,
    pub roughness: Vec<[f32; 4]>,
    pub metallic: Vec<[f32; 4]>,
    pub normal: Vec<[f32; 4]>,
    pub height: Vec<[f32; 4]>,
    pub ambient_occlusion: Vec<[f32; 4]>,
    pub emissive: Vec<[f32; 4]>,
    pub opacity: Vec<[f32; 4]>,
}

impl TileMaps {
    /// The map for a standard channel; `None` for user channels.
    pub fn map(&self, channel: Channel) -> Option<&[[f32; 4]]> {
        Some(match channel {
            Channel::BaseColor => &self.base_color,
            Channel::Roughness => &self.roughness,
            Channel::Metallic => &self.metallic,
            Channel::Normal => &self.normal,
            Channel::Height => &self.height,
            Channel::AmbientOcclusion => &self.ambient_occlusion,
            Channel::Emissive => &self.emissive,
            Channel::Opacity => &self.opacity,
            Channel::User(_) => return None,
        })
    }
}

impl MaterialMaps {
    /// The tiles whose maps change when the images of the layers `layers` (ids) change in `changed` (tile
    /// coordinates of the set's grid), sorted, and the area where the composite may change; or why the set has to
    /// be built whole.
    pub fn tiles_reached(
        set: &TextureSet,
        options: &OutputOptions,
        effects: &EffectRegistry,
        fills: &FillContext,
        layers: &[&str],
        changed: &[(u32, u32)],
    ) -> Result<Reached, WholeReason> {
        let reach = Compositor::with_effects(set, effects).with_fills(fills).reach(layers).ok_or(WholeReason::ReadsEverything)?;
        let grid = grid(set);
        let (w, h) = set.resolution;
        let area: Vec<Rect> = changed.iter().filter(|t| inside(**t, grid)).filter_map(|t| tile_rect(set, *t)).map(|r| grow(r, reach, (w, h))).collect();
        let mut tiles: Vec<(u32, u32)> = area
            .iter()
            .flat_map(|r| {
                let (x0, y0, x1, y1) = (r.x / TILE_SIZE, r.y / TILE_SIZE, (r.x + r.w).saturating_sub(1) / TILE_SIZE, (r.y + r.h).saturating_sub(1) / TILE_SIZE);
                (y0..=y1).flat_map(move |y| (x0..=x1).map(move |x| (x, y)))
            })
            .collect();
        if derives_normal(set, options) {
            tiles = with_ring(&tiles, grid);
        }
        tiles.sort_unstable();
        tiles.dedup();
        let total = (grid.0 * grid.1) as usize;
        // most of a big set: building it whole is as quick (small sets go tile by tile, it costs little either way)
        if total > 16 && tiles.len() * 4 > total * 3 {
            return Err(WholeReason::TooFar);
        }
        Ok(Reached { tiles, area })
    }

    /// The maps over `tiles` (see the module docs), with expensive masks from and into `cache`; an error if the
    /// set's maps can't be built tile by tile. Tiles outside the set's grid are left out. Compositing warnings are
    /// not collected (the whole build reports them).
    pub fn build_tiles(
        set: &TextureSet,
        options: &OutputOptions,
        effects: &EffectRegistry,
        fills: &FillContext,
        cache: Option<&CacheSession>,
        tiles: &[(u32, u32)],
    ) -> Result<Vec<TileMaps>, WholeReason> {
        let compositor = Compositor::with_effects(set, effects).with_fills(fills).with_cache(cache);
        let (w, h) = (set.resolution.0 as usize, set.resolution.1 as usize);
        let baked_normal = Baked::of(set, &BakedKind::Normal, cache);
        let baked_ao = Baked::of(set, &BakedKind::AmbientOcclusion, cache);
        let grid = grid(set);
        let mut wanted: Vec<(u32, u32)> = tiles.iter().copied().filter(|t| inside(*t, grid)).collect();
        wanted.sort_unstable();
        wanted.dedup();
        let builder = Builder { set, compositor, baked_normal, baked_ao, slope: None, heights: HashMap::new() };
        let builder = if derives_normal(set, options) && w * h > 0 {
            let slope = Slope::new(options.height_filter, options.height_normal_strength, (w, h));
            let heights = builder.heights(&with_ring(&wanted, grid));
            Builder { slope: Some(slope), heights, ..builder }
        } else {
            builder
        };
        let mut out: Vec<Option<TileMaps>> = vec![None; wanted.len()];
        for_each_mut(&mut out, |i, slot| *slot = wanted.get(i).and_then(|t| builder.tile(*t)));
        Ok(out.into_iter().flatten().collect())
    }
}

/// A tile's area and its texels, row by row.
type TilePixels = (Rect, Vec<[f32; 4]>);
/// Composited heights by tile.
type Heights = HashMap<(u32, u32), TilePixels>;

/// What a change reaches.
#[derive(Debug, Clone, PartialEq)]
pub struct Reached {
    /// The tiles to build again.
    pub tiles: Vec<(u32, u32)>,
    /// Where the composite may differ (kept results elsewhere stay valid).
    pub area: Vec<Rect>,
}

/// Why a texture set's maps are built whole instead of tile by tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WholeReason {
    /// An effect reads the whole image, or a changed layer is not in the set.
    ReadsEverything,
    /// The change reaches most of the set.
    TooFar,
}

/// `r` grown by `by` on every side, clipped to `size`.
fn grow(r: Rect, by: u32, size: (u32, u32)) -> Rect {
    let (x, y) = (r.x.saturating_sub(by), r.y.saturating_sub(by));
    let (x1, y1) = ((r.x + r.w).saturating_add(by).min(size.0), (r.y + r.h).saturating_add(by).min(size.1));
    Rect { x, y, w: x1.saturating_sub(x), h: y1.saturating_sub(y) }
}

/// What every tile of one [`MaterialMaps::build_tiles`] reads.
struct Builder<'a> {
    set: &'a TextureSet,
    compositor: Compositor<'a>,
    baked_normal: Option<Baked<'a>>,
    baked_ao: Option<Baked<'a>>,
    /// The height → normal conversion, when the set derives its normal from its height.
    slope: Option<Slope>,
    /// The composited height of the tiles asked for and their neighbours (alpha 1), by tile.
    heights: Heights,
}

impl Builder<'_> {
    /// One channel over `r` as the whole build has it: composited with alpha 1, or the default where the set lacks
    /// the channel.
    fn channel(&self, c: Channel, r: Rect) -> Vec<[f32; 4]> {
        if !self.set.has_channel(c) {
            return vec![c.default_value(); r.area()];
        }
        let mut px = vec![[0.0f32; 4]; r.area()];
        self.compositor.composite_rect(c, r, &mut px);
        px.iter_mut().for_each(|p| p[3] = 1.0);
        px
    }

    /// The composited height of `tiles`, in parallel.
    fn heights(&self, tiles: &[(u32, u32)]) -> Heights {
        let mut out: Vec<Option<((u32, u32), TilePixels)>> = vec![None; tiles.len()];
        for_each_mut(&mut out, |i, slot| {
            *slot = tiles.get(i).and_then(|&t| tile_rect(self.set, t).map(|r| (t, (r, self.channel(Channel::Height, r)))));
        });
        out.into_iter().flatten().collect()
    }

    /// The composited height at texel `(x, y)` of the set (0 outside the tiles composited).
    fn height_at(&self, x: usize, y: usize) -> f32 {
        let t = TILE_SIZE as usize;
        let Some((r, px)) = self.heights.get(&((x / t) as u32, (y / t) as u32)) else { return 0.0 };
        px.get((y - r.y as usize) * r.w as usize + (x - r.x as usize)).map_or(0.0, |p| p[0])
    }

    fn tile(&self, t: (u32, u32)) -> Option<TileMaps> {
        let r = tile_rect(self.set, t)?;
        let base_color = self.channel(Channel::BaseColor, r);
        let roughness = self.channel(Channel::Roughness, r);
        let metallic = self.channel(Channel::Metallic, r);
        let height = match self.heights.get(&t) {
            Some((_, px)) => px.clone(),
            None => self.channel(Channel::Height, r),
        };
        let mut normal = self.channel(Channel::Normal, r);
        let mut ambient_occlusion = self.channel(Channel::AmbientOcclusion, r);
        let emissive = self.channel(Channel::Emissive, r);
        let opacity = self.channel(Channel::Opacity, r);

        let baked_normal = self.baked_normal.as_ref().map(|b| b.read(r));
        if self.slope.is_some() || baked_normal.is_some() {
            for (i, p) in normal.iter_mut().enumerate() {
                let (x, y) = (r.x as usize + i % r.w as usize, r.y as usize + i / r.w as usize);
                let derived = self.slope.map(|s| s.normal_at(x, y, |xx, yy| self.height_at(xx, yy)));
                let baked = baked_normal.as_ref().and_then(|b| b.get(i)).map_or(normal::FLAT, |&c| normal::decode(c));
                let derived = derived.map_or(normal::FLAT, normal::decode);
                *p = normal::encode(normal_combine::combine(baked, derived, normal::decode(*p)));
            }
        } else {
            normal.iter_mut().for_each(|p| *p = normal::encode(normal::decode(*p)));
        }
        if let Some(ao) = self.baked_ao.as_ref().map(|b| b.read(r)) {
            for (i, p) in ambient_occlusion.iter_mut().enumerate() {
                let k = ao.get(i).map_or(1.0, |a| a[0]);
                *p = [p[0] * k, p[1] * k, p[2] * k, 1.0];
            }
        }
        Some(TileMaps { rect: r, base_color, roughness, metallic, normal, height, ambient_occlusion, emissive, opacity })
    }
}

/// True when the set's normal map is derived from its height (as [`MaterialMaps::build_with`] does).
fn derives_normal(set: &TextureSet, options: &OutputOptions) -> bool {
    set.has_channel(Channel::Height) && options.height_normal_strength != 0.0
}

/// A baked map as the whole build reads it: at the set's resolution, or resampled to it (whole, once: kept in the
/// stack cache when there is one).
enum Baked<'a> {
    Same(&'a TiledImage),
    Resampled(Arc<Tex>),
}

impl<'a> Baked<'a> {
    fn of(set: &'a TextureSet, kind: &BakedKind, cache: Option<&CacheSession>) -> Option<Baked<'a>> {
        let img = set.baked(kind)?;
        if img.size() == set.resolution {
            return Some(Baked::Same(img));
        }
        let (w, h) = (set.resolution.0 as usize, set.resolution.1 as usize);
        let compute = || Arc::new(resample(&img.to_tex(), w, h));
        Some(Baked::Resampled(match cache {
            Some(c) => c.resampled(kind.id(), compute),
            None => compute(),
        }))
    }

    /// Its texels over `r`, row by row.
    fn read(&self, r: Rect) -> Vec<[f32; 4]> {
        match self {
            Baked::Same(img) => {
                let mut px = vec![[0.0f32; 4]; r.area()];
                img.read_region(r, &mut px);
                px
            }
            Baked::Resampled(t) => (r.y as usize..(r.y + r.h) as usize)
                .flat_map(|y| (r.x as usize..(r.x + r.w) as usize).map(move |x| (x, y)))
                .map(|(x, y)| t.px.get(y * t.w + x).copied().unwrap_or([0.0; 4]))
                .collect(),
        }
    }
}

/// Tiles per row and column of the set.
fn grid(set: &TextureSet) -> (u32, u32) {
    (set.resolution.0.div_ceil(TILE_SIZE), set.resolution.1.div_ceil(TILE_SIZE))
}

fn inside(t: (u32, u32), grid: (u32, u32)) -> bool {
    t.0 < grid.0 && t.1 < grid.1
}

/// `tiles` and their neighbours inside the grid, sorted, without repeats.
fn with_ring(tiles: &[(u32, u32)], grid: (u32, u32)) -> Vec<(u32, u32)> {
    let mut out = Vec::with_capacity(tiles.len() * 9);
    for &(tx, ty) in tiles {
        for ny in ty.saturating_sub(1)..=(ty + 1).min(grid.1.saturating_sub(1)) {
            for nx in tx.saturating_sub(1)..=(tx + 1).min(grid.0.saturating_sub(1)) {
                out.push((nx, ny));
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// The pixel area of tile `t` of the set, clipped to it.
fn tile_rect(set: &TextureSet, t: (u32, u32)) -> Option<Rect> {
    let (w, h) = set.resolution;
    let (x, y) = (t.0.checked_mul(TILE_SIZE)?, t.1.checked_mul(TILE_SIZE)?);
    (x < w && y < h).then(|| Rect { x, y, w: (w - x).min(TILE_SIZE), h: (h - y).min(TILE_SIZE) })
}

#[cfg(test)]
#[path = "output_tiles_tests.rs"]
mod tests;

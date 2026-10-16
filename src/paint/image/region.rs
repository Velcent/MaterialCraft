//! Bulk pixel access to a [`TiledImage`] by region and by tile, including the parallel tile update.

use super::{tile_rect, Rect, TileBuf, TiledImage, T};
use crate::paint::par::for_each_mut;
use crate::paint::pixel::PixelFormat;

impl TiledImage {
    /// Decodes `r` row by row into `out` (`r.w * r.h` pixels); pixels outside the image read as
    /// zero samples. If `out` is shorter, only the rows that fit are written.
    pub fn read_region(&self, r: Rect, out: &mut [[f32; 4]]) {
        let zero = self.zero_pixel();
        let bpp = self.format.bytes_per_pixel();
        for (row, line) in out
            .chunks_exact_mut(r.w as usize)
            .take(r.h as usize)
            .enumerate()
        {
            let y = r.y as usize + row;
            if y >= self.height as usize {
                line.fill(zero);
                continue;
            }
            for (col, p) in line.iter_mut().enumerate() {
                let x = r.x as usize + col;
                *p = if x >= self.width as usize {
                    zero
                } else {
                    match &self.tiles[(y / T) * self.tiles_x() as usize + x / T] {
                        None => zero,
                        Some(t) => {
                            let o = ((y % T) * T + x % T) * bpp;
                            self.format.decode(&t[o..o + bpp])
                        }
                    }
                };
            }
        }
    }

    /// Decodes tile (tx, ty) into `out` (its [`Rect::area`] pixels). Returns its rect, or `None`
    /// outside the tile grid.
    pub fn read_tile(&self, tx: u32, ty: u32, out: &mut [[f32; 4]]) -> Option<Rect> {
        let r = self.tile_rect(tx, ty)?;
        self.read_region(r, out);
        Some(r)
    }

    /// Encodes `pixels` (row by row, the tile rect's size) into tile (tx, ty) and marks it dirty.
    pub fn write_tile(&mut self, tx: u32, ty: u32, pixels: &[[f32; 4]]) -> bool {
        let (Some(i), Some(r)) = (self.tile_index(tx, ty), self.tile_rect(tx, ty)) else {
            return false;
        };
        let fmt = self.format;
        let t = self.tiles[i]
            .get_or_insert_with(|| vec![0u8; T * T * fmt.bytes_per_pixel()].into_boxed_slice());
        encode_tile(fmt, r, pixels, t);
        self.dirty[i] = true;
        true
    }

    /// Rewrites the listed tiles in parallel: `f` gets the tile rect and its current pixels, and
    /// edits them in place. Tiles outside the grid and repeated entries are ignored. Marks them dirty.
    pub fn update_tiles<F: Fn(Rect, &mut [[f32; 4]]) + Sync>(
        &mut self,
        tiles: &[(u32, u32)],
        f: F,
    ) {
        let (w, h, fmt) = (self.width, self.height, self.format);
        let mut wanted = vec![false; self.tiles.len()];
        for &(tx, ty) in tiles {
            if let Some(i) = self.tile_index(tx, ty) {
                wanted[i] = true;
            }
        }
        let tiles_x = self.tiles_x();
        let mut jobs: Vec<(u32, &mut TileBuf)> = self
            .tiles
            .iter_mut()
            .enumerate()
            .filter(|(i, _)| wanted[*i])
            .map(|(i, t)| (i as u32, t))
            .collect();
        for_each_mut(&mut jobs, |_, (i, tile)| {
            let Some(r) = tile_rect(w, h, *i % tiles_x, *i / tiles_x) else {
                return;
            };
            let mut px = vec![[0.0f32; 4]; r.area()];
            let buf = tile
                .get_or_insert_with(|| vec![0u8; T * T * fmt.bytes_per_pixel()].into_boxed_slice());
            decode_tile(fmt, r, buf, &mut px);
            f(r, &mut px);
            encode_tile(fmt, r, &px, buf);
        });
        for (d, wnt) in self.dirty.iter_mut().zip(wanted) {
            *d |= wnt;
        }
    }

    /// Sets every pixel to `p` (allocating all tiles).
    pub fn fill(&mut self, p: [f32; 4]) {
        let all = self.all_tiles();
        self.update_tiles(&all, |_, px| px.fill(p));
    }

    /// Frees every tile (all pixels read as zero again) and marks them dirty.
    pub fn clear(&mut self) {
        self.tiles.iter_mut().for_each(|t| *t = None);
        self.dirty.iter_mut().for_each(|d| *d = true);
    }
}

pub(super) fn decode_tile(fmt: PixelFormat, r: Rect, buf: &[u8], out: &mut [[f32; 4]]) {
    let bpp = fmt.bytes_per_pixel();
    for (row, line) in out
        .chunks_exact_mut(r.w as usize)
        .take(r.h as usize)
        .enumerate()
    {
        for (col, p) in line.iter_mut().enumerate() {
            let o = (row * T + col) * bpp;
            *p = fmt.decode(&buf[o..o + bpp]);
        }
    }
}

pub(super) fn encode_tile(fmt: PixelFormat, r: Rect, px: &[[f32; 4]], buf: &mut [u8]) {
    let bpp = fmt.bytes_per_pixel();
    for (row, line) in px.chunks_exact(r.w as usize).take(r.h as usize).enumerate() {
        for (col, p) in line.iter().enumerate() {
            let o = (row * T + col) * bpp;
            fmt.encode(*p, &mut buf[o..o + bpp]);
        }
    }
}

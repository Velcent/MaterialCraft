//! [`TiledImage`]: channel storage in lazily allocated 128×128 tiles with dirty tracking.
//!
//! Tiles hold raw samples in the image's [`PixelFormat`] (little-endian). A tile that was never
//! written is not allocated and reads as all-zero samples, so an empty paint layer costs nothing.
//! Pixel access is normalised RGBA (see [`crate::paint::pixel`]).

use super::pixel::PixelFormat;

use std::fmt;

mod convert;
mod region;
mod swap;
mod tile;

use region::{decode_tile, encode_tile};
pub use tile::Tile;

/// Edge length of a tile in pixels.
pub const TILE_SIZE: u32 = 128;
/// Largest accepted width or height.
pub const MAX_DIMENSION: u32 = 65536;

const T: usize = TILE_SIZE as usize;

/// A rectangle of pixels; for a tile, its area clipped to the image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub fn area(self) -> usize {
        self.w as usize * self.h as usize
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageError {
    /// Width or height is above [`MAX_DIMENSION`].
    TooLarge { width: u64, height: u64 },
    /// Raw data or texture does not match the size and format.
    WrongDataSize { expected: usize, found: usize },
    /// Tile (tx, ty) is outside the tile grid.
    NoSuchTile { tx: u32, ty: u32 },
}

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ImageError::TooLarge { width, height } => {
                write!(f, "image {width}x{height} is larger than {MAX_DIMENSION}")
            }
            ImageError::WrongDataSize { expected, found } => {
                write!(f, "expected {expected} bytes of pixel data, found {found}")
            }
            ImageError::NoSuchTile { tx, ty } => write!(f, "tile ({tx}, {ty}) is outside the image"),
        }
    }
}

impl std::error::Error for ImageError {}

type TileBuf = Option<Tile>;

/// A width × height image stored in tiles. See the module docs. Tiles are shared copy-on-write ([`Tile`]):
/// a clone costs one pointer per tile until either copy is written.
#[derive(Clone)]
pub struct TiledImage {
    width: u32,
    height: u32,
    format: PixelFormat,
    tiles: Vec<TileBuf>,
    dirty: Vec<bool>,
}

impl TiledImage {
    /// An empty image (every pixel reads as zero samples). No memory is used until pixels are written.
    pub fn new(width: u32, height: u32, format: PixelFormat) -> Result<TiledImage, ImageError> {
        if width > MAX_DIMENSION || height > MAX_DIMENSION {
            return Err(ImageError::TooLarge { width: width as u64, height: height as u64 });
        }
        let n = (width.div_ceil(TILE_SIZE) * height.div_ceil(TILE_SIZE)) as usize;
        Ok(TiledImage { width, height, format, tiles: vec![None; n], dirty: vec![false; n] })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn format(&self) -> PixelFormat {
        self.format
    }

    /// Tiles per row and per column.
    pub fn tiles_x(&self) -> u32 {
        self.width.div_ceil(TILE_SIZE)
    }

    pub fn tiles_y(&self) -> u32 {
        self.height.div_ceil(TILE_SIZE)
    }

    /// Pixel area of tile (tx, ty), clipped to the image; `None` outside the tile grid.
    pub fn tile_rect(&self, tx: u32, ty: u32) -> Option<Rect> {
        tile_rect(self.width, self.height, tx, ty)
    }

    /// Every tile coordinate, row by row.
    pub fn all_tiles(&self) -> Vec<(u32, u32)> {
        let tx = self.tiles_x();
        (0..self.tiles.len() as u32).map(|i| (i % tx, i / tx)).collect()
    }

    fn tile_index(&self, tx: u32, ty: u32) -> Option<usize> {
        (tx < self.tiles_x() && ty < self.tiles_y()).then(|| (ty * self.tiles_x() + tx) as usize)
    }

    fn zero_pixel(&self) -> [f32; 4] {
        self.format.decode(&[0u8; 16])
    }

    /// Normalised RGBA at (x, y). Pixels outside the image read like unallocated ones.
    pub fn get(&self, x: u32, y: u32) -> [f32; 4] {
        if x >= self.width || y >= self.height {
            return self.zero_pixel();
        }
        let i = ((y / TILE_SIZE) * self.tiles_x() + x / TILE_SIZE) as usize;
        match &self.tiles[i] {
            None => self.zero_pixel(),
            Some(t) => {
                let bpp = self.format.bytes_per_pixel();
                let o = ((y as usize % T) * T + x as usize % T) * bpp;
                self.format.decode(&t.bytes()[o..o + bpp])
            }
        }
    }

    /// Writes one pixel, allocating its tile and marking it dirty. Returns false outside the image.
    pub fn set(&mut self, x: u32, y: u32, p: [f32; 4]) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        let bpp = self.format.bytes_per_pixel();
        let i = ((y / TILE_SIZE) * self.tiles_x() + x / TILE_SIZE) as usize;
        let t = self.tiles[i].get_or_insert_with(|| Tile::zeroed(T * T * bpp)).make_mut();
        let o = ((y as usize % T) * T + x as usize % T) * bpp;
        self.format.encode(p, &mut t[o..o + bpp]);
        self.dirty[i] = true;
        true
    }

    /// True if tile (tx, ty) holds pixel data.
    pub fn tile_allocated(&self, tx: u32, ty: u32) -> bool {
        self.tile_index(tx, ty).is_some_and(|i| self.tiles[i].is_some())
    }

    /// True if any tile overlapping `r` holds pixel data.
    pub fn region_allocated(&self, r: Rect) -> bool {
        let (x1, y1) = ((r.x + r.w).min(self.width), (r.y + r.h).min(self.height));
        if r.x >= x1 || r.y >= y1 {
            return false;
        }
        (r.y / TILE_SIZE..=(y1 - 1) / TILE_SIZE).any(|ty| (r.x / TILE_SIZE..=(x1 - 1) / TILE_SIZE).any(|tx| self.tile_allocated(tx, ty)))
    }

    /// Tiles written since the last [`Self::clear_dirty`], row by row.
    pub fn dirty_tiles(&self) -> Vec<(u32, u32)> {
        let tx = self.tiles_x();
        self.dirty.iter().enumerate().filter(|(_, d)| **d).map(|(i, _)| (i as u32 % tx, i as u32 / tx)).collect()
    }

    pub fn clear_dirty(&mut self) {
        self.dirty.iter_mut().for_each(|d| *d = false);
    }

    pub fn mark_dirty(&mut self, tx: u32, ty: u32) {
        if let Some(i) = self.tile_index(tx, ty) {
            self.dirty[i] = true;
        }
    }

    pub fn mark_all_dirty(&mut self) {
        self.dirty.iter_mut().for_each(|d| *d = true);
    }

    /// Number of tiles in the grid.
    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }

    /// Number of tiles holding pixel data.
    pub fn allocated_tiles(&self) -> usize {
        self.tiles.iter().filter(|t| t.is_some()).count()
    }

    /// Bytes used by pixel data.
    pub fn memory_bytes(&self) -> usize {
        self.allocated_tiles() * T * T * self.format.bytes_per_pixel()
    }
}

fn tile_rect(width: u32, height: u32, tx: u32, ty: u32) -> Option<Rect> {
    let (x, y) = (tx.checked_mul(TILE_SIZE)?, ty.checked_mul(TILE_SIZE)?);
    (x < width && y < height).then(|| Rect { x, y, w: (width - x).min(TILE_SIZE), h: (height - y).min(TILE_SIZE) })
}

impl fmt::Debug for TiledImage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TiledImage({}x{} {}, {}/{} tiles)", self.width, self.height, self.format.id(), self.allocated_tiles(), self.tile_count())
    }
}

/// Equal when size, format and pixel bytes match (an unallocated tile equals an all-zero one).
/// Dirty flags are ignored.
impl PartialEq for TiledImage {
    fn eq(&self, other: &TiledImage) -> bool {
        self.size() == other.size() && self.format == other.format && self.to_raw() == other.to_raw()
    }
}

#[cfg(test)]
mod tests;

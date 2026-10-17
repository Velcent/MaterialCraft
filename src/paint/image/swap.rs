//! Whole-tile access to a [`TiledImage`] for undo: reading the shared [`Tile`]s, putting old ones back and
//! finding which tiles an edit replaced.

use super::{ImageError, T, Tile, TiledImage};

impl TiledImage {
    /// The shared tile at (tx, ty); `None` if it is unallocated or outside the grid.
    pub fn tile(&self, tx: u32, ty: u32) -> Option<&Tile> {
        self.tiles.get(self.tile_index(tx, ty)?)?.as_ref()
    }

    /// Every allocated tile with its coordinates, row by row.
    pub fn tiles(&self) -> impl Iterator<Item = ((u32, u32), &Tile)> {
        let tx = self.tiles_x().max(1);
        self.tiles.iter().enumerate().filter_map(move |(i, t)| t.as_ref().map(|t| ((i as u32 % tx, i as u32 / tx), t)))
    }

    /// Puts `tile` (or, with `None`, an unallocated tile) at (tx, ty), marks it dirty and returns what was there.
    /// The tile must come from an image of the same pixel format.
    pub fn replace_tile(&mut self, tx: u32, ty: u32, tile: Option<Tile>) -> Result<Option<Tile>, ImageError> {
        let i = self.tile_index(tx, ty).ok_or(ImageError::NoSuchTile { tx, ty })?;
        let expected = T * T * self.format.bytes_per_pixel();
        if let Some(t) = &tile
            && t.len() != expected
        {
            return Err(ImageError::WrongDataSize { expected, found: t.len() });
        }
        let slot = self.tiles.get_mut(i).ok_or(ImageError::NoSuchTile { tx, ty })?;
        let old = std::mem::replace(slot, tile);
        if let Some(d) = self.dirty.get_mut(i) {
            *d = true;
        }
        Ok(old)
    }

    /// Tiles that are not the same allocation as in `before` (written, freed or allocated since `before` was
    /// cloned from this image), row by row. Compares allocations, not bytes, so it costs one pointer
    /// comparison per tile. Every tile differs if the size or format differs.
    pub fn changed_tiles(&self, before: &TiledImage) -> Vec<(u32, u32)> {
        let same_shape = self.size() == before.size() && self.format == before.format;
        let tx = self.tiles_x().max(1);
        let differs = |(a, b): (&Option<Tile>, Option<&Option<Tile>>)| match (a, b) {
            (None, Some(None)) => false,
            (Some(a), Some(Some(b))) => !a.same(b),
            _ => true,
        };
        (0..self.tiles.len())
            .filter(|&i| !same_shape || self.tiles.get(i).is_some_and(|a| differs((a, before.tiles.get(i)))))
            .map(|i| (i as u32 % tx, i as u32 / tx))
            .collect()
    }
}

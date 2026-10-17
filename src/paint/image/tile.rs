//! [`Tile`]: one shared, copy-on-write tile of a [`super::TiledImage`].
//!
//! Tiles sit behind an [`Arc`], so cloning an image, keeping an old tile for undo or duplicating a layer
//! shares pixel data; the first write to a shared tile copies it (the design of PhotoCraft's `Arc<Tile>`
//! surfaces, written here for 128² tiles in any [`crate::paint::PixelFormat`]).

use std::fmt;
use std::sync::Arc;

/// The raw samples of one tile (always `TILE_SIZE²` pixels of its image's format, row by row). Cheap to
/// clone; the bytes are shared until one owner writes.
#[derive(Clone, PartialEq, Eq)]
pub struct Tile(pub(super) Arc<[u8]>);

impl Tile {
    pub(super) fn zeroed(len: usize) -> Tile {
        Tile(Arc::from(vec![0u8; len]))
    }

    pub(super) fn from_vec(bytes: Vec<u8>) -> Tile {
        Tile(Arc::from(bytes))
    }

    /// The samples for writing; copies them first if another owner shares them.
    pub(super) fn make_mut(&mut self) -> &mut [u8] {
        Arc::make_mut(&mut self.0)
    }

    /// The raw samples.
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }

    /// Size of the pixel data in bytes.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Always false: a tile holds a whole tile's samples.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Identity of the shared allocation: equal for clones of the same tile, distinct for any two live
    /// allocations. Lets memory accounting count a tile shared by several owners once.
    pub fn id(&self) -> usize {
        Arc::as_ptr(&self.0) as *const u8 as usize
    }

    /// True if both are the same allocation (not merely equal bytes).
    pub fn same(&self, other: &Tile) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for Tile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Tile({:#x}, {} bytes)", self.id(), self.len())
    }
}

//! [`GeometryMask`]: a second mask of a layer that limits it to parts of the geometry, by name or UV tile, so
//! it survives a mesh re-import. Data only for now: the compositor does not apply it yet and reports that.

#[derive(Debug, Clone, PartialEq)]
pub enum GeometryMask {
    /// The layer applies only on these mesh parts (by [`crate::mesh::MeshPart::name`]).
    Parts(Vec<String>),
    /// The layer applies only on these UV tiles (UDIM numbers, 1001 = the first tile).
    UvTiles(Vec<u32>),
}

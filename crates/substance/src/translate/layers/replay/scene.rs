//! [`SetScene`]: one texture set's mesh as the brush pipeline needs it to replay strokes: the set's parts as one
//! mesh, its view scene in Painter's world (no fit: the stored cameras look at the mesh as it is), its surface
//! map and texel index at the set's resolution.

use super::super::decal::geometry::SetGeometry;
use materialcraft::mesh::{Fit, Mesh, MeshPart, SurfaceKey, SurfaceMap};
use materialcraft::projection::ViewScene;
use materialcraft::stamp::TexelIndex;
use materialcraft_brush::ReplayScene;

/// Gutter around the islands that strokes paint into (as the surface map extends the surface there).
const GUTTER: u32 = 8;

pub struct SetScene {
    /// The whole mesh's bounding sphere (model-sized brushes).
    pub model: Option<materialcraft::mesh::Sphere>,
    mesh: Mesh,
    view: ViewScene,
    surface: SurfaceMap,
    index: TexelIndex,
}

impl SetScene {
    /// `None` when the set has no parts or no usable UVs.
    pub fn build(geometry: &SetGeometry, resolution: (u32, u32)) -> Option<SetScene> {
        let parts: Vec<MeshPart> = geometry
            .parts
            .iter()
            .map(|p| MeshPart {
                material: 0,
                ..p.part.clone()
            })
            .collect();
        if parts.is_empty() {
            return None;
        }
        let mesh = Mesh {
            materials: vec!["set".into()],
            parts,
        };
        let key = SurfaceKey {
            material: 0,
            uv_set: 0,
            width: resolution.0,
            height: resolution.1,
            gutter: GUTTER,
            mesh_revision: 0,
        };
        let surface = SurfaceMap::build(&mesh, key).ok()?;
        let index = TexelIndex::build(&surface);
        let view = ViewScene::new(&mesh, Fit::IDENTITY);
        Some(SetScene {
            model: geometry.scene,
            mesh,
            view,
            surface,
            index,
        })
    }

    /// The scene a replay paints through.
    pub fn replay(&self) -> ReplayScene<'_> {
        ReplayScene {
            mesh: &self.mesh,
            view: &self.view,
            surface: &self.surface,
            index: &self.index,
            factors: &[],
        }
    }

    /// The images' size.
    pub fn size(&self) -> (u32, u32) {
        (self.surface.width() as u32, self.surface.height() as u32)
    }
}

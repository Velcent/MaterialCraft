//! [`Inputs`]: what one case (one texture set of one project) gives the subjects: the project mesh with the set's
//! slots, the high mesh, and a bake computed once on first use and shared by every baked-map subject.

mod finder;
mod high;

pub use finder::MeshFinder;
pub use high::{BoundsCheck, HighCache, HighMesh, HighSource, MAX_HIGH_POLY_BYTES, bounds};

use crate::settings::Params;
use crate::subject::CaseError;
use materialcraft::bake::{self, BakedMaps, CancelToken, TextureSetRef};
use materialcraft::diag::NullSink;
use materialcraft::mesh::Mesh;
use std::cell::OnceCell;
use std::rc::Rc;

pub struct Inputs {
    /// The project's mesh (Painter's normalised space).
    pub mesh: Rc<Mesh>,
    /// The texture set's material slots in it.
    pub set: TextureSetRef,
    /// Shared with the project's other sets that bake from the same mesh.
    pub high: Rc<HighMesh>,
    /// Our baker's maps; computed with the first caller's params (one case has one set of params).
    baked: OnceCell<Result<BakedMaps, CaseError>>,
}

impl Inputs {
    pub fn new(mesh: Rc<Mesh>, set: TextureSetRef, high: Rc<HighMesh>) -> Inputs {
        Inputs {
            mesh,
            set,
            high,
            baked: OnceCell::new(),
        }
    }

    /// Our bake of this case (every map at once, as Painter's bake of a set works).
    pub fn bake(&self, params: &Params) -> Result<&BakedMaps, CaseError> {
        self.baked
            .get_or_init(|| {
                bake::bake_set(
                    &self.mesh,
                    &self.set,
                    &self.high.scene,
                    &params.bake,
                    &CancelToken::new(),
                    &NullSink,
                )
                .map_err(|e| CaseError::failed(e.to_string()))
            })
            .as_ref()
            .map_err(Clone::clone)
    }

    /// True once the bake has run (successfully or not).
    pub fn baked(&self) -> bool {
        self.baked.get().is_some()
    }
}

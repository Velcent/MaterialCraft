//! [`Inputs`]: what one case (one texture set of one project) gives the subjects: the set's low mesh, the
//! high mesh, and a bake computed once on first use and shared by every baked-map subject.

mod finder;
mod high;
mod low;

pub use finder::MeshFinder;
pub use high::{BoundsCheck, HighCache, HighMesh, HighSource, MAX_HIGH_POLY_BYTES};
pub use low::{bounds, to_bake_mesh};

use crate::settings::Params;
use crate::subject::CaseError;
use materialcraft::bake;
use materialcraft::tex::Tex;
use std::cell::OnceCell;
use std::collections::BTreeMap;
use std::rc::Rc;

pub struct Inputs {
    /// The texture set's triangles (all parts on its material slots).
    pub low: bake::Mesh,
    /// Shared with the project's other sets that bake from the same mesh.
    pub high: Rc<HighMesh>,
    /// Our baker's maps by name; computed with the first caller's params (one case has one set of params).
    baked: OnceCell<Result<BTreeMap<String, Tex>, CaseError>>,
}

impl Inputs {
    pub fn new(low: bake::Mesh, high: Rc<HighMesh>) -> Inputs {
        Inputs {
            low,
            high,
            baked: OnceCell::new(),
        }
    }

    /// Our bake of this case (all maps at once, as the baker works).
    pub fn bake(&self, params: &Params) -> Result<&BTreeMap<String, Tex>, CaseError> {
        self.baked
            .get_or_init(|| bake::bake(&self.low, &self.high.mesh, &params.bake).map_err(CaseError::failed))
            .as_ref()
            .map_err(Clone::clone)
    }

    /// True once the bake has run (successfully or not).
    pub fn baked(&self) -> bool {
        self.baked.get().is_some()
    }
}

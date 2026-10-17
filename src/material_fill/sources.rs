//! [`MaterialFills`]: material fills ([`FillSource::Material`]) as a [`FillSources`] implementation for the
//! compositor's fill hook. A material is resolved and parsed once per instance, rendered once per key in the
//! shared [`MaterialCache`] (at the texture set's resolution) and sampled at the projection's taps with the
//! fill's filter and repetition.
//!
//! A reference that does not resolve, a graph that does not parse or render and an unknown parameter make the
//! material not handled: the compositor leaves the fill out and reports it. A channel the material has no
//! output for gets no coverage (the fill leaves it as it is).

use super::cache::MaterialCache;
use super::material::{Material, MaterialError};
use super::reference::MaterialResolver;
use super::render::{MAX_RENDER_SIZE, RenderedMaterial};
use crate::paint::effect::EvalTarget;
use crate::paint::texture_set::TextureSet;
use crate::paint::working_space::SpaceConverter;
use crate::paint::{Compositor, FillRequest, FillSource, FillSources};
use crate::projection::Tap;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

type MaterialId = (String, Vec<(String, String)>);

/// Evaluates material fills from `.mca` graphs.
pub struct MaterialFills {
    resolver: Arc<dyn MaterialResolver>,
    cache: Arc<MaterialCache>,
    loaded: Mutex<HashMap<MaterialId, Result<Arc<Material>, MaterialError>>>,
}

impl MaterialFills {
    /// Resolves references with `resolver`; renders go to `cache` (share one cache between runs so a material is
    /// rendered once).
    pub fn new(resolver: Arc<dyn MaterialResolver>, cache: Arc<MaterialCache>) -> MaterialFills {
        MaterialFills { resolver, cache, loaded: Mutex::new(HashMap::new()) }
    }

    pub fn cache(&self) -> &Arc<MaterialCache> {
        &self.cache
    }

    /// The material `reference` with `params` applied (resolved and parsed on the first call).
    pub fn material(&self, reference: &str, params: &[(String, String)]) -> Result<Arc<Material>, MaterialError> {
        let id = (reference.to_string(), params.to_vec());
        let mut loaded = self.loaded.lock().unwrap_or_else(|e| e.into_inner());
        loaded.entry(id).or_insert_with(|| Material::load(self.resolver.as_ref(), reference, params).map(Arc::new)).clone()
    }

    /// The material rendered at `size` × `size`. Also the entry point for tools that measure a material against
    /// a reference result (e.g. how well a tray material replaces an imported one).
    pub fn render(&self, reference: &str, params: &[(String, String)], size: usize) -> Result<Arc<RenderedMaterial>, MaterialError> {
        let material = self.material(reference, params)?;
        self.cache.get_or_render(&material, size)
    }

    /// The render size for `set`: its larger side, at most [`MAX_RENDER_SIZE`].
    pub fn render_size(set: &TextureSet) -> usize {
        (set.resolution.0.max(set.resolution.1) as usize).clamp(1, MAX_RENDER_SIZE)
    }
}

impl FillSources for MaterialFills {
    fn handles(&self, source: &FillSource) -> bool {
        match source {
            FillSource::Material { path, params } => self.material(path, params).is_ok(),
            _ => false,
        }
    }

    fn sample(&self, request: &FillRequest, taps: &[Tap], out: &mut [[f32; 4]]) -> bool {
        let (FillSource::Material { path, params }, EvalTarget::Channel(channel)) = (request.source, request.target) else {
            return false;
        };
        let Ok(rendered) = self.render(path, params, MaterialFills::render_size(request.set)) else {
            return false;
        };
        let Some(texture) = rendered.channel(channel) else {
            out.fill([0.0; 4]);
            return true;
        };
        let levels = texture.levels();
        let space = SpaceConverter::new(Compositor::new(request.set).working_space(channel));
        for (tap, o) in taps.iter().zip(out.iter_mut()) {
            *o = match levels.sample(tap.uv, tap.footprint, request.filter, request.repeat) {
                Some(v) => space.to_working(v),
                None => [0.0; 4],
            };
        }
        true
    }
}

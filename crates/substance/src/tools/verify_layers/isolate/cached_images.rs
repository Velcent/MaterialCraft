//! [`CachedImages`]: an [`ImageSource`] that decodes each bitmap once. The isolation imports the same set many
//! times (once per slot and per probe), and every import would otherwise decode its bitmaps again.

use crate::translate::layers::ImageSource;
use crate::translate::layers::images::LoadedImage;
use materialcraft::paint::TiledImage;
use std::collections::HashMap;
use std::sync::Mutex;

/// A decoded bitmap (image, float format) or why it could not be read.
type Loaded = Result<(TiledImage, bool), String>;

pub struct CachedImages<'a> {
    inner: &'a dyn ImageSource,
    loaded: Mutex<HashMap<String, Loaded>>,
}

impl<'a> CachedImages<'a> {
    pub fn new(inner: &'a dyn ImageSource) -> Self {
        CachedImages {
            inner,
            loaded: Mutex::new(HashMap::new()),
        }
    }
}

impl ImageSource for CachedImages<'_> {
    fn load(&self, url: &str) -> Result<LoadedImage, String> {
        if let Some(hit) = self.loaded.lock().ok().and_then(|l| l.get(url).cloned()) {
            return hit.map(|(image, float)| LoadedImage { image, float });
        }
        let result = self.inner.load(url).map(|l| (l.image, l.float));
        if let Ok(mut l) = self.loaded.lock() {
            l.insert(url.to_string(), result.clone());
        }
        result.map(|(image, float)| LoadedImage { image, float })
    }
}

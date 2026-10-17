//! [`CachedImages`]: an [`ImageSource`] that decodes each bitmap once. The isolation imports the same set many
//! times (once per slot and per probe), and every import would otherwise decode its bitmaps again.

use crate::translate::layers::ImageSource;
use crate::translate::layers::images::LoadedImage;
use materialcraft::paint::TiledImage;
use std::cell::RefCell;
use std::collections::HashMap;

/// A decoded bitmap (image, float format) or why it could not be read.
type Loaded = Result<(TiledImage, bool), String>;

pub struct CachedImages<'a> {
    inner: &'a dyn ImageSource,
    loaded: RefCell<HashMap<String, Loaded>>,
}

impl<'a> CachedImages<'a> {
    pub fn new(inner: &'a dyn ImageSource) -> Self {
        CachedImages {
            inner,
            loaded: RefCell::new(HashMap::new()),
        }
    }
}

impl ImageSource for CachedImages<'_> {
    fn load(&self, url: &str) -> Result<LoadedImage, String> {
        if let Some(hit) = self.loaded.borrow().get(url) {
            return hit.clone().map(|(image, float)| LoadedImage { image, float });
        }
        let result = self.inner.load(url).map(|l| (l.image, l.float));
        self.loaded.borrow_mut().insert(url.to_string(), result.clone());
        result.map(|(image, float)| LoadedImage { image, float })
    }
}

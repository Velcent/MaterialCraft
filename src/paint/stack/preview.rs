//! One layer on its own, for thumbnails and the isolated-mask view: its content for a channel (base with
//! content effects, without mask, opacity or blend) and its mask's grey values.

use super::Compositor;
use super::pass::Pass;
use crate::paint::channel::Channel;
use crate::paint::image::Rect;
use crate::paint::layer::Layer;
use crate::paint::working_space::SpaceConverter;

impl Compositor<'_> {
    /// The content of `layer` (any layer, it need not be in the set) for `channel` over `r` into `out`
    /// (`r.area()` pixels): stored encoding, alpha = coverage. A group's content is its children composited in
    /// isolation.
    pub fn layer_content_rect(&self, layer: &Layer, channel: Channel, r: Rect, out: &mut [[f32; 4]]) {
        let pass = Pass { channel, r, space: SpaceConverter::new(self.working_space(channel)) };
        let content = self.content(layer, layer.effects.len(), &pass, 0);
        for (o, c) in out.iter_mut().zip(content) {
            *o = c;
        }
        pass.space.to_stored_all(out);
    }

    /// The grey value of `layer`'s mask over `r` into `out` (`r.area()` values), whether or not the mask is
    /// switched on; `None` if the layer has no mask.
    pub fn mask_rect(&self, layer: &Layer, r: Rect, out: &mut [f32]) -> Option<()> {
        let mask = layer.mask.as_ref()?;
        let grey = self.mask_values(layer, mask, mask.effects.len(), r);
        for (o, g) in out.iter_mut().zip(grey) {
            *o = g;
        }
        Some(())
    }
}

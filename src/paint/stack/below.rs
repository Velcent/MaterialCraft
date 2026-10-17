//! [`LayerStack`]: the [`StackContext`] a mask generator gets: the height its layer writes and the composited
//! height of everything below that layer, over any rectangle.
//!
//! "Below" follows the stack order of the whole texture set up to the layer. Layers in folders that contain
//! the layer are composited straight into the result (as a pass-through folder would); a folder that
//! composites in isolation is therefore approximated there. A layer that is not in the set (a preview) sees
//! the whole stack below it.

use super::Compositor;
use super::pass::{Pass, Scratch};
use crate::paint::channel::Channel;
use crate::paint::effect::StackContext;
use crate::paint::image::Rect;
use crate::paint::layer::{Layer, LayerKind};
use crate::paint::working_space::SpaceConverter;

/// One layer of the compositor's texture set and the stack around it.
#[derive(Debug)]
pub(super) struct LayerStack<'c, 'a> {
    pub compositor: &'c Compositor<'a>,
    pub layer: &'c Layer,
}

impl LayerStack<'_, '_> {
    fn pass(&self, r: Rect) -> Pass {
        Pass { channel: Channel::Height, r, space: SpaceConverter::new(self.compositor.working_space(Channel::Height)) }
    }
}

impl StackContext for LayerStack<'_, '_> {
    fn layer_height(&self, rect: Rect) -> Vec<[f32; 2]> {
        let pass = self.pass(rect);
        let mut content = self.compositor.content(self.layer, self.layer.effects.len(), &pass, 0);
        pass.space.to_stored_all(&mut content);
        content.into_iter().map(|c| [c[0], c[3]]).collect()
    }

    fn height_below(&self, rect: Rect) -> Vec<f32> {
        let pass = self.pass(rect);
        let mut acc = vec![pass.empty(); rect.area()];
        let mut scratch = Scratch::default();
        self.compositor.apply_until(&self.compositor.set.layers, self.layer, &pass, &mut acc, 0, &mut scratch);
        pass.space.to_stored_all(&mut acc);
        acc.into_iter().map(|c| c[0]).collect()
    }
}

impl Compositor<'_> {
    /// Applies `layers` in order until `stop`; true if `stop` was reached.
    fn apply_until(&self, layers: &[Layer], stop: &Layer, pass: &Pass, acc: &mut [[f32; 4]], depth: usize, s: &mut Scratch) -> bool {
        for l in layers {
            if std::ptr::eq(l, stop) {
                return true;
            }
            if let LayerKind::Group(children) = &l.kind
                && contains(children, stop)
            {
                return self.apply_until(children, stop, pass, acc, depth + 1, s);
            }
            self.apply_layer(l, pass, acc, depth, s);
        }
        false
    }
}

/// True if `stop` is one of `layers` or inside one of their folders.
fn contains(layers: &[Layer], stop: &Layer) -> bool {
    layers.iter().any(|l| {
        std::ptr::eq(l, stop)
            || match &l.kind {
                LayerKind::Group(children) => contains(children, stop),
                _ => false,
            }
    })
}

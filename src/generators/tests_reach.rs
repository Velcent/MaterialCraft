//! What the generators read of the layer stack: those that say `None` never touch it, and the height comparison
//! reads the heights at the texel itself.

use super::*;
use crate::paint::effect::{EvalRequest, EvalTarget, Reach, StackContext};
use crate::paint::image::Rect;
use crate::paint::{Channel, TextureSet};
use std::sync::atomic::{AtomicBool, Ordering};

const N: u32 = 16;

/// Heights per texel of an N x N set; notes whether it was read.
#[derive(Debug)]
struct Heights {
    own: Vec<[f32; 2]>,
    below: Vec<f32>,
    read: AtomicBool,
}

impl Heights {
    fn new(f: impl Fn(u32) -> f32) -> Heights {
        let own = (0..N * N).map(|i| [f(i), if i % 3 == 0 { 0.0 } else { 1.0 }]).collect();
        let below = (0..N * N).map(|i| 0.9 - 0.5 * f(i)).collect();
        Heights { own, below, read: AtomicBool::new(false) }
    }

    fn rect<T: Copy>(&self, v: &[T], r: Rect) -> Vec<T> {
        self.read.store(true, Ordering::Relaxed);
        (0..r.area()).map(|k| v[((r.y + k as u32 / r.w) * N + r.x + k as u32 % r.w) as usize]).collect()
    }
}

impl StackContext for Heights {
    fn layer_height(&self, rect: Rect) -> Vec<[f32; 2]> {
        self.rect(&self.own, rect)
    }

    fn height_below(&self, rect: Rect) -> Vec<f32> {
        self.rect(&self.below, rect)
    }
}

fn run(id: &str, set: &TextureSet, stack: &Heights, rect: Rect) -> (Option<Reach>, Vec<[f32; 4]>) {
    let g = builtin().generator(id).unwrap();
    let params = vec![("offset".to_string(), "0.1".to_string()), ("contrast".to_string(), "0.8".to_string())];
    let request = EvalRequest { set, target: EvalTarget::Mask, rect, params: &params, stack: Some(stack), surface: None, frame: None };
    let mut out = vec![[0.0; 4]; rect.area()];
    let _ = g.generate(&request, &mut out);
    (g.stack_reach(&request), out)
}

#[test]
fn generators_that_read_no_stack_never_touch_it() {
    let set = TextureSet::new("reach", "S", (N, N)).with_channel(Channel::Roughness);
    let ids = GENERATORS.iter().map(|g| g.id).chain(crate::proc_textures::TEXTURES.iter().map(|t| t.id));
    for id in ids {
        let stack = Heights::new(|i| (i % 5) as f32 / 4.0);
        let (reach, _) = run(id, &set, &stack, Rect { x: 2, y: 3, w: 5, h: 4 });
        match reach {
            None => assert!(!stack.read.load(Ordering::Relaxed), "{id} reads the stack"),
            Some(r) => assert_eq!((id, r), (HEIGHT_COMBINATION, Reach::Texel)),
        }
    }
}

#[test]
fn height_combination_reads_the_heights_at_its_texel() {
    let set = TextureSet::new("reach", "S", (N, N)).with_channel(Channel::Roughness);
    let base = |i: u32| ((i * 37) % 11) as f32 / 10.0;
    let c = 5 * N + 6;
    let whole = Rect { x: 0, y: 0, w: N, h: N };
    let (_, before) = run(HEIGHT_COMBINATION, &set, &Heights::new(base), whole);
    for k in (0..N * N).filter(|k| *k != c) {
        let stack = Heights::new(|i| if i == k { 1.0 - base(i) } else { base(i) });
        let (_, after) = run(HEIGHT_COMBINATION, &set, &stack, whole);
        assert_eq!(before[c as usize], after[c as usize], "changing {k}");
        let (_, one) = run(HEIGHT_COMBINATION, &set, &stack, Rect { x: c % N, y: c / N, w: 1, h: 1 });
        assert_eq!(one[0], before[c as usize]);
    }
}

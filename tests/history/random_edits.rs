//! Random structural and pixel edits on a small paint document, for the history gate.

use materialcraft::history::{CoalesceKey, PaintHistory};
use materialcraft::paint::edit::{Edit, ImageRef, LayerProperty, LayerRef, LayerSlot, StackEffect, StackId, TileCapture};
use materialcraft::paint::effect::{EffectKind, Procedural};
use materialcraft::paint::height_normal::HeightFilter;
use materialcraft::paint::{BlendMode, Channel, ContentEffect, Document, Layer, LayerKind, Mask, PaintContent, Rect, TextureSet, TiledImage};

/// xorshift64*: a deterministic generator, so a failure reproduces from its seed.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.max(1))
    }

    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { (self.next() % n as u64) as usize }
    }

    pub fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }

    fn unit(&mut self) -> f32 {
        self.below(17) as f32 / 16.0
    }

    fn pixel(&mut self) -> [f32; 4] {
        [self.unit(), self.unit(), self.unit(), self.unit()]
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        items.get(self.below(items.len()))
    }
}

pub const CHANNELS: [Channel; 2] = [Channel::BaseColor, Channel::Roughness];

fn blank(res: (u32, u32), channel: Channel) -> TiledImage {
    TiledImage::new(res.0, res.1, channel.default_format()).unwrap()
}

fn paint_layer(id: &str, res: (u32, u32)) -> Layer {
    let mut content = PaintContent::default();
    let mut layer = Layer::new(id, id, LayerKind::Paint(PaintContent::default()));
    for c in CHANNELS {
        content.set_image(c, blank(res, c));
        layer.set_blend(c, BlendMode::Normal, 1.0);
    }
    layer.kind = LayerKind::Paint(content);
    layer
}

pub fn texture_set(id: &str, res: (u32, u32)) -> TextureSet {
    let mut set = TextureSet::new(id, id, res);
    for c in CHANNELS {
        set = set.with_channel(c);
    }
    set
}

/// A document with one texture set of `res` and two paint layers.
pub fn document(res: (u32, u32)) -> Document {
    let mut set = texture_set("set0", res);
    set.layers = vec![paint_layer("l0", res), paint_layer("l1", res)];
    let mut doc = Document::default();
    doc.texture_sets.push(set);
    doc
}

/// Nearest-neighbour copy of every image of `set` at `res` (a resolution change).
fn resized(set: &TextureSet, res: (u32, u32)) -> TextureSet {
    let mut out = set.clone();
    out.resolution = res;
    let ids: Vec<String> = set.walk_layers().map(|(_, l)| l.id.clone()).collect();
    for id in ids {
        let (Some(src), Some(dst)) = (set.layer(&id), out.layer_mut(&id)) else { continue };
        for (which, img) in src.images() {
            let mut new = TiledImage::new(res.0, res.1, img.format()).unwrap();
            for y in 0..res.1 {
                for x in 0..res.0 {
                    let p = img.get(x * img.width() / res.0, y * img.height() / res.1);
                    if p != [0.0; 4] {
                        new.set(x, y, p);
                    }
                }
            }
            *dst.image_mut(which).unwrap() = new;
        }
    }
    out
}

/// One random operation on the document and its history.
pub enum Op {
    Edit {
        label: &'static str,
        key: Option<CoalesceKey>,
        edits: Vec<Edit>,
    },
    /// Dabs painted straight into an image, each recorded through a [`TileCapture`] under one stroke key.
    Stroke {
        image: ImageRef,
        dabs: Vec<(u32, u32, [f32; 4])>,
        key: CoalesceKey,
    },
    Undo,
    Redo,
    Jump(usize),
}

/// Makes random operations; ids and coalesce keys never repeat.
pub struct Generator {
    pub rng: Rng,
    next_id: u64,
}

impl Generator {
    pub fn new(seed: u64) -> Generator {
        Generator { rng: Rng::new(seed), next_id: 0 }
    }

    fn id(&mut self, prefix: &str) -> String {
        self.next_id += 1;
        format!("{prefix}{}", self.next_id)
    }

    fn key(&mut self) -> CoalesceKey {
        self.next_id += 1;
        CoalesceKey(self.next_id)
    }

    /// An undo, redo or jump with probability `history_percent`, an edit otherwise.
    pub fn op(&mut self, doc: &Document, history_len: usize, history_percent: u64) -> Op {
        if self.rng.chance(history_percent) {
            return match self.rng.below(5) {
                0..=2 => Op::Undo,
                3 => Op::Redo,
                _ => Op::Jump(self.rng.below(history_len + 1)),
            };
        }
        loop {
            if let Some(op) = self.edit(doc) {
                return op;
            }
        }
    }

    fn edit(&mut self, doc: &Document) -> Option<Op> {
        let set = self.rng.pick(&doc.texture_sets)?.clone();
        let layers: Vec<String> = set.walk_layers().map(|(_, l)| l.id.clone()).collect();
        let groups: Vec<String> = set.walk_layers().filter(|(_, l)| matches!(l.kind, LayerKind::Group(_))).map(|(_, l)| l.id.clone()).collect();
        let images: Vec<ImageRef> =
            set.walk_layers().flat_map(|(_, l)| l.images().into_iter().map(|(w, _)| ImageRef::new(LayerRef::new(&set.id, &l.id), w))).collect();
        let layer = self.rng.pick(&layers).map(|l| LayerRef::new(&set.id, l));
        let res = set.resolution;
        let single = |label, edit| Some(Op::Edit { label, key: None, edits: vec![edit] });
        match self.rng.below(100) {
            0..=9 => {
                let slot = self.slot(&set, &groups);
                let mut new = paint_layer(&self.id("p"), res);
                if self.rng.chance(30) {
                    new = Layer::new(self.id("g"), "group", LayerKind::Group(vec![new]));
                }
                single("Add layer", Edit::InsertLayer { set: set.id.clone(), at: slot, layer: Box::new(new) })
            }
            10..=14 if layers.len() > 2 => single("Delete layer", Edit::RemoveLayer(layer?)),
            15..=21 => {
                let to = self.slot(&set, &groups);
                single("Move layer", Edit::MoveLayer { layer: layer?, to })
            }
            22..=29 => {
                // A slider drag: several values under one key.
                let key = self.key();
                let layer = layer?;
                let edits =
                    (0..1 + self.rng.below(6)).map(|_| Edit::SetLayer { layer: layer.clone(), property: LayerProperty::Opacity(self.rng.unit()) }).collect();
                Some(Op::Edit { label: "Opacity", key: Some(key), edits })
            }
            30..=33 => {
                let property = match self.rng.below(3) {
                    0 => LayerProperty::Visible(self.rng.chance(50)),
                    1 => LayerProperty::Name(self.id("name")),
                    _ => LayerProperty::Blend(vec![]),
                };
                single("Layer property", Edit::SetLayer { layer: layer?, property })
            }
            34..=39 => {
                let mask = match self.rng.below(3) {
                    0 => None,
                    1 => Some(Mask::new(self.rng.unit())),
                    _ => {
                        let mut img = blank(res, Channel::Roughness);
                        for _ in 0..20 {
                            img.set(self.rng.below(res.0 as usize) as u32, self.rng.below(res.1 as usize) as u32, self.rng.pixel());
                        }
                        Some(Mask::from_image(img))
                    }
                };
                single("Mask", Edit::SetLayer { layer: layer?, property: LayerProperty::Mask(mask) })
            }
            40..=47 => self.effect_edit(&set, layer?),
            48..=69 => {
                let image = self.rng.pick(&images)?.clone();
                let (w, h) = (1 + self.rng.below(150) as u32, 1 + self.rng.below(150) as u32);
                let (w, h) = (w.min(res.0), h.min(res.1));
                let rect = Rect { x: self.rng.below((res.0 - w + 1) as usize) as u32, y: self.rng.below((res.1 - h + 1) as usize) as u32, w, h };
                let p = self.rng.pixel();
                single("Fill region", Edit::WritePixels { image, rect, pixels: vec![p; rect.area()] })
            }
            70..=89 => {
                let image = self.rng.pick(&images)?.clone();
                let mut dabs = Vec::new();
                let (mut x, mut y) = (self.rng.below(res.0 as usize) as u32, self.rng.below(res.1 as usize) as u32);
                for _ in 0..2 + self.rng.below(12) {
                    x = (x + self.rng.below(40) as u32).saturating_sub(20).min(res.0 - 1);
                    y = (y + self.rng.below(40) as u32).saturating_sub(20).min(res.1 - 1);
                    dabs.push((x, y, self.rng.pixel()));
                }
                Some(Op::Stroke { image, dabs, key: self.key() })
            }
            90..=92 => {
                let new_res = if res.0 > 200 { (res.0 / 2, res.1 / 2) } else { (res.0 * 2, res.1 * 2) };
                single("Resolution", Edit::ReplaceTextureSet(Box::new(resized(&set, new_res))))
            }
            93..=94 => {
                let mut mesh = doc.mesh.clone();
                if mesh.materials.is_empty() {
                    mesh.materials.push(self.id("material"));
                } else {
                    mesh.materials.clear();
                }
                single("Re-import mesh", Edit::SetMesh(Box::new(mesh)))
            }
            95..=96 => {
                let filter = if doc.height_filter == HeightFilter::Sobel { HeightFilter::Backward } else { HeightFilter::Sobel };
                single("Height filter", Edit::SetHeightFilter(filter))
            }
            97 => {
                let mut new = texture_set(&self.id("set"), (130, 130));
                new.layers.push(paint_layer(&self.id("p"), (130, 130)));
                let index = self.rng.below(doc.texture_sets.len() + 1);
                single("Add texture set", Edit::InsertTextureSet { index, set: Box::new(new) })
            }
            98 if doc.texture_sets.len() > 1 => single("Delete texture set", Edit::RemoveTextureSet { set: set.id.clone() }),
            _ => None,
        }
    }

    fn slot(&mut self, set: &TextureSet, groups: &[String]) -> LayerSlot {
        let parent = if self.rng.chance(40) { self.rng.pick(groups).cloned() } else { None };
        let len = match &parent {
            Some(g) => set.layer(g).map_or(0, |l| l.children().len()),
            None => set.layers.len(),
        };
        // Sometimes one past the end, which a move can reject.
        LayerSlot { parent, index: self.rng.below(len + 2) }
    }

    fn effect_edit(&mut self, set: &TextureSet, layer: LayerRef) -> Option<Op> {
        let current = set.layer(&layer.layer)?;
        let n = current.effects.len();
        let edit = match self.rng.below(4) {
            0 | 1 => {
                let effect = if self.rng.chance(50) {
                    ContentEffect::new(Vec::new(), EffectKind::Generator(Procedural::new(self.id("gen"))))
                } else {
                    let mut px = PaintContent::default();
                    let mut img = blank(set.resolution, Channel::Roughness);
                    img.set(self.rng.below(set.resolution.0 as usize) as u32, 0, self.rng.pixel());
                    px.set_image(Channel::Roughness, img);
                    ContentEffect::new(Vec::new(), EffectKind::Paint(px)).with_blend(Channel::Roughness, BlendMode::Multiply, self.rng.unit())
                };
                Edit::InsertEffect { layer, index: self.rng.below(n + 1), effect: StackEffect::Content(effect) }
            }
            2 if n > 0 => Edit::RemoveEffect { layer, stack: StackId::Content, index: self.rng.below(n) },
            3 if n > 1 => Edit::MoveEffect { layer, stack: StackId::Content, from: self.rng.below(n), to: self.rng.below(n) },
            _ if n > 0 => {
                let index = self.rng.below(n);
                let mut effect = current.effects.get(index)?.clone();
                effect.enabled = !effect.enabled;
                Edit::ReplaceEffect { layer, index, effect: StackEffect::Content(effect) }
            }
            _ => return None,
        };
        Some(Op::Edit { label: "Effect", key: None, edits: vec![edit] })
    }
}

/// Runs `op`. Returns false if an edit was rejected (nothing changed, nothing recorded).
pub fn run(op: Op, doc: &mut Document, history: &mut PaintHistory) -> bool {
    match op {
        Op::Edit { label, key, edits } => {
            let mut ok = true;
            for edit in edits {
                ok &= history.apply(doc, label, key, edit).is_ok();
            }
            history.close();
            ok
        }
        Op::Stroke { image, dabs, key } => {
            for (x, y, p) in dabs {
                let capture = TileCapture::begin(doc, image.clone()).unwrap();
                let set = doc.texture_set_mut(&image.layer.set).unwrap();
                let target = set.layer_mut(&image.layer.layer).unwrap().image_mut(image.image).unwrap();
                for dy in 0..6 {
                    for dx in 0..6 {
                        target.set(x + dx, y + dy, p);
                    }
                }
                history.record(doc, "Brush stroke", Some(key), capture.finish(doc).unwrap());
            }
            history.close();
            true
        }
        Op::Undo => history.undo(doc).map(|_| true).unwrap(),
        Op::Redo => history.redo(doc).map(|_| true).unwrap(),
        Op::Jump(n) => history.jump_to(doc, n).map(|_| true).unwrap(),
    }
}

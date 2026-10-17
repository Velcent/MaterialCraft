//! The preview object smart mask thumbnails are drawn on: a ball covered in rounded bricks, with the baked maps a
//! mask generator reads (ambient occlusion and curvature of the tiles' relief, world-space normal and position of
//! the ball) made without a bake: the relief comes from a small graph of the designer side's nodes, the ball's
//! normal and position from its UV layout (`render3d`'s sphere: u runs half way round, v from pole to pole).
//!
//! A mask is composited on the preview set through the generators ([`materialcraft::generators::builtin`]) and
//! shown as an accent colour over grey on the lit ball.

use materialcraft::generators;
use materialcraft::graph::Graph;
use materialcraft::paint::{BakedKind, BakedMap, BlendMode, Channel, Compositor, Fill, Layer, LayerKind, Mask, PixelFormat, TextureSet, TiledImage};
use materialcraft::render3d::{self, Maps, View};
use materialcraft::tex::Tex;
use std::f32::consts::PI;

/// Side of the preview set's maps, in texels.
pub const PREVIEW_PX: usize = 192;

/// The relief: rounded bricks with a little noise; `ao` and `curvature` nodes read it.
const RELIEF: &str = "size 256
node 1 bricks rows=4 cols=2 mortar=0.05 bevel=0.07 lum_var=0.1 seed=4
node 2 blur in=@1 radius=2
node 3 fbm scale=6 octaves=4 seed=11
node 4 blend fg=@3 bg=@2 mode=5 opacity=0.2
node 5 ao in=@4 radius=10 intensity=4
node 6 curvature in=@4 mode=1 intensity=8 radius=4
output height 4
output ambient_occlusion 5
output curvature 6
";

/// The preview object's maps.
pub struct Preview {
    pub height: Tex,
    pub ao: Tex,
    set: TextureSet,
}

impl Preview {
    pub fn new() -> Result<Preview, String> {
        let graph = Graph::parse(RELIEF).map_err(|e| e.to_string())?;
        let mut out = graph.evaluate(Some(PREVIEW_PX)).map_err(|e| e.to_string())?;
        let mut take = |name: &str| out.remove(name).ok_or_else(|| format!("the preview relief has no {name} output"));
        let (height, ao, curvature) = (take("height")?, take("ambient_occlusion")?, take("curvature")?);
        let n = PREVIEW_PX as u32;
        let mut set = TextureSet::new("preview", "Preview", (n, n)).with_channel(Channel::Roughness);
        set.baked.push(map(BakedKind::AmbientOcclusion, |i| grey(&ao, i))?);
        set.baked.push(map(BakedKind::Curvature, |i| grey(&curvature, i))?);
        set.baked.push(map(BakedKind::Thickness, |i| grey(&ao, i))?);
        set.baked.push(map(BakedKind::WorldSpaceNormal, |i| ball_point(i).map(|c| c * 0.5 + 0.5))?);
        set.baked.push(map(BakedKind::Position, |i| ball_point(i).map(|c| c * 0.5 + 0.5))?);
        Ok(Preview { height, ao, set })
    }

    /// The value of `mask` over the preview set, row by row.
    pub fn mask_values(&self, mask: &Mask) -> Result<Vec<f32>, String> {
        let r = Channel::Roughness;
        let mut set = self.set.clone();
        let below = Layer::new("below", "Below", LayerKind::Fill(Fill::uniform(vec![(r, [0.0, 0.0, 0.0, 1.0])]))).with_blend(r, BlendMode::Normal, 1.0);
        let mut top = Layer::new("mask", "Mask", LayerKind::Fill(Fill::uniform(vec![(r, [1.0; 4])]))).with_blend(r, BlendMode::Normal, 1.0);
        top.mask = Some(mask.clone());
        set.layers = vec![below, top];
        let (tex, warnings) = Compositor::with_effects(&set, generators::builtin()).composite_tex(r);
        if let Some(w) = warnings.first() {
            return Err(format!("the mask can't be evaluated: {w:?}"));
        }
        Ok(tex.px.iter().map(|p| p[0].clamp(0.0, 1.0)).collect())
    }

    /// `values` (a mask over the preview set) as `accent` over grey on the lit ball, `side` pixels square.
    pub fn ball(&self, values: &[f32], accent: [f32; 3], side: usize) -> Tex {
        let s = PREVIEW_PX;
        let at = |i: usize| values.get(i).copied().unwrap_or(0.0);
        let grey = [0.16, 0.17, 0.19];
        let base = Tex { w: s, h: s, px: (0..s * s).map(|i| [0, 1, 2].map(|c| lerp(pick(grey, c), pick(accent, c), at(i))).with_alpha()).collect() };
        let rough = Tex { w: s, h: s, px: (0..s * s).map(|i| [lerp(0.55, 0.4, at(i)); 3].with_alpha()).collect() };
        let (flat, zero) = (Tex::solid(s, s, [0.5, 0.5, 1.0, 1.0]), Tex::solid(s, s, [0.0, 0.0, 0.0, 1.0]));
        let maps = Maps { base: &base, normal: &flat, rough: &rough, metal: &zero, ao: &self.ao, emissive: None, height: Some(&self.height) };
        let view = View { tess: 48, relief: render3d::RELIEF_BUMP, ..View::default() };
        render3d::render(&maps, &view, side, side)
    }
}

trait WithAlpha {
    fn with_alpha(self) -> [f32; 4];
}

impl WithAlpha for [f32; 3] {
    fn with_alpha(self) -> [f32; 4] {
        [self[0], self[1], self[2], 1.0]
    }
}

fn pick(c: [f32; 3], i: usize) -> f32 {
    c.get(i).copied().unwrap_or(0.0)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn grey(t: &Tex, i: usize) -> [f32; 3] {
    let v = t.px.get(i).map_or(0.5, |p| p[0]);
    [v; 3]
}

/// The point of the unit ball at texel `i` of the preview maps (also its outward normal), Y up.
fn ball_point(i: usize) -> [f32; 3] {
    let n = PREVIEW_PX;
    let (u, v) = (((i % n) as f32 + 0.5) / n as f32, ((i / n) as f32 + 0.5) / n as f32);
    let (lon, theta) = (u * PI - PI, v * PI);
    [theta.sin() * lon.sin(), theta.cos(), theta.sin() * lon.cos()]
}

/// A baked map of the preview set from `f(texel index)` (0..1 per component).
fn map(kind: BakedKind, f: impl Fn(usize) -> [f32; 3]) -> Result<BakedMap, String> {
    let n = PREVIEW_PX as u32;
    let format = PixelFormat::from_id("rgb16").ok_or("no rgb16 pixel format")?;
    let mut image = TiledImage::new(n, n, format).map_err(|e| format!("{e:?}"))?;
    for y in 0..n {
        for x in 0..n {
            let c = f((y * n + x) as usize);
            image.set(x, y, c.with_alpha());
        }
    }
    Ok(BakedMap { kind, image })
}

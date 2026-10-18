//! Filter reach: over random parameters, inputs, centres and tilings, a change to one input texel just outside a
//! filter's declared [`Reach`] never changes the output at the centre (and for per-texel filters, a change at
//! any other texel never does).

use super::tests::{N, apply, params, touching};
use super::*;
use crate::mesh::SurfaceMap;
use crate::paint::effect::{EvalRequest, EvalTarget, Reach};
use crate::paint::image::Rect;
use crate::paint::{Channel, TextureSet};

/// A small deterministic generator (xorshift64*), so failures repeat.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /// Uniform in lo..hi.
    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Uniform in 0..n.
    fn below(&mut self, n: u32) -> u32 {
        (self.next() % n.max(1) as u64) as u32
    }

    fn text(&mut self, lo: f32, hi: f32) -> String {
        self.f(lo, hi).to_string()
    }
}

/// Random parameters for filter `id` whose reach stays well inside the N x N test image.
fn random_params(id: &str, g: &mut Rng) -> Vec<(String, String)> {
    let pairs: Vec<(&str, String)> = match id {
        BLUR => vec![("radius", g.text(0.0, 0.07))],
        SHARPEN => vec![("radius", g.text(0.0, 0.04)), ("amount", g.text(0.0, 5.0))],
        BEVEL => vec![("distance", g.text(-0.12, 0.12)), ("smoothing", g.text(0.0, 0.025)), ("round", g.below(2).to_string())],
        MASK_OUTLINE => vec![("width", g.text(0.0, 0.15)), ("position", g.below(3).to_string()), ("curve", g.text(-1.0, 1.0)), ("blur", g.text(0.0, 0.025))],
        WARP => vec![
            ("intensity", g.text(0.0, 0.15)),
            ("scale", g.text(0.1, 6.0)),
            ("seed", g.below(1000).to_string()),
            ("directional", g.below(2).to_string()),
            ("angle", g.text(0.0, 1.0)),
        ],
        SLOPE_BLUR => vec![
            ("intensity", g.text(0.0, 0.15)),
            ("scale", g.text(0.1, 6.0)),
            ("seed", g.below(1000).to_string()),
            ("mode", g.below(3).to_string()),
            ("samples", (2 + g.below(40)).to_string()),
        ],
        HSL => vec![("hue", g.text(-1.0, 1.0)), ("saturation", g.text(-1.0, 1.0)), ("lightness", g.text(-1.0, 1.0))],
        HEIGHT_ADJUST => vec![("multiplier", g.text(-3.0, 3.0)), ("offset", g.text(-1.0, 1.0)), ("invert", g.below(2).to_string())],
        CLAMP => vec![("min", g.text(0.0, 0.5)), ("max", g.text(0.5, 1.0))],
        HISTOGRAM_SCAN => vec![("position", g.text(0.0, 1.0)), ("contrast", g.text(0.0, 1.0))],
        HISTOGRAM_SHIFT => vec![("position", g.text(0.0, 1.0))],
        CONTRAST_LUMINOSITY => vec![("contrast", g.text(-1.0, 1.0)), ("luminosity", g.text(-1.0, 1.0))],
        FINISH_ROUGH => vec![("intensity", g.text(0.0, 1.0)), ("grunge", (1 + g.below(15)).to_string()), ("seed", g.below(1000).to_string())],
        _ => vec![("invert", g.below(2).to_string())],
    };
    pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

/// A random input: noise, or a few soft discs (so edge filters see shapes with long distances to their edges).
fn random_image(g: &mut Rng) -> Vec<[f32; 4]> {
    if g.below(2) == 0 {
        return (0..N * N).map(|_| [g.f(0.0, 1.0), g.f(0.0, 1.0), g.f(0.0, 1.0), g.f(0.5, 1.0)]).collect();
    }
    let discs: Vec<(f32, f32, f32)> = (0..1 + g.below(4)).map(|_| (g.f(0.0, N as f32), g.f(0.0, N as f32), g.f(3.0, 24.0))).collect();
    (0..N * N)
        .map(|i| {
            let (x, y) = ((i % N) as f32 + 0.5, (i / N) as f32 + 0.5);
            let inside = discs.iter().any(|(cx, cy, r)| (x - cx).hypot(y - cy) < *r);
            let v = if inside { 0.75 + 0.2 * (x * 0.37).sin() } else { 0.2 + 0.1 * (y * 0.29).cos() };
            [v, v * 0.8, v * 0.6, 1.0]
        })
        .collect()
}

fn reach_of(id: &str, p: &[(String, String)], target: EvalTarget, size: (u32, u32)) -> Reach {
    let set = TextureSet::new("s", "S", size).with_channel(Channel::Roughness);
    let request = EvalRequest { set: &set, target, rect: Rect { x: 0, y: 0, w: size.0, h: size.1 }, params: p, stack: None, surface: None, frame: None };
    filter(id).unwrap().reach(&request)
}

/// A texel at Chebyshev distance exactly `d` from `(cx, cy)` inside the image, if there is one.
fn on_ring(g: &mut Rng, cx: u32, cy: u32, d: u32) -> Option<(u32, u32)> {
    for _ in 0..64 {
        let along = g.below(2 * d + 1) as i64 - d as i64;
        let side = if g.below(2) == 0 { -(d as i64) } else { d as i64 };
        let (dx, dy) = if g.below(2) == 0 { (side, along) } else { (along, side) };
        let (x, y) = (cx as i64 + dx, cy as i64 + dy);
        if x >= 0 && y >= 0 && x < N as i64 && y < N as i64 {
            return Some((x as u32, y as u32));
        }
    }
    None
}

/// The output at `(cx, cy)` with the filter run over several rectangles containing it: the texel alone, the
/// whole image and a random tile.
fn centre_outputs(
    id: &str,
    p: &[(String, String)],
    target: EvalTarget,
    surface: Option<&SurfaceMap>,
    px: &[[f32; 4]],
    c: (u32, u32),
    tile: Rect,
) -> Vec<[f32; 4]> {
    let whole = Rect { x: 0, y: 0, w: N, h: N };
    let mut out = vec![apply(id, p, target, surface, px, Rect { x: c.0, y: c.1, w: 1, h: 1 })[0]];
    out.push(apply(id, p, target, surface, px, whole)[(c.1 * N + c.0) as usize]);
    out.push(apply(id, p, target, surface, px, tile)[((c.1 - tile.y) * tile.w + c.0 - tile.x) as usize]);
    out
}

/// A random tile of the image containing `c`.
fn tile_around(g: &mut Rng, c: (u32, u32)) -> Rect {
    let (x0, y0) = (g.below(c.0 + 1), g.below(c.1 + 1));
    let (x1, y1) = (c.0 + 1 + g.below(N - c.0), c.1 + 1 + g.below(N - c.1));
    Rect { x: x0, y: y0, w: x1 - x0, h: y1 - y0 }
}

fn targets(g: &mut Rng) -> EvalTarget {
    [EvalTarget::Mask, EvalTarget::Channel(Channel::Roughness), EvalTarget::Channel(Channel::Height)][g.below(3) as usize]
}

fn same(a: &[[f32; 4]], b: &[[f32; 4]], tolerance: f32) -> bool {
    a.iter().zip(b).all(|(p, q)| (0..4).all(|k| (p[k] - q[k]).abs() <= tolerance))
}

#[test]
fn area_filters_never_read_beyond_their_reach() {
    let surface = touching();
    let mut g = Rng(0x5eed_1234_abcd_0001);
    for info in FILTERS.iter().filter(|f| f.area) {
        let (mut checked, mut inside_mattered) = (0, 0);
        for trial in 0..60 {
            let p = random_params(info.id, &mut g);
            let target = targets(&mut g);
            let Some(r) = reach_of(info.id, &p, target, (N, N)).texels() else { panic!("{}: {p:?} reaches the whole image", info.id) };
            let surface = (trial % 2 == 0).then_some(&surface);
            let input = random_image(&mut g);
            let c = (g.below(N), g.below(N));
            let Some(k) = on_ring(&mut g, c.0, c.1, r + 1) else { continue };
            let tile = tile_around(&mut g, c);
            let before = centre_outputs(info.id, &p, target, surface, &input, c, tile);
            let mut changed = input.clone();
            changed[(k.1 * N + k.0) as usize] = [g.f(0.0, 1.0), g.f(0.0, 1.0), g.f(0.0, 1.0), g.f(0.5, 1.0)];
            let after = centre_outputs(info.id, &p, target, surface, &changed, c, tile);
            // running box sums carry rounding from far texels along a line
            assert!(same(&before, &after, 1e-4), "{} {p:?} {target:?}: texel {k:?} outside reach {r} of {c:?} changed {before:?} to {after:?}", info.id);
            checked += 1;
            // the same change next to the centre does change it sometimes: the test measures something
            if let Some(n) = on_ring(&mut g, c.0, c.1, 1) {
                let mut near = input.clone();
                near[(n.1 * N + n.0) as usize] = [1.0 - input[(n.1 * N + n.0) as usize][0], 0.5, 0.25, 1.0];
                inside_mattered += (!same(&before, &centre_outputs(info.id, &p, target, surface, &near, c, tile), 1e-4)) as u32;
            }
        }
        assert!(checked >= 30, "{}: only {checked} trials", info.id);
        assert!(inside_mattered > 0, "{}: no change near the centre ever mattered", info.id);
    }
}

#[test]
fn point_filters_read_only_their_texel() {
    let mut g = Rng(0x5eed_1234_abcd_0002);
    for info in FILTERS.iter().filter(|f| !f.area) {
        for _ in 0..40 {
            let p = random_params(info.id, &mut g);
            let target = targets(&mut g);
            assert_eq!(reach_of(info.id, &p, target, (N, N)), Reach::Texel, "{}", info.id);
            let input = random_image(&mut g);
            let c = (g.below(N), g.below(N));
            let k = loop {
                let k = (g.below(N), g.below(N));
                if k != c {
                    break k;
                }
            };
            let tile = tile_around(&mut g, c);
            let before = centre_outputs(info.id, &p, target, None, &input, c, tile);
            let mut changed = input.clone();
            changed[(k.1 * N + k.0) as usize] = [g.f(0.0, 1.0), g.f(0.0, 1.0), g.f(0.0, 1.0), g.f(0.0, 1.0)];
            assert_eq!(before, centre_outputs(info.id, &p, target, None, &changed, c, tile), "{} {p:?}", info.id);
        }
    }
}

#[test]
fn reaches_scale_with_the_resolution_and_tile_dependent_sizes_are_whole() {
    let m = EvalTarget::Mask;
    let blur = params(&[("radius", "0.002")]);
    let (small, large) = (reach_of(BLUR, &blur, m, (1024, 1024)).texels().unwrap(), reach_of(BLUR, &blur, m, (4096, 4096)).texels().unwrap());
    assert!(small > 0 && large >= 3 * small, "{small} {large}");
    // defaults at 2K stay small
    for info in FILTERS.iter().filter(|f| f.area) {
        let r = reach_of(info.id, &[], m, (2048, 2048)).texels().unwrap();
        assert!(r > 0 && r <= 128, "{}: {r}", info.id);
    }
    // sizes beyond the largest radius the compositor provides depend on the tiling
    for (id, p) in [
        (BLUR, params(&[("radius", "0.25")])),
        (BEVEL, params(&[("distance", "0.25")])),
        (MASK_OUTLINE, params(&[("width", "0.25"), ("position", "0")])),
        (WARP, params(&[("intensity", "0.25")])),
        (SLOPE_BLUR, params(&[("intensity", "0.25")])),
    ] {
        assert_eq!(reach_of(id, &p, m, (4096, 4096)), Reach::Whole, "{id}");
    }
    // a blur too small to do anything reads nothing around
    assert_eq!(reach_of(BLUR, &params(&[("radius", "0")]), m, (N, N)), Reach::Texel);
}

/// Why area filters reach twice their kernel: next to an island border, a texel the kernel reads across the
/// border is padded from the nearest texel of the centre's island, and among equally near ones that can be
/// as far again from the centre (here diagonally up the border).
#[test]
fn island_padding_doubles_the_kernel_extent() {
    let surface = touching();
    let p = params(&[("radius", &(1.0 / N as f32).to_string())]);
    let m = EvalTarget::Mask;
    assert_eq!(reach_of(BLUR, &p, m, (N, N)), Reach::Radius(6), "a sigma of one texel: kernel extent 3");
    let input: Vec<[f32; 4]> = (0..N * N).map(|i| if i % N < N / 2 { [0.8, 0.8, 0.8, 1.0] } else { [0.2, 0.2, 0.2, 1.0] }).collect();
    let (c, k) = ((31, 30), (31, 24));
    let mut changed = input.clone();
    changed[(k.1 * N + k.0) as usize] = [0.0, 0.0, 0.0, 1.0];
    let whole = Rect { x: 0, y: 0, w: N, h: N };
    let at = |px: &[[f32; 4]]| apply(BLUR, &p, m, Some(&surface), px, whole)[(c.1 * N + c.0) as usize];
    assert_ne!(at(&input), at(&changed), "texel {k:?}, 6 away from {c:?}, reaches it through the padding");
}

// SPDX-License-Identifier: MIT OR Apache-2.0
//! GPU-versus-CPU parity. Needs the `gpu` feature and an adapter (a software one is accepted here); skips otherwise.
#![cfg(feature = "gpu")]
// Edition 2024: `set_var` is unsafe (other threads may read the environment); every test here takes LOCK first.
#![allow(unsafe_code)]
use materialcraft::{gpu, noise, ops, tex::Tex};

static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Serialises the tests (they flip global switches) and reports whether an adapter exists.
fn setup() -> Option<std::sync::MutexGuard<'static, ()>> {
    let g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // SAFETY: the tests in this binary are serialised by LOCK and nothing else reads the environment meanwhile.
    unsafe { std::env::set_var("MC_GPU", "force") };
    gpu::set_enabled(true);
    gpu::set_min_pixels(0);
    if gpu::adapter_name().is_some() { Some(g) } else { None }
}

fn max_diff(a: &Tex, b: &Tex) -> f32 {
    assert_eq!((a.w, a.h), (b.w, b.h));
    a.px.iter().zip(&b.px).flat_map(|(p, q)| (0..4).map(move |c| (p[c] - q[c]).abs())).fold(0.0, f32::max)
}

#[test]
fn perlin_matches_cpu() {
    let Some(_guard) = setup() else { return };
    let g = gpu::noise(96, 64, false, 5, 3, 1, 0.5, 7, 0).expect("gpu perlin");
    let c = Tex::gray_fn(96, 64, |u, v| noise::perlin(u, v, 5, 3, 7));
    assert!(max_diff(&g, &c) < 2e-3, "{}", max_diff(&g, &c));
}

#[test]
fn fractal_matches_cpu_in_every_mode() {
    let Some(_guard) = setup() else { return };
    for mode in 0..3 {
        let g = gpu::noise(64, 64, true, 4, 4, 5, 0.5, 3, mode).expect("gpu fbm");
        let k = noise::Fbm::new(4, 4, 5, 0.5, 3, mode);
        let c = Tex::gray_fn(64, 64, |u, v| k.sample(u, v));
        assert!(max_diff(&g, &c) < 5e-3, "mode {mode}: {}", max_diff(&g, &c));
    }
}

#[test]
fn blur_matches_cpu() {
    let Some(_guard) = setup() else { return };
    let src = Tex::from_fn(80, 48, |u, v| [u, v, (u * 7.0).sin() * 0.5 + 0.5, 1.0]);
    let g = gpu::blur(&src, 6.0).expect("gpu blur");
    assert!(max_diff(&g, &ops::blur(&src, 6.0)) < 1e-3);
}

#[test]
fn normal_matches_cpu() {
    let Some(_guard) = setup() else { return };
    let src = Tex::gray_fn(64, 64, |u, v| noise::perlin(u, v, 4, 4, 1));
    for dx in [false, true] {
        let g = gpu::normal_from_height(&src, 2.0, dx).expect("gpu normal");
        assert!(max_diff(&g, &ops::normal_from_height(&src, 2.0, dx)) < 1e-3);
    }
}

#[test]
fn small_textures_stay_on_the_cpu_by_default() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    gpu::set_min_pixels(1 << 20);
    assert!(gpu::noise(8, 8, false, 4, 4, 1, 0.5, 0, 0).is_none());
    gpu::set_min_pixels(0);
}

#[test]
fn levels_matches_cpu() {
    let Some(_guard) = setup() else { return };
    let src = Tex::from_fn(64, 32, |u, v| [u, v, (u * 5.0).sin() * 0.5 + 0.5, 0.75]);
    for (lo, hi, g, ol, oh) in [(0.1, 0.9, 1.0, 0.0, 1.0), (0.2, 0.7, 2.2, 0.1, 0.8), (0.9, 0.1, 1.0, 0.0, 1.0)] {
        let a = gpu::levels(&src, lo, hi, g, ol, oh).expect("gpu levels");
        assert!(max_diff(&a, &ops::levels(&src, lo, hi, g, ol, oh)) < 2e-4);
    }
}

#[test]
fn directional_blur_matches_cpu() {
    let Some(_guard) = setup() else { return };
    let src = Tex::from_fn(64, 48, |u, v| [u, (v * 9.0).sin() * 0.5 + 0.5, 0.3, 1.0]);
    for (a, l) in [(0.0, 0.1), (0.125, 0.3), (0.7, 0.05)] {
        let g = gpu::directional_blur(&src, a, l).expect("gpu directional blur");
        assert!(max_diff(&g, &ops::directional_blur(&src, a, l)) < 1e-3, "angle {a} length {l}");
    }
}

#[test]
fn blend_matches_cpu_in_every_mode() {
    let Some(_guard) = setup() else { return };
    let fg = Tex::from_fn(40, 24, |u, v| [u, v, 1.0 - u, 0.8]);
    let bg = Tex::from_fn(40, 24, |u, v| [(v * 3.0).fract(), u * 0.5, 0.4, 1.0]);
    let mask = Tex::from_fn(40, 24, |u, _| [u, u, u, 1.0]);
    for mode in (0..=18).chain([20, 21]) {
        for am in 0..4 {
            for m in [None, Some(&mask)] {
                let g = gpu::blend(&fg, &bg, m, mode, 0.7, am).expect("gpu blend");
                let c = ops::blend_ex(&fg, &bg, m, mode, 0.7, am, [0.0, 0.0, 1.0, 1.0]);
                assert!(max_diff(&g, &c) < 2e-4, "mode {mode} alpha {am}: {}", max_diff(&g, &c));
            }
        }
    }
    // dissolve and cropped blends stay on the CPU
    assert!(gpu::blend(&fg, &bg, None, 19, 1.0, 1).is_none());
}

#[test]
fn whole_graph_matches_the_cpu() {
    use materialcraft::graph::Graph;
    let Some(_guard) = setup() else { return };
    let g = Graph::parse(concat!(
        "size 64 48\n",
        "node 1 fbm scale=5 octaves=4\nnode 2 levels in=@1 low=0.2 high=0.8 gamma=1.4\nnode 3 blur in=@2 radius=6\n",
        "node 4 directional_blur in=@3 angle=0.1 length=0.1\nnode 5 perlin scale=3 seed=4\n",
        "node 6 blend fg=@5 bg=@4 mode=5 opacity=0.8\nnode 7 normal in=@6 strength=3\n",
        "output h 6\noutput n 7\n",
    ))
    .unwrap();
    gpu::set_enabled(true);
    assert!(gpu::active());
    let on = g.evaluate(None).unwrap();
    gpu::set_enabled(false);
    assert!(!gpu::active());
    let off = g.evaluate(None).unwrap();
    gpu::set_enabled(true);
    assert!(max_diff(&on["h"], &off["h"]) < 3e-3, "height {}", max_diff(&on["h"], &off["h"]));
    assert!(max_diff(&on["n"], &off["n"]) < 2e-2, "normal {}", max_diff(&on["n"], &off["n"]));
}

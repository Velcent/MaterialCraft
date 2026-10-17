// SPDX-License-Identifier: MIT OR Apache-2.0
use materialcraft::graph::{EXAMPLE, Graph};
use materialcraft::render3d::{Maps, View, render};
use materialcraft::tex::Tex;
fn main() {
    let g = Graph::parse(EXAMPLE).unwrap();
    let out = g.evaluate(Some(512)).unwrap();
    let m = Maps {
        base: &out["basecolor"],
        normal: &out["normal"],
        rough: &out["roughness"],
        metal: &out["metallic"],
        ao: &out["ambient_occlusion"],
        emissive: None,
        height: Some(&out["height"]),
    };
    let mut sheet: Vec<Tex> = Vec::new();
    for (hs, bump, shape) in [(0.0, 0.0, 0u32), (0.08, 1.0, 0), (0.25, 3.0, 0), (0.25, 3.0, 2)] {
        let v = View { height_scale: hs, bump, shape, yaw: 0.9, pitch: 0.25, ..View::default() };
        sheet.push(render(&m, &v, 420, 420));
    }
    let mut px = vec![[0.0f32; 4]; 420 * 4 * 420];
    for (i, t) in sheet.iter().enumerate() {
        for y in 0..420 {
            for x in 0..420 {
                px[y * 1680 + i * 420 + x] = t.px[y * 420 + x];
            }
        }
    }
    let big = Tex { w: 1680, h: 420, px };
    std::fs::write(std::env::temp_dir().join("h3d.png"), materialcraft::png::encode(big.w, big.h, &big.px, 8)).unwrap();
}

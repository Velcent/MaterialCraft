//! Making one thumbnail (on a worker thread): an image asset (or an asset's thumbnail file) decoded from PNG or
//! OpenEXR and scaled down, or a material graph evaluated small through `material_fill` and shown on a lit ball
//! by the software renderer (`render3d`).

use std::collections::BTreeMap;
use std::path::Path;

use materialcraft::material_fill::{MAX_GRAPH_BYTES, Material};
use materialcraft::paint::working_space::linear_to_srgb;
use materialcraft::render3d::{self, Maps, View};
use materialcraft::tex::Tex;
use materialcraft::{exr, png};

/// Longest side of a thumbnail, in pixels.
pub const THUMB_PX: usize = 128;

/// Largest image file read for a thumbnail.
const MAX_IMAGE_BYTES: u64 = 512 << 20;

/// Size a material graph is evaluated at for its ball.
const MATERIAL_EVAL: usize = 128;

/// The image at `path`, scaled to fit [`THUMB_PX`], display-encoded.
pub fn image(path: &Path) -> Result<Tex, String> {
    let bytes = read(path, MAX_IMAGE_BYTES)?;
    let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    let tex = match ext.as_str() {
        "exr" => exr_to_display(&exr::decode(&bytes)?)?,
        _ => png::decode(&bytes)?,
    };
    Ok(fit(&tex, THUMB_PX))
}

/// The material graph at `path` on a ball, [`THUMB_PX`] square.
pub fn material(path: &Path) -> Result<Tex, String> {
    let text = String::from_utf8(read(path, MAX_GRAPH_BYTES)?).map_err(|_| "the graph is not UTF-8 text".to_string())?;
    let material = Material::from_text(&text, &[]).map_err(|e| e.to_string())?;
    let graph = material.graph();
    let mut outputs = graph.evaluate(Some(MATERIAL_EVAL)).map_err(|e| e.to_string())?;
    let mut by_usage: BTreeMap<String, Tex> = BTreeMap::new();
    for (usage, ident, _) in graph.output_nodes() {
        if !by_usage.contains_key(&usage)
            && let Some(t) = outputs.remove(&ident)
        {
            by_usage.insert(usage, t);
        }
    }
    let s = MATERIAL_EVAL;
    let (half, flat, one, zero) =
        (Tex::solid(s, s, [0.5, 0.5, 0.5, 1.0]), Tex::solid(s, s, [0.5, 0.5, 1.0, 1.0]), Tex::solid(s, s, [1.0; 4]), Tex::solid(s, s, [0.0, 0.0, 0.0, 1.0]));
    let get = |names: &[&str]| names.iter().find_map(|n| by_usage.get(*n));
    let maps = Maps {
        base: get(&["basecolor", "diffuse"]).unwrap_or(&half),
        normal: get(&["normal"]).unwrap_or(&flat),
        rough: get(&["roughness"]).unwrap_or(&half),
        metal: get(&["metallic"]).unwrap_or(&zero),
        ao: get(&["ambient_occlusion"]).unwrap_or(&one),
        emissive: get(&["emissive"]),
        height: get(&["height"]),
    };
    let view = View { tess: 48, relief: render3d::RELIEF_BUMP, ..View::default() };
    Ok(render3d::render(&maps, &view, THUMB_PX, THUMB_PX))
}

fn read(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let len = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if len > limit {
        return Err(format!("{len} bytes is more than a thumbnail reads"));
    }
    std::fs::read(path).map_err(|e| e.to_string())
}

/// Linear EXR values (R, G, B or Y, and A) as display values.
fn exr_to_display(img: &exr::ExrImage) -> Result<Tex, String> {
    let n = img.width * img.height;
    let plane = |names: &[&str]| names.iter().find_map(|c| img.channel(c)).map(|c| &c.data).filter(|d| d.len() == n);
    let grey = plane(&["Y", "L"]);
    let (r, g, b) = match (plane(&["R"]), plane(&["G"]), plane(&["B"])) {
        (Some(r), Some(g), Some(b)) => (r, g, b),
        _ => match grey.or_else(|| img.channels.first().map(|c| &c.data).filter(|d| d.len() == n)) {
            Some(y) => (y, y, y),
            None => return Err("the image has no colour or grey channel".into()),
        },
    };
    let a = plane(&["A"]);
    let enc = |v: f32| if v.is_finite() { linear_to_srgb(v.clamp(0.0, 1.0)) } else { 0.0 };
    let px = (0..n)
        .map(|i| {
            let at = |p: &Vec<f32>| p.get(i).copied().unwrap_or(0.0);
            [enc(at(r)), enc(at(g)), enc(at(b)), a.map_or(1.0, |a| at(a).clamp(0.0, 1.0))]
        })
        .collect();
    Ok(Tex { w: img.width, h: img.height, px })
}

/// `t` scaled down (box filter) to fit `side` × `side`; smaller images are kept as they are.
pub fn fit(t: &Tex, side: usize) -> Tex {
    if t.w <= side && t.h <= side || t.w == 0 || t.h == 0 {
        return Tex { w: t.w, h: t.h, px: t.px.clone() };
    }
    let scale = (t.w.max(t.h) as f32) / side as f32;
    let (w, h) = (((t.w as f32 / scale).round() as usize).max(1), ((t.h as f32 / scale).round() as usize).max(1));
    let mut px = Vec::with_capacity(w * h);
    for y in 0..h {
        let (y0, y1) = (y * t.h / h, ((y + 1) * t.h / h).max(y * t.h / h + 1));
        for x in 0..w {
            let (x0, x1) = (x * t.w / w, ((x + 1) * t.w / w).max(x * t.w / w + 1));
            let mut sum = [0.0f32; 4];
            let mut count = 0.0;
            for sy in y0..y1.min(t.h) {
                for sx in x0..x1.min(t.w) {
                    if let Some(p) = t.px.get(sy * t.w + sx) {
                        for (s, v) in sum.iter_mut().zip(p) {
                            *s += v;
                        }
                        count += 1.0;
                    }
                }
            }
            px.push(if count > 0.0 { sum.map(|s| s / count) } else { [0.0; 4] });
        }
    }
    Tex { w, h, px }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_keeps_the_aspect_and_averages() {
        let t = Tex { w: 512, h: 256, px: (0..512 * 256).map(|i| if i % 2 == 0 { [1.0; 4] } else { [0.0, 0.0, 0.0, 1.0] }).collect() };
        let f = fit(&t, THUMB_PX);
        assert_eq!((f.w, f.h), (128, 64));
        assert!(f.px.iter().all(|p| (p[0] - 0.5).abs() < 1e-5 && p[3] == 1.0));
        assert_eq!(fit(&Tex::solid(8, 4, [0.2; 4]), THUMB_PX).w, 8);
    }

    #[test]
    fn png_and_exr_images_and_a_material_make_thumbnails() {
        let dir = std::env::temp_dir().join(format!("mc-tray-thumbs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("a.png");
        std::fs::write(&p, png::encode(300, 200, &vec![[0.25, 0.5, 0.75, 1.0]; 300 * 200], 8)).unwrap();
        let t = image(&p).unwrap();
        assert_eq!((t.w, t.h), (128, 85));
        let e = dir.join("b.exr");
        let data = vec![0.5f32; 64 * 64];
        let channels = ["B", "G", "R"].iter().map(|n| exr::ExrChannel::new(n, exr::SampleKind::Half, data.clone())).collect();
        std::fs::write(&e, exr::encode(&exr::ExrImage { width: 64, height: 64, channels }).unwrap()).unwrap();
        let t = image(&e).unwrap();
        assert!((t.px[0][0] - linear_to_srgb(0.5)).abs() < 1e-2, "{:?}", t.px[0]);
        let m = dir.join("c.mca");
        std::fs::write(&m, materialcraft::graph::EXAMPLE).unwrap();
        let t = material(&m).unwrap();
        assert_eq!((t.w, t.h), (THUMB_PX, THUMB_PX));
        assert!(image(&dir.join("missing.png")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

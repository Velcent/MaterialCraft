//! CPU side of the material maps: graph outputs converted into upload-ready buffers.
use crate::tex::{lum, par_map, Tex};

/// Texture data prepared on the CPU (resampled to one size, packed, converted) and ready to upload.
pub struct GpuMaps {
    pub size: usize,
    /// sRGB RGBA8
    pub base: Vec<u8>,
    /// linear RGBA8 tangent-space normal
    pub normal: Vec<u8>,
    /// R = roughness, G = metallic, B = ambient occlusion
    pub rma: Vec<u8>,
    /// sRGB RGBA8, absent when the graph has no emissive output
    pub emissive: Option<Vec<u8>>,
    /// luminance height (w, h, values)
    pub height: Option<(usize, usize, Vec<f32>)>,
}

pub struct MapInputs<'a> {
    pub base: Option<&'a Tex>,
    pub normal: Option<&'a Tex>,
    pub rough: Option<&'a Tex>,
    pub metal: Option<&'a Tex>,
    pub ao: Option<&'a Tex>,
    pub emissive: Option<&'a Tex>,
    pub height: Option<&'a Tex>,
}

fn b8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

/// Convert graph outputs into upload-ready buffers. All colour maps are brought to the largest map size.
/// Bring a map to `size` x `size`: exact box averaging when shrinking by a whole factor (no aliasing, e.g. 8K -> 4K),
/// bilinear otherwise.
fn fit(t: &Tex, size: usize) -> Tex {
    if t.w == size && t.h == size {
        return t.clone();
    }
    if t.w > size && t.h > size && t.w.is_multiple_of(size) && t.h.is_multiple_of(size) {
        let (fx, fy) = (t.w / size, t.h / size);
        let inv = 1.0 / (fx * fy) as f32;
        let px = par_map(size * size, |i| {
            let (x0, y0) = ((i % size) * fx, (i / size) * fy);
            let mut a = [0.0f32; 4];
            for y in y0..y0 + fy {
                for x in x0..x0 + fx {
                    let p = t.px[y * t.w + x];
                    for c in 0..4 {
                        a[c] += p[c];
                    }
                }
            }
            [a[0] * inv, a[1] * inv, a[2] * inv, a[3] * inv]
        });
        return Tex { w: size, h: size, px };
    }
    Tex::from_fn(size, size, |u, v| t.sample(u, v))
}

/// Convert graph outputs into upload-ready buffers. All colour maps are brought to the largest map size.
pub fn prepare(m: &MapInputs) -> GpuMaps {
    let all = [m.base, m.normal, m.rough, m.metal, m.ao, m.emissive];
    let size = all.iter().flatten().map(|t| t.w.max(t.h)).max().unwrap_or(4).clamp(1, 4096);
    let fitted: Vec<Option<Tex>> = all.iter().map(|t| t.filter(|t| t.w != size || t.h != size).map(|t| fit(t, size))).collect();
    let px = |k: usize, t: Option<&Tex>, dflt: [f32; 4], i: usize| -> [f32; 4] {
        match (&fitted[k], t) {
            (Some(f), _) => f.px[i],
            (None, Some(t)) => t.px[i],
            (None, None) => dflt,
        }
    };
    let n = size * size;
    let rgba = |k: usize, t: Option<&Tex>, dflt: [f32; 4]| -> Vec<u8> {
        let v: Vec<[u8; 4]> = par_map(n, |i| {
            let p = px(k, t, dflt, i);
            [b8(p[0]), b8(p[1]), b8(p[2]), 255]
        });
        v.into_iter().flatten().collect()
    };
    let base = rgba(0, m.base, [0.5, 0.5, 0.5, 1.0]);
    let normal = rgba(1, m.normal, [0.5, 0.5, 1.0, 1.0]);
    let rma: Vec<u8> = {
        let v: Vec<[u8; 4]> = par_map(n, |i| {
            let r = lum(px(2, m.rough, [0.5, 0.5, 0.5, 1.0], i));
            let me = lum(px(3, m.metal, [0.0, 0.0, 0.0, 1.0], i));
            let a = lum(px(4, m.ao, [1.0, 1.0, 1.0, 1.0], i));
            [b8(r), b8(me), b8(a), 255]
        });
        v.into_iter().flatten().collect()
    };
    let emissive = m.emissive.map(|e| rgba(5, Some(e), [0.0, 0.0, 0.0, 1.0]));
    let height = m.height.map(|h| (h.w, h.h, h.px.iter().map(|p| lum(*p)).collect::<Vec<f32>>()));
    GpuMaps { size, base, normal, rma, emissive, height }
}

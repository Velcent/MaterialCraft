//! CPU side of the material maps: graph outputs converted into upload-ready buffers.
use crate::tex::{Tex, lum, par_map};

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

/// New texels for rectangles of maps already uploaded (a brush stroke's tiles).
pub struct MapsPatch {
    pub tiles: Vec<MapsTile>,
}

/// New texels for one rectangle: the same layout as [`GpuMaps`], rows of `w` texels. Maps the uploaded set does not
/// have are ignored, as is a rectangle outside their size.
pub struct MapsTile {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
    pub base: Vec<u8>,
    pub normal: Vec<u8>,
    pub rma: Vec<u8>,
    pub emissive: Option<Vec<u8>>,
    pub height: Option<Vec<f32>>,
}

impl MapsPatch {
    /// Bytes this patch uploads.
    pub fn bytes(&self) -> usize {
        self.tiles
            .iter()
            .map(|t| t.base.len() + t.normal.len() + t.rma.len() + t.emissive.as_ref().map_or(0, Vec::len) + t.height.as_ref().map_or(0, |h| h.len() * 4))
            .sum()
    }
}

/// One rectangle's maps for [`prepare_patch`]: as [`MapInputs`], every map holding the rectangle's `w × h` texels
/// row by row (maps of another length count as absent).
pub struct TileInputs<'a> {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
    pub base: Option<&'a [[f32; 4]]>,
    pub normal: Option<&'a [[f32; 4]]>,
    pub rough: Option<&'a [[f32; 4]]>,
    pub metal: Option<&'a [[f32; 4]]>,
    pub ao: Option<&'a [[f32; 4]]>,
    pub emissive: Option<&'a [[f32; 4]]>,
    pub height: Option<&'a [[f32; 4]]>,
}

/// Upload-ready texels for rectangles of maps that [`prepare`] made from maps of the same size: the bytes `prepare`
/// gives those texels.
pub fn prepare_patch(tiles: &[TileInputs]) -> MapsPatch {
    let tiles = tiles
        .iter()
        .map(|t| {
            let n = t.w * t.h;
            fn sized(m: Option<&[[f32; 4]]>, n: usize) -> Option<&[[f32; 4]]> {
                m.filter(|m| m.len() == n)
            }
            let get = |m| sized(m, n);
            let at = |m: Option<&[[f32; 4]]>, dflt: [f32; 4], i: usize| sized(m, n).and_then(|m| m.get(i)).copied().unwrap_or(dflt);
            let rgba = |m: Option<&[[f32; 4]]>, dflt: [f32; 4]| -> Vec<u8> { (0..n).flat_map(|i| rgba_texel(at(m, dflt, i))).collect() };
            let rma = (0..n).flat_map(|i| rma_texel(at(t.rough, ROUGH, i), at(t.metal, BLACK, i), at(t.ao, AO, i))).collect();
            MapsTile {
                x: t.x,
                y: t.y,
                w: t.w,
                h: t.h,
                base: rgba(t.base, BASE),
                normal: rgba(t.normal, NORMAL),
                rma,
                emissive: get(t.emissive).map(|e| e.iter().flat_map(|p| rgba_texel(*p)).collect()),
                height: get(t.height).map(|h| h.iter().map(|p| lum(*p)).collect()),
            }
        })
        .collect();
    MapsPatch { tiles }
}

/// Defaults of absent maps.
const BASE: [f32; 4] = [0.5, 0.5, 0.5, 1.0];
const NORMAL: [f32; 4] = [0.5, 0.5, 1.0, 1.0];
const ROUGH: [f32; 4] = [0.5, 0.5, 0.5, 1.0];
const BLACK: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
const AO: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

/// An RGBA8 texel (alpha opaque).
fn rgba_texel(p: [f32; 4]) -> [u8; 4] {
    [b8(p[0]), b8(p[1]), b8(p[2]), 255]
}

/// A packed roughness / metallic / occlusion texel.
fn rma_texel(r: [f32; 4], m: [f32; 4], a: [f32; 4]) -> [u8; 4] {
    [b8(lum(r)), b8(lum(m)), b8(lum(a)), 255]
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
        let v: Vec<[u8; 4]> = par_map(n, |i| rgba_texel(px(k, t, dflt, i)));
        v.into_iter().flatten().collect()
    };
    let base = rgba(0, m.base, BASE);
    let normal = rgba(1, m.normal, NORMAL);
    let rma: Vec<u8> = {
        let v: Vec<[u8; 4]> = par_map(n, |i| rma_texel(px(2, m.rough, ROUGH, i), px(3, m.metal, BLACK, i), px(4, m.ao, AO, i)));
        v.into_iter().flatten().collect()
    };
    let emissive = m.emissive.map(|e| rgba(5, Some(e), BLACK));
    let height = m.height.map(|h| (h.w, h.h, h.px.iter().map(|p| lum(*p)).collect::<Vec<f32>>()));
    GpuMaps { size, base, normal, rma, emissive, height }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tex(seed: f32) -> Tex {
        Tex::from_fn(8, 8, |u, v| [(u * 3.1 + seed).fract(), (v * 2.3 + seed).fract(), (u * v + seed).fract(), 1.0])
    }

    /// The texels of `rect` of `t`, row by row.
    fn cut(t: &Tex, (x, y, w, h): (usize, usize, usize, usize)) -> Vec<[f32; 4]> {
        (y..y + h).flat_map(|yy| (x..x + w).map(move |xx| (xx, yy))).map(|(xx, yy)| t.px[yy * t.w + xx]).collect()
    }

    #[test]
    fn a_patch_has_the_bytes_of_the_whole_preparation() {
        let maps: Vec<Tex> = (0..7).map(|k| tex(k as f32 * 0.17)).collect();
        let whole = prepare(&MapInputs {
            base: Some(&maps[0]),
            normal: Some(&maps[1]),
            rough: Some(&maps[2]),
            metal: None,
            ao: Some(&maps[4]),
            emissive: Some(&maps[5]),
            height: Some(&maps[6]),
        });
        let r = (2, 3, 4, 2);
        let cuts: Vec<Vec<[f32; 4]>> = maps.iter().map(|m| cut(m, r)).collect();
        let patch = prepare_patch(&[TileInputs {
            x: r.0,
            y: r.1,
            w: r.2,
            h: r.3,
            base: Some(&cuts[0]),
            normal: Some(&cuts[1]),
            rough: Some(&cuts[2]),
            metal: None,
            ao: Some(&cuts[4]),
            emissive: Some(&cuts[5]),
            height: Some(&cuts[6]),
        }]);
        let t = &patch.tiles[0];
        for (i, (xx, yy)) in (r.1..r.1 + r.3).flat_map(|yy| (r.0..r.0 + r.2).map(move |xx| (xx, yy))).enumerate() {
            let at = yy * whole.size + xx;
            assert_eq!(t.base[i * 4..i * 4 + 4], whole.base[at * 4..at * 4 + 4]);
            assert_eq!(t.normal[i * 4..i * 4 + 4], whole.normal[at * 4..at * 4 + 4]);
            assert_eq!(t.rma[i * 4..i * 4 + 4], whole.rma[at * 4..at * 4 + 4]);
            assert_eq!(t.emissive.as_ref().unwrap()[i * 4..i * 4 + 4], whole.emissive.as_ref().unwrap()[at * 4..at * 4 + 4]);
            assert_eq!(t.height.as_ref().unwrap()[i].to_bits(), whole.height.as_ref().unwrap().2[at].to_bits());
        }
        assert_eq!(patch.bytes(), 8 * 4 * 4 + 8 * 4);
    }
}

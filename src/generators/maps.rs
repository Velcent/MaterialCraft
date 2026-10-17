//! [`SetMaps`]: the baked mesh maps of a texture set as the generators read them: ambient occlusion,
//! curvature and thickness as [`Pyramid`]s (any blur radius), world-space normal and position as vectors.
//! Every map is read by UV, so baked maps of another resolution than the set need no resampling; a missing
//! map reads as its neutral value (open AO, flat curvature, mid thickness, normal up, centred position).
//!
//! Conventions of the stored maps: AO 0 occluded .. 1 open; curvature 0.5 flat, brighter on convex edges,
//! darker in cavities; world normals encoded 0..1 per axis (decoded here to −1..1); position 0..1 across the
//! scene's bounding box; thickness 0 thin .. 1 thick.

use super::pyramid::Pyramid;
use crate::paint::{BakedKind, TextureSet, TiledImage};

/// Full-resolution levels above this side are dropped: generators are smooth enough at 2K, and a 4K set's
/// maps would otherwise take hundreds of megabytes.
pub const MAX_SIDE: usize = 2048;

/// A vector map read by nearest texel.
struct VectorMap {
    w: usize,
    h: usize,
    px: Vec<[f32; 3]>,
}

impl VectorMap {
    fn at(&self, u: f32, v: f32) -> Option<[f32; 3]> {
        let x = ((u * self.w as f32) as isize).clamp(0, self.w as isize - 1) as usize;
        let y = ((v * self.h as f32) as isize).clamp(0, self.h as isize - 1) as usize;
        self.px.get(y * self.w + x).copied()
    }
}

pub struct SetMaps {
    ao: Option<Pyramid>,
    curvature: Option<Pyramid>,
    thickness: Option<Pyramid>,
    normal: Option<VectorMap>,
    position: Option<VectorMap>,
}

impl SetMaps {
    pub fn of(set: &TextureSet) -> SetMaps {
        let grey = |kind: BakedKind| set.baked(&kind).map(grey_pyramid);
        SetMaps {
            ao: grey(BakedKind::AmbientOcclusion),
            curvature: grey(BakedKind::Curvature),
            thickness: grey(BakedKind::Thickness),
            normal: set.baked(&BakedKind::WorldSpaceNormal).map(|i| vector_map(i, |c| [c[0] * 2.0 - 1.0, c[1] * 2.0 - 1.0, c[2] * 2.0 - 1.0])),
            position: set.baked(&BakedKind::Position).map(|i| vector_map(i, |c| [c[0], c[1], c[2]])),
        }
    }

    /// Ambient occlusion blurred by `blur` (a fraction of the texture width).
    pub fn ao(&self, u: f32, v: f32, blur: f32) -> f32 {
        read(&self.ao, u, v, blur, 1.0)
    }

    pub fn curvature(&self, u: f32, v: f32, blur: f32) -> f32 {
        read(&self.curvature, u, v, blur, 0.5)
    }

    pub fn thickness(&self, u: f32, v: f32, blur: f32) -> f32 {
        read(&self.thickness, u, v, blur, 0.5)
    }

    /// World-space normal, −1..1 per axis (Y up).
    pub fn normal(&self, u: f32, v: f32) -> [f32; 3] {
        self.normal.as_ref().and_then(|m| m.at(u, v)).unwrap_or([0.0, 1.0, 0.0])
    }

    /// Position, 0..1 per axis across the scene.
    pub fn position(&self, u: f32, v: f32) -> [f32; 3] {
        self.position.as_ref().and_then(|m| m.at(u, v)).unwrap_or([0.5; 3])
    }
}

fn read(p: &Option<Pyramid>, u: f32, v: f32, blur: f32, neutral: f32) -> f32 {
    match p {
        Some(p) => p.sample(u, v, blur.max(0.0) * p.width() as f32),
        None => neutral,
    }
}

/// The image's grey value (red) as a pyramid, halved until it fits [`MAX_SIDE`].
fn grey_pyramid(image: &TiledImage) -> Pyramid {
    let (w, h, px) = shrink(image, |c| c[0]);
    Pyramid::new(w, h, px)
}

fn vector_map(image: &TiledImage, decode: impl Fn([f32; 4]) -> [f32; 3]) -> VectorMap {
    let (w, h, px) = shrink(image, decode);
    VectorMap { w, h, px }
}

/// The image's texels through `f`, every `step`-th texel so the result fits [`MAX_SIDE`].
fn shrink<T: Copy + Default>(image: &TiledImage, f: impl Fn([f32; 4]) -> T) -> (usize, usize, Vec<T>) {
    let (iw, ih) = (image.width() as usize, image.height() as usize);
    let step = iw.max(ih).div_ceil(MAX_SIDE).max(1);
    let (w, h) = (iw.div_ceil(step).max(1), ih.div_ceil(step).max(1));
    let mut px = vec![T::default(); w * h];
    let mut row = vec![[0.0f32; 4]; iw];
    for y in 0..h {
        let sy = (y * step) as u32;
        if sy >= ih as u32 {
            break;
        }
        image.read_region(crate::paint::Rect { x: 0, y: sy, w: iw as u32, h: 1 }, &mut row);
        for x in 0..w {
            if let (Some(c), Some(p)) = (row.get(x * step), px.get_mut(y * w + x)) {
                *p = f(*c);
            }
        }
    }
    (w, h, px)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::{BakedMap, PixelFormat};

    #[test]
    fn missing_maps_are_neutral_and_present_ones_read_by_uv() {
        let mut set = TextureSet::new("s", "S", (8, 8));
        let empty = SetMaps::of(&set);
        assert_eq!((empty.ao(0.5, 0.5, 0.0), empty.curvature(0.1, 0.1, 0.1)), (1.0, 0.5));
        assert_eq!(empty.normal(0.5, 0.5), [0.0, 1.0, 0.0]);
        let mut ao = TiledImage::new(4, 4, PixelFormat::from_id("gray8").unwrap()).unwrap();
        ao.set(3, 0, [1.0; 4]);
        set.baked.push(BakedMap { kind: BakedKind::AmbientOcclusion, image: ao });
        let mut n = TiledImage::new(2, 2, PixelFormat::from_id("rgb8").unwrap()).unwrap();
        n.fill([0.5, 0.5, 1.0, 1.0]);
        set.baked.push(BakedMap { kind: BakedKind::WorldSpaceNormal, image: n });
        let maps = SetMaps::of(&set);
        assert_eq!(maps.ao(0.9, 0.1, 0.0), 1.0);
        assert_eq!(maps.ao(0.1, 0.9, 0.0), 0.0);
        assert!(maps.normal(0.3, 0.3).iter().zip([0.0, 0.0, 1.0]).all(|(a, b)| (a - b).abs() < 0.01));
    }
}

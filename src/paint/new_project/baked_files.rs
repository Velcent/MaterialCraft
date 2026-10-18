//! Reading a baked map file (PNG or OpenEXR) into a [`BakedMap`] stored like the bakers' own results: 16 bits per
//! sample, grey for single-value maps. A tangent-space normal map in the DirectX convention gets its green flipped,
//! since documents keep normal maps in the OpenGL convention.

use crate::paint::bake_config::{map_kind, storage_format};
use crate::paint::project_settings::NormalMapFormat;
use crate::paint::{BakedKind, BakedMap, TiledImage};
use crate::tex::Tex;
use std::path::Path;

/// Reads the image at `path` as a baked map of `kind`; normal maps are read as `normals`.
pub fn read(path: &Path, kind: &BakedKind, normals: NormalMapFormat) -> Result<BakedMap, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    from_bytes(&bytes, kind, normals)
}

/// [`read`] from the file's bytes (PNG or OpenEXR, told apart by their signatures).
pub fn from_bytes(bytes: &[u8], kind: &BakedKind, normals: NormalMapFormat) -> Result<BakedMap, String> {
    let mut tex = decode(bytes)?;
    if *kind == BakedKind::Normal && normals.flips_green() {
        tex.px.iter_mut().for_each(|p| p[1] = 1.0 - p[1]);
    }
    let map = map_kind(kind).ok_or_else(|| format!("{} maps are not read from files", kind.id()))?;
    let image = TiledImage::from_tex(&tex, storage_format(map)).map_err(|e| e.to_string())?;
    Ok(BakedMap { kind: kind.clone(), image })
}

const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
const EXR_SIGNATURE: [u8; 4] = [0x76, 0x2f, 0x31, 0x01];

fn decode(bytes: &[u8]) -> Result<Tex, String> {
    if bytes.starts_with(&PNG_SIGNATURE) {
        return crate::png::decode(bytes);
    }
    if bytes.starts_with(&EXR_SIGNATURE) {
        return exr_tex(bytes);
    }
    Err("not a PNG or OpenEXR image".into())
}

/// An OpenEXR image's R, G, B and A (a grey `Y` or single channel fills R, G and B; a missing alpha is 1).
fn exr_tex(bytes: &[u8]) -> Result<Tex, String> {
    let img = crate::exr::decode(bytes)?;
    let grey = img.channel("Y").or_else(|| (img.channels.len() == 1).then(|| img.channels.first()).flatten());
    let pick = |name: &str| img.channel(name).or(grey).map(|c| c.data.as_slice());
    let (Some(r), Some(g), Some(b)) = (pick("R"), pick("G"), pick("B")) else {
        return Err("the OpenEXR file has no R, G and B (or Y) channels".into());
    };
    let a = img.channel("A").map(|c| c.data.as_slice());
    let n = img.width.checked_mul(img.height).ok_or("the OpenEXR image is too large")?;
    let at = |c: &[f32], i: usize| c.get(i).copied().unwrap_or(0.0);
    if [r, g, b].iter().any(|c| c.len() < n) {
        return Err("the OpenEXR file's channels are shorter than the image".into());
    }
    let px = (0..n).map(|i| [at(r, i), at(g, i), at(b, i), a.map_or(1.0, |a| at(a, i))]).collect();
    Ok(Tex { w: img.width, h: img.height, px })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::PixelLayout;

    fn png(w: usize, h: usize, c: [f32; 4]) -> Vec<u8> {
        crate::png::encode(w, h, &Tex::solid(w, h, c).px, 16)
    }

    #[test]
    fn grey_maps_are_stored_grey_and_normals_flip_for_directx() {
        let ao = from_bytes(&png(8, 4, [0.5, 0.5, 0.5, 1.0]), &BakedKind::AmbientOcclusion, NormalMapFormat::OpenGl).unwrap();
        assert_eq!(ao.image.size(), (8, 4));
        assert_eq!(ao.image.format().layout, PixelLayout::Gray);
        let bytes = png(4, 4, [0.5, 0.25, 1.0, 1.0]);
        let gl = from_bytes(&bytes, &BakedKind::Normal, NormalMapFormat::OpenGl).unwrap();
        let dx = from_bytes(&bytes, &BakedKind::Normal, NormalMapFormat::DirectX).unwrap();
        let green = |m: &BakedMap| m.image.to_tex().px[0][1];
        assert!((green(&gl) - 0.25).abs() < 0.01 && (green(&dx) - 0.75).abs() < 0.01);
    }

    #[test]
    fn exr_and_bad_files() {
        let tex = Tex::solid(2, 2, [0.25, 0.5, 0.75, 1.0]);
        let exr = crate::exr::encode_exr(&tex, true);
        let pos = from_bytes(&exr, &BakedKind::Position, NormalMapFormat::OpenGl).unwrap();
        assert!((pos.image.to_tex().px[3][2] - 0.75).abs() < 0.001);
        assert!(from_bytes(b"GIF89a", &BakedKind::Id, NormalMapFormat::OpenGl).is_err());
        assert!(from_bytes(&png(2, 2, [1.0; 4]), &BakedKind::Other("bent".into()), NormalMapFormat::OpenGl).is_err());
    }
}

// SPDX-License-Identifier: MIT OR Apache-2.0
//! Environment maps for image-based lighting (EN-01): an HDR lat-long image (OpenEXR through [`crate::exr`],
//! Radiance through [`crate::hdr`]) prefiltered once on the CPU into what the viewport samples:
//!
//! - [`EnvMap::sharp`]: the image itself, at most [`SHARP_MAX`] (mirror reflections and the backdrop);
//! - [`EnvMap::specular`]: [`LEVELS`] images of equal size, the environment convolved with the GGX lobe of
//!   roughness 0, 1/(LEVELS−1), …, 1;
//! - [`EnvMap::diffuse`]: E(n)/π from nine spherical-harmonic coefficients;
//! - [`EnvMap::lights`]: the dominant directions shadows are cast from.
//!
//! Std only; prefiltering a typical 2K map takes well under a second on several cores. Files: `latlong` (the
//! image and the direction mapping), `specular`, `irradiance`, `lights`.

pub mod irradiance;
pub mod latlong;
pub mod lights;
pub mod specular;

pub use latlong::{LatLong, V3};
pub use lights::DominantLight;

use crate::tex::Tex;

/// Roughness levels of the specular prefilter.
pub const LEVELS: usize = 12;
/// Size of each specular level.
pub const SPECULAR_SIZE: (usize, usize) = (512, 256);
/// Largest size of the sharp image.
pub const SHARP_MAX: (usize, usize) = (2048, 1024);
/// Size of the diffuse image.
pub const DIFFUSE_SIZE: (usize, usize) = (64, 32);
/// Size of the image dominant lights are looked for in.
const LIGHTS_SIZE: (usize, usize) = (64, 32);

/// The GGX roughness of specular level `i`.
pub fn level_roughness(i: usize) -> f32 {
    i as f32 / (LEVELS - 1) as f32
}

/// A prefiltered environment.
#[derive(Debug, Clone, PartialEq)]
pub struct EnvMap {
    /// The environment at full detail (at most [`SHARP_MAX`]).
    pub sharp: LatLong,
    /// [`LEVELS`] images of [`SPECULAR_SIZE`].
    pub specular: Vec<LatLong>,
    pub diffuse: LatLong,
    pub lights: Vec<DominantLight>,
}

impl EnvMap {
    /// Prefilters a lat-long environment image.
    pub fn prefilter(img: &LatLong) -> EnvMap {
        let (sw, sh) = SPECULAR_SIZE;
        let base = img.resized(sw, sh);
        let chain = base.chain(4);
        let specular = (0..LEVELS).map(|i| specular::level(&chain, level_roughness(i), sw, sh)).collect();
        let small = img.resized(LIGHTS_SIZE.0, LIGHTS_SIZE.1);
        let sh9 = irradiance::Sh9::project(&small);
        let lights = lights::find(&small, &sh9);
        let sharp = if img.w > SHARP_MAX.0 || img.h > SHARP_MAX.1 { img.resized(SHARP_MAX.0, SHARP_MAX.1) } else { img.clone() };
        EnvMap { sharp, specular, diffuse: sh9.diffuse_image(DIFFUSE_SIZE.0, DIFFUSE_SIZE.1), lights }
    }

    /// Reads and prefilters an environment file's bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<EnvMap, String> {
        Ok(EnvMap::prefilter(&read_image(bytes)?))
    }
}

/// Reads an HDR environment image: OpenEXR (scanline; uncompressed, RLE, ZIPS or ZIP) or Radiance `.hdr`.
/// Grey EXR files (one `Y` channel or a single channel) give grey light.
pub fn read_image(bytes: &[u8]) -> Result<LatLong, String> {
    let (w, h, px) = if crate::hdr::sniff(bytes) {
        let t = crate::hdr::decode(bytes)?;
        let px = t.px.iter().map(|p| [p[0], p[1], p[2]]).collect();
        (t.w, t.h, px)
    } else if bytes.starts_with(&[0x76, 0x2f, 0x31, 0x01]) {
        let img = crate::exr::decode(bytes).map_err(|e| format!("{e} (save the map with ZIP or no compression)"))?;
        let grey = img.channel("Y").or_else(|| (img.channels.len() == 1).then(|| img.channels.first()).flatten());
        let pick = |name: &str| img.channel(name).or(grey).map(|c| c.data.as_slice());
        let (r, g, b) = (pick("R"), pick("G"), pick("B"));
        let (Some(r), Some(g), Some(b)) = (r, g, b) else { return Err("the EXR file has no R, G and B (or Y) channels".into()) };
        let n = img.width * img.height;
        if r.len() < n || g.len() < n || b.len() < n {
            return Err("the EXR file's channels are shorter than the image".into());
        }
        (img.width, img.height, (0..n).map(|i| [r[i], g[i], b[i]]).collect())
    } else {
        return Err("not an HDR environment (OpenEXR or Radiance .hdr)".into());
    };
    if w < 4 || h < 2 {
        return Err(format!("the environment is too small ({w} × {h})"));
    }
    LatLong::new(w, h, px).ok_or_else(|| "the environment's pixels don't match its size".to_string())
}

/// A lat-long image from a texture's RGB.
pub fn from_tex(t: &Tex) -> Option<LatLong> {
    LatLong::new(t.w, t.h, t.px.iter().map(|p| [p[0], p[1], p[2]]).collect())
}

#[cfg(test)]
mod tests {
    use super::latlong::{dot, normalize};
    use super::*;
    use crate::exr::{self, ExrChannel, ExrImage, SampleKind};

    fn sky() -> LatLong {
        let sun = normalize([0.4, 0.7, -0.5]);
        LatLong::from_fn(256, 128, |d| {
            if dot(d, sun) > 0.999 {
                [3000.0, 2800.0, 2500.0]
            } else if d[1] > 0.0 {
                [0.3, 0.45, 0.8]
            } else {
                [0.15, 0.12, 0.1]
            }
        })
    }

    #[test]
    fn a_sky_prefilters_into_levels_diffuse_light_and_its_sun() {
        let t0 = std::time::Instant::now();
        let env = EnvMap::prefilter(&sky());
        let took = t0.elapsed();
        assert_eq!(env.specular.len(), LEVELS);
        assert!(env.specular.iter().all(|l| (l.w, l.h) == SPECULAR_SIZE));
        let sun = normalize([0.4, 0.7, -0.5]);
        assert!(dot(env.lights[0].dir, sun) > 0.99);
        // facing the sun is brighter than facing the ground
        let (up, down) = (env.diffuse.sample(sun), env.diffuse.sample([0.0, -1.0, 0.0]));
        assert!(up[0] > 4.0 * down[0], "{up:?} {down:?}");
        eprintln!("prefilter of 256 × 128: {took:?}");
    }

    #[test]
    fn exr_and_hdr_files_read_and_bad_ones_are_refused() {
        let t = Tex::from_fn(16, 8, |u, v| [u * 4.0, v, 0.5, 1.0]);
        let from_hdr = read_image(&exr::encode_hdr(&t)).unwrap();
        let from_exr = read_image(&exr::encode_exr(&t, true)).unwrap();
        assert_eq!((from_hdr.w, from_hdr.h, from_exr.w, from_exr.h), (16, 8, 16, 8));
        assert!((from_exr.get(15, 0)[0] - t.px[15][0]).abs() < 1e-3);
        let grey = ExrImage { width: 4, height: 2, channels: vec![ExrChannel::new("Y", SampleKind::Half, vec![2.0; 8])] };
        assert_eq!(read_image(&exr::encode(&grey).unwrap()).unwrap().get(1, 1), [2.0; 3]);
        assert!(read_image(b"\x89PNG....").is_err());
        assert!(read_image(&[0x76, 0x2f, 0x31, 0x01, 2, 0]).is_err());
        assert!(from_tex(&t).is_some());
    }
}

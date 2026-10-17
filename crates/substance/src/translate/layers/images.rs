//! Images for bitmap fills and bitmap masks: [`ImageSource`] loads the embedded resource a bitmap source
//! names; [`channel_pixels`] brings it into a channel's encoding and [`resample`] onto the set's texels where a
//! bitmap cannot stay an image fill.

use crate::project::SppProject;
use crate::translate::resources::{read_image, split_resource_url};
use materialcraft::paint::working_space::linear_to_srgb;
use materialcraft::paint::{ChannelSpec, ColorSpace, TiledImage};
use materialcraft::projection::{ImageLevels, LevelChain, Prepared, Projection, Projector, SurfacePoint};

/// A decoded image: straight values, rows top first.
pub struct LoadedImage {
    pub image: TiledImage,
    /// Float formats hold linear values; integer formats hold encoded ones.
    pub float: bool,
}

/// Where bitmap sources come from.
pub trait ImageSource {
    /// The image a `DataBitmap.urlToBitmapRes` URL names, or why it cannot be read.
    fn load(&self, url: &str) -> Result<LoadedImage, String>;
}

/// Embedded image resources of an opened project.
pub struct ProjectImages<'a> {
    project: &'a SppProject,
}

impl<'a> ProjectImages<'a> {
    pub fn new(project: &'a SppProject) -> Self {
        ProjectImages { project }
    }
}

impl ImageSource for ProjectImages<'_> {
    fn load(&self, url: &str) -> Result<LoadedImage, String> {
        let Some((_, id)) = split_resource_url(url) else {
            return Err(format!("bitmap {url:?} is a shelf resource, not stored in the project"));
        };
        let read = read_image(self.project, id).map_err(|e| format!("bitmap {url:?}: {e}"))?;
        if read.missing_bytes > 0 {
            return Err(format!(
                "bitmap {url:?}: stored data {} bytes short",
                read.missing_bytes
            ));
        }
        Ok(LoadedImage {
            image: read.image,
            float: read.float,
        })
    }
}

/// No images at all (documents without a project, tests).
pub struct NoImages;

impl ImageSource for NoImages {
    fn load(&self, url: &str) -> Result<LoadedImage, String> {
        Err(format!("bitmap {url:?} not available"))
    }
}

/// `image` laid onto a texture set of `size` through `projection`, sampled as the compositor samples an image
/// fill, for where a bitmap cannot stay a fill (masks, layers whose channels are placed differently). Texels the
/// image does not cover get alpha 0. `None` when the projection cannot be evaluated without the mesh.
pub fn resample(image: &TiledImage, projection: &Projection, size: (u32, u32)) -> Option<TiledImage> {
    let prepared = projection.prepare(None, false).ok()?;
    let identity = prepared == Prepared::TextureSpace
        || matches!(prepared, Prepared::Uv(p) if p == materialcraft::projection::UvPlacement::identity());
    if identity && image.size() == size {
        return Some(image.clone());
    }
    let mut out = TiledImage::new(size.0, size.1, image.format().with_alpha()).ok()?;
    let chain = LevelChain::build(image);
    let levels = ImageLevels::new(image, &chain);
    let (w, h) = (size.0.max(1) as f32, size.1.max(1) as f32);
    let texel_len = 1.0 / w.max(h);
    let all = out.all_tiles();
    out.update_tiles(&all, |r, px| {
        for (i, p) in px.iter_mut().enumerate() {
            let (x, y) = (r.x as usize + i % r.w as usize, r.y as usize + i / r.w as usize);
            let point = SurfacePoint {
                uv: [(x as f32 + 0.5) / w, (y as f32 + 0.5) / h],
                texel_len,
                ..SurfacePoint::default()
            };
            let taps = prepared.project(&point);
            *p = match taps.used().first() {
                Some(t) => match levels.sample(t.uv, t.footprint, projection.filter, projection.repeat) {
                    Some(v) => [v[0], v[1], v[2], v[3] * t.weight * taps.coverage],
                    None => [0.0; 4],
                },
                None => [0.0; 4],
            };
        }
    });
    Some(out)
}

/// Layer pixels for a channel: the channel's layer format, values in its stored encoding (float images are
/// linear, so they are sRGB-encoded for sRGB channels), alpha = the image's alpha as coverage.
pub fn channel_pixels(loaded: &LoadedImage, spec: &ChannelSpec) -> TiledImage {
    let format = spec.layer_format();
    let encode = loaded.float && spec.color_space == ColorSpace::Srgb;
    if !encode {
        return loaded.image.converted(format);
    }
    let mut out = loaded.image.converted(format);
    let all = out.all_tiles();
    let source = &loaded.image;
    out.update_tiles(&all, |r, px| {
        source.read_region(r, px);
        for p in px.iter_mut() {
            for v in &mut p[..3] {
                *v = linear_to_srgb(v.clamp(0.0, 1.0));
            }
        }
    });
    out
}

/// Grey values (red × alpha) of an image, row by row.
pub fn grey_values(image: &TiledImage) -> Vec<f32> {
    let (w, h) = image.size();
    let mut out = Vec::with_capacity(w as usize * h as usize);
    for y in 0..h {
        for x in 0..w {
            let p = image.get(x, y);
            out.push((p[0] * p[3]).clamp(0.0, 1.0));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft::paint::{Channel, PixelFormat, PixelLayout, SampleType};

    fn image(w: u32, h: u32, format: PixelFormat, v: [f32; 4]) -> TiledImage {
        let mut img = TiledImage::new(w, h, format).unwrap();
        img.fill(v);
        img
    }

    #[test]
    fn resampling_keeps_matching_images_and_filters_others() {
        let f = PixelFormat::new(PixelLayout::Rgba, SampleType::U8);
        let same = resample(&image(4, 4, f, [1.0; 4]), &Projection::default(), (4, 4)).unwrap();
        assert_eq!(same.size(), (4, 4));
        assert_eq!(same.get(2, 2), [1.0; 4]);
        // 8 → 4: each texel averages a 2 × 2 block
        let mut big = image(8, 8, f, [0.0, 0.0, 0.0, 1.0]);
        big.set(0, 0, [1.0; 4]);
        let small = resample(&big, &Projection::default(), (4, 4)).unwrap();
        assert!((small.get(0, 0)[0] - 0.25).abs() < 0.01, "{:?}", small.get(0, 0));
        assert_eq!(small.get(3, 3)[0], 0.0);
        let planar = Projection {
            mode: materialcraft::projection::ProjectionMode::default_of("planar").unwrap(),
            ..Projection::default()
        };
        assert!(resample(&big, &planar, (4, 4)).is_none());
    }

    #[test]
    fn float_images_are_encoded_for_srgb_channels() {
        let f = PixelFormat::new(PixelLayout::Rgba, SampleType::F32);
        let loaded = LoadedImage {
            image: image(2, 2, f, [0.5, 0.5, 0.5, 1.0]),
            float: true,
        };
        let spec = crate::translate::channel_table::channel_spec(Channel::BaseColor, Some(0));
        let px = channel_pixels(&loaded, &spec);
        assert!((px.get(0, 0)[0] - 0.735).abs() < 0.01, "{:?}", px.get(0, 0));
        assert_eq!(grey_values(&loaded.image).len(), 4);
        assert!(NoImages.load("/x?version=1").is_err());
    }
}

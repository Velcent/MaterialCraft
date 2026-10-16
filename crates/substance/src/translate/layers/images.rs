//! Images for bitmap fills and bitmap masks: [`ImageSource`] loads the embedded resource a bitmap source
//! names; [`fit`] brings it to the set's resolution and [`channel_pixels`] into a channel's encoding.

use super::computability::Computability;
use crate::project::SppProject;
use crate::translate::resources::{read_image, split_resource_url};
use materialcraft::paint::working_space::linear_to_srgb;
use materialcraft::paint::{ChannelSpec, ColorSpace, TiledImage};

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

/// `image` at `size`: as is when it matches, else resampled (nearest texel) and reported as approximated.
pub fn fit(image: TiledImage, size: (u32, u32)) -> (TiledImage, Computability) {
    if image.size() == size || image.width() == 0 || image.height() == 0 {
        return (image, Computability::Exact);
    }
    let note = Computability::approximated(format!(
        "bitmap is {}x{}, set is {}x{}; resampled",
        image.width(),
        image.height(),
        size.0,
        size.1
    ));
    let Ok(mut out) = TiledImage::new(size.0, size.1, image.format()) else {
        return (image, note);
    };
    let (sw, sh) = (image.width() as u64, image.height() as u64);
    let all = out.all_tiles();
    out.update_tiles(&all, |r, px| {
        for (i, p) in px.iter_mut().enumerate() {
            let (x, y) = (
                r.x as u64 + (i as u64 % r.w as u64),
                r.y as u64 + (i as u64 / r.w as u64),
            );
            let sx = (x * sw / size.0 as u64).min(sw - 1) as u32;
            let sy = (y * sh / size.1 as u64).min(sh - 1) as u32;
            *p = image.get(sx, sy);
        }
    });
    (out, note)
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
    fn fitting_keeps_matching_images_and_resamples_others() {
        let f = PixelFormat::new(PixelLayout::Rgba, SampleType::U8);
        let (same, note) = fit(image(4, 4, f, [1.0; 4]), (4, 4));
        assert!(note.is_exact() && same.size() == (4, 4));
        let mut small = image(2, 2, f, [0.0, 0.0, 0.0, 1.0]);
        small.set(1, 1, [1.0; 4]);
        let (big, note) = fit(small, (4, 4));
        assert!(!note.is_exact());
        assert_eq!(big.get(3, 3), [1.0; 4]);
        assert_eq!(big.get(1, 1)[0], 0.0);
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

//! The set's surface on the mesh for the import: bitmap fills in masks hold one grey value per texel, so the importer
//! lays a bitmap on itself; in a volume mode (tri-planar, planar, spherical, cylindrical) that needs the set's surface
//! map and the scene frame. [`SurfaceSource`] hands them out (built on first use: most sets need none);
//! [`project_image`] lays an image on through the compositor, exactly as it lays on an image fill.

use materialcraft::mesh::{Mesh, SurfaceKey, SurfaceMap};
use materialcraft::paint::{
    BlendMode, Channel, ChannelSpec, ColorSpace, Compositor, Fill, FillContext, FillSource, ImageFill, Layer,
    LayerKind, NoFillSources, PixelFormat, PixelLayout, SampleType, TextureSet, TiledImage,
};
use materialcraft::projection::{Projection, SceneFrame};
use std::sync::OnceLock;

/// A set's surface map (at the set's resolution) and the frame of the whole scene.
pub trait SurfaceSource {
    fn surface(&self) -> Option<(&SurfaceMap, Option<SceneFrame>)>;
}

/// The surface of the parts on one material slot, built the first time it is asked for.
pub struct LazySurface<'m> {
    mesh: &'m Mesh,
    key: SurfaceKey,
    built: OnceLock<Option<(SurfaceMap, Option<SceneFrame>)>>,
}

impl<'m> LazySurface<'m> {
    /// The surface of the parts of `mesh` on `material` in UV set `uv_set` at `size`, without gutter.
    pub fn new(mesh: &'m Mesh, material: usize, uv_set: usize, size: (u32, u32)) -> LazySurface<'m> {
        let key = SurfaceKey {
            material,
            uv_set,
            width: size.0,
            height: size.1,
            gutter: 0,
            mesh_revision: 0,
        };
        LazySurface {
            mesh,
            key,
            built: OnceLock::new(),
        }
    }
}

impl SurfaceSource for LazySurface<'_> {
    fn surface(&self) -> Option<(&SurfaceMap, Option<SceneFrame>)> {
        let built = self.built.get_or_init(|| {
            let map = SurfaceMap::build(self.mesh, self.key).ok()?;
            Some((map, SceneFrame::of_mesh(self.mesh)))
        });
        built.as_ref().map(|(m, f)| (m, *f))
    }
}

/// `image` laid on the surface by `projection` at the surface map's size, as the compositor lays on an image fill:
/// values in the image's encoding, alpha = coverage. `None` if the compositor cannot evaluate the projection.
pub fn project_image(
    image: &TiledImage,
    projection: &Projection,
    (map, frame): (&SurfaceMap, Option<SceneFrame>),
) -> Option<TiledImage> {
    // a linear float RGB channel, so the values pass unchanged
    let channel = Channel::Roughness;
    let mut fill = ImageFill::default();
    fill.set_image(channel, image.clone());
    let fill = Fill::new(FillSource::Image(fill)).with_projection(projection.clone());
    let size = (map.width() as u32, map.height() as u32);
    let mut set = TextureSet::new("mask", "mask", size);
    set.channels.push(ChannelSpec {
        channel,
        format: PixelFormat::new(PixelLayout::Rgb, SampleType::F32),
        color_space: ColorSpace::Linear,
    });
    set.layers
        .push(Layer::new("bitmap", "bitmap", LayerKind::Fill(fill)).with_blend(channel, BlendMode::Normal, 1.0));
    let context = FillContext::with_sources(&NoFillSources).with_surface(map, frame);
    let compositor = Compositor::new(&set).with_fills(&context);
    if !compositor.warnings(channel).is_empty() {
        return None;
    }
    compositor.composite_image(channel).ok().map(|(img, _)| img)
}

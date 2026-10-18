//! GPU textures: one material's map set (base, normal, rma, emissive, height), mip-mapped with anisotropic filtering.
use super::maps::MapsPatch;
use super::{GpuMaps, bytes_of};
use eframe::egui_glow::glow;
use glow::HasContext;

const ANISO_PARAM: u32 = 0x84FE;
const ANISO_MAX: u32 = 0x84FF;

// SAFETY (whole file): GL calls on the current context (module invariant); uploaded slices are sized by their
// callers for w * h texels of the given format, and textures are freed only by their owner.
/// Highest anisotropic filtering level the driver offers (1 when the extension is missing).
pub(super) unsafe fn max_aniso(gl: &glow::Context) -> f32 {
    unsafe {
        let ext = gl.supported_extensions();
        let has_aniso = ext.contains("GL_EXT_texture_filter_anisotropic") || ext.contains("GL_ARB_texture_filter_anisotropic");
        if has_aniso { gl.get_parameter_f32(ANISO_MAX).max(1.0) } else { 1.0 }
    }
}

/// The requested level (0 = maximum) limited to what the GPU offers.
pub(super) fn aniso_level(req: f32, max: f32) -> f32 {
    if req <= 0.0 { max } else { req.min(max) }
}

pub(super) struct Tx {
    pub id: glow::Texture,
    pub w: usize,
    pub h: usize,
}

impl Tx {
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn new(gl: &glow::Context, internal: u32, fmt: u32, ty: u32, w: usize, h: usize, data: &[u8], aniso: f32) -> Result<Tx, String> {
        unsafe {
            let id = gl.create_texture()?;
            gl.bind_texture(glow::TEXTURE_2D, Some(id));
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            gl.tex_image_2d(glow::TEXTURE_2D, 0, internal as i32, w as i32, h as i32, 0, fmt, ty, glow::PixelUnpackData::Slice(Some(data)));
            gl.generate_mipmap(glow::TEXTURE_2D);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::REPEAT as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::REPEAT as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR_MIPMAP_LINEAR as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
            if aniso > 1.0 {
                gl.tex_parameter_f32(glow::TEXTURE_2D, ANISO_PARAM, aniso);
            }
            Ok(Tx { id, w, h })
        }
    }

    /// Replaces the texels of the rectangle in the top level (`data` holds `w × h` texels of the texture's format);
    /// the mip levels are rebuilt by [`Tx::rebuild_mips`]. False (and nothing written) for a rectangle outside the
    /// texture.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn write(&self, gl: &glow::Context, fmt: u32, ty: u32, x: usize, y: usize, w: usize, h: usize, data: &[u8]) -> bool {
        if x + w > self.w || y + h > self.h || w == 0 || h == 0 {
            return false;
        }
        unsafe {
            gl.bind_texture(glow::TEXTURE_2D, Some(self.id));
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            gl.tex_sub_image_2d(glow::TEXTURE_2D, 0, x as i32, y as i32, w as i32, h as i32, fmt, ty, glow::PixelUnpackData::Slice(Some(data)));
        }
        true
    }

    /// Derives the mip levels from the top level again.
    pub unsafe fn rebuild_mips(&self, gl: &glow::Context) {
        unsafe {
            gl.bind_texture(glow::TEXTURE_2D, Some(self.id));
            gl.generate_mipmap(glow::TEXTURE_2D);
        }
    }

    /// 1x1 zero height, bound when a material has no height map.
    pub unsafe fn flat_height(gl: &glow::Context) -> Result<Tx, String> {
        unsafe { Tx::new(gl, glow::R16F, glow::RED, glow::FLOAT, 1, 1, bytes_of(&[0.0f32]), 1.0) }
    }

    pub unsafe fn free(&self, gl: &glow::Context) {
        unsafe {
            gl.delete_texture(self.id);
        }
    }
}

/// The uploaded maps of one material.
pub(super) struct MapTextures {
    pub base: Tx,
    pub normal: Tx,
    pub rma: Tx,
    pub emis: Option<Tx>,
    pub height: Option<Tx>,
}

impl MapTextures {
    pub unsafe fn upload(gl: &glow::Context, maps: &GpuMaps, aniso: f32) -> Result<MapTextures, String> {
        unsafe {
            let s = maps.size;
            let base = Tx::new(gl, glow::SRGB8_ALPHA8, glow::RGBA, glow::UNSIGNED_BYTE, s, s, &maps.base, aniso)?;
            let normal = Tx::new(gl, glow::RGBA8, glow::RGBA, glow::UNSIGNED_BYTE, s, s, &maps.normal, aniso)?;
            let rma = Tx::new(gl, glow::RGBA8, glow::RGBA, glow::UNSIGNED_BYTE, s, s, &maps.rma, aniso)?;
            let emis = match &maps.emissive {
                Some(e) => Some(Tx::new(gl, glow::SRGB8_ALPHA8, glow::RGBA, glow::UNSIGNED_BYTE, s, s, e, aniso)?),
                None => None,
            };
            let height = match &maps.height {
                Some((w, h, v)) => Some(Tx::new(gl, glow::R16F, glow::RED, glow::FLOAT, *w, *h, bytes_of(v), aniso)?),
                None => None,
            };
            Ok(MapTextures { base, normal, rma, emis, height })
        }
    }

    /// Writes a patch's rectangles into the maps and rebuilds the mip levels of the maps it wrote (once each).
    /// Rectangles outside a map and maps the set lacks are left alone.
    pub unsafe fn patch(&self, gl: &glow::Context, p: &MapsPatch) {
        let rgba = (glow::RGBA, glow::UNSIGNED_BYTE);
        // base, normal, rma, emissive, height
        let mut wrote = [false; 5];
        unsafe {
            for t in &p.tiles {
                let (x, y, w, h) = (t.x, t.y, t.w, t.h);
                let n = w * h;
                if t.base.len() == n * 4 {
                    wrote[0] |= self.base.write(gl, rgba.0, rgba.1, x, y, w, h, &t.base);
                }
                if t.normal.len() == n * 4 {
                    wrote[1] |= self.normal.write(gl, rgba.0, rgba.1, x, y, w, h, &t.normal);
                }
                if t.rma.len() == n * 4 {
                    wrote[2] |= self.rma.write(gl, rgba.0, rgba.1, x, y, w, h, &t.rma);
                }
                if let (Some(tx), Some(e)) = (&self.emis, &t.emissive)
                    && e.len() == n * 4
                {
                    wrote[3] |= tx.write(gl, rgba.0, rgba.1, x, y, w, h, e);
                }
                if let (Some(tx), Some(v)) = (&self.height, &t.height)
                    && v.len() == n
                {
                    wrote[4] |= tx.write(gl, glow::RED, glow::FLOAT, x, y, w, h, bytes_of(v));
                }
            }
            let maps = [Some(&self.base), Some(&self.normal), Some(&self.rma), self.emis.as_ref(), self.height.as_ref()];
            for (tx, w) in maps.into_iter().zip(wrote) {
                if let (Some(tx), true) = (tx, w) {
                    tx.rebuild_mips(gl);
                }
            }
            gl.bind_texture(glow::TEXTURE_2D, None);
        }
    }

    /// Changes the anisotropic filtering level of every map (1 = off).
    pub unsafe fn set_aniso(&self, gl: &glow::Context, aniso: f32) {
        unsafe {
            for t in [Some(&self.base), Some(&self.normal), Some(&self.rma), self.emis.as_ref(), self.height.as_ref()].into_iter().flatten() {
                gl.bind_texture(glow::TEXTURE_2D, Some(t.id));
                gl.tex_parameter_f32(glow::TEXTURE_2D, ANISO_PARAM, aniso.max(1.0));
            }
            gl.bind_texture(glow::TEXTURE_2D, None);
        }
    }

    /// Size of the height map in texels, (1, 1) without one.
    pub fn height_size(&self) -> (usize, usize) {
        self.height.as_ref().map(|t| (t.w, t.h)).unwrap_or((1, 1))
    }

    pub unsafe fn free(&self, gl: &glow::Context) {
        unsafe {
            for t in [Some(&self.base), Some(&self.normal), Some(&self.rma), self.emis.as_ref(), self.height.as_ref()].into_iter().flatten() {
                t.free(gl);
            }
        }
    }
}

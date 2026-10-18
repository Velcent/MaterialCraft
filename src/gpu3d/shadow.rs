//! Scene shadows (EN-07, EN-08): depth maps drawn from the environment's dominant directions with the material's
//! own vertex stage (so displaced surfaces cast what they show), and the choice of direction per accumulation
//! sample. Sample 0 uses the light's centre and is kept (the "plain" map) for as long as the light and the geometry
//! stay the same, so orbiting a world-fixed light or painting costs no extra pass; later samples spread over the
//! light's size and land in a second map, so the accumulated picture shows soft shadows.

use super::camera::CamOut;
use super::env::{Lighting, ShadowLight};
use super::lighting::ShadowQuality;
use super::pass::{Drawable, MaterialPass, Shade};
use super::shaders::Program;
use crate::camera::{Camera, Lens, lens_point};
use crate::render3d::View;
use eframe::egui_glow::glow;
use glow::HasContext;

/// Size of a shadow map in texels.
const SIZE: i32 = 2048;
/// The texture unit the shadow map is bound to.
const UNIT: u32 = 8;

/// What a kept shadow map was drawn for.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ShadowKey {
    pub dir: [u32; 3],
    /// The geometry: its generation and, with displacement, the content and height settings it was displaced with.
    pub geometry: (u64, u32, u32),
    pub displacement: Option<(u64, u32, u32)>,
}

impl ShadowKey {
    pub fn new(dir: [f32; 3], view: &View, geometry: u64, content: u64) -> ShadowKey {
        let displaced = view.relief == crate::render3d::RELIEF_DISPLACE && view.height_scale != 0.0;
        ShadowKey {
            dir: dir.map(f32::to_bits),
            geometry: (geometry, view.shape, view.tess),
            displacement: displaced.then(|| (content, view.height_scale.to_bits(), view.height_level.to_bits())),
        }
    }
}

/// One depth map and the light camera it was drawn with.
struct DepthMap {
    fbo: glow::Framebuffer,
    depth: glow::Texture,
    vp: crate::camera::Mat4,
    /// World size of one texel.
    texel: f32,
}

// SAFETY (whole file): GL calls on the current context (module invariant); the maps are created and freed by
// `ShadowMaps`, which owns them.
impl DepthMap {
    unsafe fn new(gl: &glow::Context) -> Result<DepthMap, String> {
        unsafe {
            let depth = gl.create_texture()?;
            gl.bind_texture(glow::TEXTURE_2D, Some(depth));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::DEPTH_COMPONENT24 as i32,
                SIZE,
                SIZE,
                0,
                glow::DEPTH_COMPONENT,
                glow::UNSIGNED_INT,
                glow::PixelUnpackData::Slice(None),
            );
            for (k, v) in [
                (glow::TEXTURE_MIN_FILTER, glow::LINEAR),
                (glow::TEXTURE_MAG_FILTER, glow::LINEAR),
                (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
                (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
                (glow::TEXTURE_COMPARE_MODE, glow::COMPARE_REF_TO_TEXTURE),
                (glow::TEXTURE_COMPARE_FUNC, glow::LEQUAL),
            ] {
                gl.tex_parameter_i32(glow::TEXTURE_2D, k, v as i32);
            }
            gl.bind_texture(glow::TEXTURE_2D, None);
            let fbo = match gl.create_framebuffer() {
                Ok(f) => f,
                Err(e) => {
                    gl.delete_texture(depth);
                    return Err(e);
                }
            };
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
            gl.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::DEPTH_ATTACHMENT, glow::TEXTURE_2D, Some(depth), 0);
            gl.draw_buffer(glow::NONE);
            gl.read_buffer(glow::NONE);
            let status = gl.check_framebuffer_status(glow::FRAMEBUFFER);
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            let map = DepthMap { fbo, depth, vp: crate::camera::IDENTITY, texel: 0.0 };
            if status != glow::FRAMEBUFFER_COMPLETE {
                map.free(gl);
                return Err(format!("the shadow framebuffer is incomplete (0x{status:x})"));
            }
            Ok(map)
        }
    }

    /// Draws `items` as seen from direction `dir`.
    unsafe fn draw(&mut self, gl: &glow::Context, prog: &Program, view: &View, items: &[Drawable], dir: [f32; 3], flat_height: glow::Texture) {
        unsafe {
            let (centre, radius) = bounds(items, view);
            let r = radius.max(1e-3);
            let eye = [centre[0] + dir[0] * 2.0 * r, centre[1] + dir[1] * 2.0 * r, centre[2] + dir[2] * 2.0 * r];
            let lens = Lens::Orthographic { height: 2.0 * r, near: 0.5 * r, far: 3.5 * r };
            let camera = Camera::looking_at(eye, centre, [0.0, 1.0, 0.0], &lens, [1.0, 1.0]);
            let cam = CamOut::of(&camera);
            self.vp = cam.vp;
            self.texel = 2.0 * r / SIZE as f32;
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo));
            gl.viewport(0, 0, SIZE, SIZE);
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LESS);
            gl.depth_mask(true);
            gl.clear_depth_f32(1.0);
            gl.clear(glow::DEPTH_BUFFER_BIT);
            gl.enable(glow::POLYGON_OFFSET_FILL);
            gl.polygon_offset(1.5, 4.0);
            let shade = Shade { self_shadow: false, unlit: false, mip_bias: 0.0, uv_space: false };
            let pass = MaterialPass::begin(gl, prog, view, &cam, &shade, flat_height);
            pass.draw(items);
            gl.disable(glow::POLYGON_OFFSET_FILL);
            gl.disable(glow::DEPTH_TEST);
        }
    }

    unsafe fn free(&self, gl: &glow::Context) {
        unsafe {
            gl.delete_framebuffer(self.fbo);
            gl.delete_texture(self.depth);
        }
    }
}

/// A sphere around everything drawn (displacement can push surfaces out by the height scale).
fn bounds(items: &[Drawable], view: &View) -> ([f32; 3], f32) {
    if items.is_empty() {
        return ([0.0; 3], 1.0);
    }
    let n = items.len() as f32;
    let mut c = [0.0f32; 3];
    for d in items {
        for (k, ck) in c.iter_mut().enumerate() {
            *ck += d.mesh.bounds.0[k] / n;
        }
    }
    let r = items
        .iter()
        .map(|d| {
            let o = d.mesh.bounds.0;
            ((o[0] - c[0]).powi(2) + (o[1] - c[1]).powi(2) + (o[2] - c[2]).powi(2)).sqrt() + d.mesh.bounds.1
        })
        .fold(0.0f32, f32::max);
    let disp = if view.relief == crate::render3d::RELIEF_DISPLACE { view.height_scale.abs() } else { 0.0 };
    (c, r + disp)
}

/// The direction of accumulation sample `index` of `frames`: the brightest light's centre for 0, otherwise a light
/// picked by weight and a point spread over its size.
pub(super) fn sample_direction(lights: &[ShadowLight], index: u32, frames: u32) -> Option<[f32; 3]> {
    let first = lights.first()?;
    if index == 0 {
        return Some(first.dir);
    }
    let total: f32 = lights.iter().map(|l| l.weight).sum();
    let pick = (index as f32 * 0.618_034).fract() * total;
    let mut acc = 0.0;
    let light = lights.iter().find(|l| {
        acc += l.weight;
        pick < acc
    });
    let l = light.unwrap_or(first);
    let p = lens_point(index, frames.max(2));
    let d = l.dir;
    // two axes across the direction
    let a = if d[1].abs() < 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
    let t1 = crate::v3::norm(crate::v3::cross(d, a));
    let t2 = crate::v3::cross(d, t1);
    let s = l.spread.clamp(0.0, 1.2).tan();
    let out = [0, 1, 2].map(|k| d[k] + (t1[k] * p[0] + t2[k] * p[1]) * s);
    Some(crate::v3::norm(out))
}

/// The shadow maps and their program.
pub(super) struct ShadowMaps {
    prog: Program,
    plain: DepthMap,
    sample: DepthMap,
    plain_key: Option<ShadowKey>,
}

/// The map a frame samples, as the material program needs it.
pub(super) struct Bound {
    vp: crate::camera::Mat4,
    texel: f32,
    depth: glow::Texture,
}

/// What a draw needs from the frame to pick its shadow.
pub(super) struct Want<'a> {
    pub view: &'a View,
    pub lighting: &'a Lighting,
    pub items: &'a [Drawable<'a>],
    pub index: u32,
    pub frames: u32,
    pub quality: ShadowQuality,
    pub geometry: u64,
    pub content: u64,
    pub flat_height: glow::Texture,
}

impl ShadowMaps {
    pub unsafe fn new(gl: &glow::Context) -> Result<ShadowMaps, String> {
        unsafe {
            let prog = Program::shadow(gl)?;
            let plain = match DepthMap::new(gl) {
                Ok(m) => m,
                Err(e) => {
                    prog.free(gl);
                    return Err(e);
                }
            };
            let sample = match DepthMap::new(gl) {
                Ok(m) => m,
                Err(e) => {
                    prog.free(gl);
                    plain.free(gl);
                    return Err(e);
                }
            };
            Ok(ShadowMaps { prog, plain, sample, plain_key: None })
        }
    }

    /// Draws the map sample `w.index` needs, if any, and returns it; `None` means the frame goes without shadows
    /// (no light, or a moving view in smooth mode whose kept map no longer fits). Leaves the framebuffer unbound.
    pub unsafe fn prepare(&mut self, gl: &glow::Context, w: &Want) -> Option<Bound> {
        unsafe {
            let centre = w.lighting.shadow_lights.first()?.dir;
            let key = ShadowKey::new(centre, w.view, w.geometry, w.content);
            let kept = self.plain_key.as_ref() == Some(&key);
            let map = if w.index == 0 || !kept {
                if !kept {
                    if w.index == 0 && !w.quality.while_moving() {
                        return None;
                    }
                    self.plain.draw(gl, &self.prog, w.view, w.items, centre, w.flat_height);
                    self.plain_key = Some(key);
                }
                &self.plain
            } else {
                let dir = sample_direction(&w.lighting.shadow_lights, w.index, w.frames)?;
                self.sample.draw(gl, &self.prog, w.view, w.items, dir, w.flat_height);
                &self.sample
            };
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            Some(Bound { vp: map.vp, texel: map.texel, depth: map.depth })
        }
    }

    /// Forgets the kept map (the geometry it showed is gone).
    pub fn invalidate(&mut self) {
        self.plain_key = None;
    }

    pub unsafe fn free(&self, gl: &glow::Context) {
        unsafe {
            self.prog.free(gl);
            self.plain.free(gl);
            self.sample.free(gl);
        }
    }
}

impl Bound {
    /// Sets the shadow uniforms of the material program for `mode` (1 key light, 2 environment share) at `opacity`.
    pub unsafe fn bind(&self, gl: &glow::Context, prog: &Program, mode: i32, opacity: f32) {
        unsafe {
            gl.active_texture(glow::TEXTURE0 + UNIT);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.depth));
            gl.uniform_1_i32(prog.loc("uShadowMap"), UNIT as i32);
            gl.uniform_matrix_4_f32_slice(prog.loc("uShadowVP"), false, &self.vp);
            gl.uniform_1_f32(prog.loc("uShadowOffset"), self.texel * 2.0);
            gl.uniform_1_f32(prog.loc("uShadowOpacity"), opacity);
            gl.uniform_1_i32(prog.loc("uShadowMode"), mode);
        }
    }

    /// Turns shadows off in the material program (the sampler still gets a unit of its own type).
    pub unsafe fn off(gl: &glow::Context, prog: &Program) {
        unsafe {
            gl.uniform_1_i32(prog.loc("uShadowMode"), 0);
            gl.uniform_1_i32(prog.loc("uShadowMap"), UNIT as i32);
        }
    }

    pub unsafe fn unbind(gl: &glow::Context) {
        unsafe {
            gl.active_texture(glow::TEXTURE0 + UNIT);
            gl.bind_texture(glow::TEXTURE_2D, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_start_at_the_centre_and_spread_over_the_lights() {
        let lights = [ShadowLight { dir: [0.0, 1.0, 0.0], weight: 3.0, spread: 0.1 }, ShadowLight { dir: [1.0, 0.0, 0.0], weight: 1.0, spread: 0.0 }];
        assert_eq!(sample_direction(&lights, 0, 32), Some([0.0, 1.0, 0.0]));
        let dirs: Vec<[f32; 3]> = (1..33).filter_map(|i| sample_direction(&lights, i, 32)).collect();
        let near_first = dirs.iter().filter(|d| d[1] > 0.9).count();
        let second = dirs.iter().filter(|d| d[0] > 0.99).count();
        assert_eq!(near_first + second, 32);
        assert!(near_first > 2 * second && second > 4, "{near_first} {second}");
        // the first light's samples stay within its spread
        assert!(dirs.iter().filter(|d| d[1] > 0.9).all(|d| d[1] >= 0.1f32.cos() - 1e-3));
        assert!(sample_direction(&[], 3, 8).is_none());
        // the kept map's key ignores content unless the surface is displaced
        let v = View { relief: crate::render3d::RELIEF_NONE, ..View::default() };
        assert_eq!(ShadowKey::new([0.0, 1.0, 0.0], &v, 1, 5), ShadowKey::new([0.0, 1.0, 0.0], &v, 1, 6));
        let d = View { relief: crate::render3d::RELIEF_DISPLACE, ..View::default() };
        assert_ne!(ShadowKey::new([0.0, 1.0, 0.0], &d, 1, 5), ShadowKey::new([0.0, 1.0, 0.0], &d, 1, 6));
    }
}

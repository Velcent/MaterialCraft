//! The environment on the GPU (EN-01, EN-09): its sharp image, its prefiltered levels as one array texture and its
//! diffuse image,
//! either baked from the built-in studio (once, on the GPU, from `glsl/studio.glsl`) or uploaded from a
//! prefiltered [`EnvMap`]. [`Lighting`] is what one frame lights with: where the environment points, the studio's
//! direct lights, and the directions shadows come from.

use super::shaders::Program;
use crate::envmap::{DominantLight, EnvMap, LEVELS, SPECULAR_SIZE, level_roughness};
use crate::render3d::{View, light_dirs};
use eframe::egui_glow::glow;
use glow::HasContext;

/// Texture units of the environment (after the five maps and the wireframe's depth).
const SPEC_UNIT: u32 = 6;
const DIFF_UNIT: u32 = 7;
const SHARP_UNIT: u32 = 9;
/// Size of the studio's sharp image.
const STUDIO_SHARP: (i32, i32) = (1024, 512);
/// Size of the studio's diffuse image (a smooth gradient).
const STUDIO_DIFFUSE: (i32, i32) = (512, 256);
/// Angular radius of the studio key light for soft shadows, radians.
const STUDIO_KEY_SPREAD: f32 = 0.08;

/// Where the environment's light comes from.
pub(super) enum Source {
    /// The built-in studio: its key and fill are direct lights, the key casts the shadows.
    Studio,
    /// An image: all light comes from the environment; its dominant lights (in the image's frame) cast shadows.
    Image(Vec<DominantLight>),
}

pub(super) struct EnvTextures {
    sharp: glow::Texture,
    specular: glow::Texture,
    diffuse: glow::Texture,
    source: Source,
}

// SAFETY (whole file): GL calls on the current context (module invariant); textures and framebuffers are created
// and freed here, uploads come from slices sized for the textures they fill.
unsafe fn array_texture(gl: &glow::Context, data: Option<&[u8]>) -> Result<glow::Texture, String> {
    unsafe {
        let (w, h) = (SPECULAR_SIZE.0 as i32, SPECULAR_SIZE.1 as i32);
        let t = gl.create_texture()?;
        gl.bind_texture(glow::TEXTURE_2D_ARRAY, Some(t));
        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
        gl.tex_image_3d(glow::TEXTURE_2D_ARRAY, 0, glow::RGB16F as i32, w, h, LEVELS as i32, 0, glow::RGB, glow::FLOAT, glow::PixelUnpackData::Slice(data));
        filters(gl, glow::TEXTURE_2D_ARRAY);
        gl.bind_texture(glow::TEXTURE_2D_ARRAY, None);
        Ok(t)
    }
}

unsafe fn flat_texture(gl: &glow::Context, w: i32, h: i32, data: Option<&[u8]>) -> Result<glow::Texture, String> {
    unsafe {
        let t = gl.create_texture()?;
        gl.bind_texture(glow::TEXTURE_2D, Some(t));
        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
        gl.tex_image_2d(glow::TEXTURE_2D, 0, glow::RGB16F as i32, w, h, 0, glow::RGB, glow::FLOAT, glow::PixelUnpackData::Slice(data));
        filters(gl, glow::TEXTURE_2D);
        gl.bind_texture(glow::TEXTURE_2D, None);
        Ok(t)
    }
}

/// Linear filtering without mip-maps; around horizontally, clamped at the poles.
unsafe fn filters(gl: &glow::Context, target: u32) {
    unsafe {
        gl.tex_parameter_i32(target, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
        gl.tex_parameter_i32(target, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
        gl.tex_parameter_i32(target, glow::TEXTURE_WRAP_S, glow::REPEAT as i32);
        gl.tex_parameter_i32(target, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
    }
}

fn rgb_bytes(px: &[[f32; 3]]) -> Vec<u8> {
    px.iter().flat_map(|p| p.iter().flat_map(|v| v.to_ne_bytes())).collect()
}

impl EnvTextures {
    /// The built-in studio, baked by `bake` (the studio bake program); `vao` is an empty vertex array.
    pub unsafe fn studio(gl: &glow::Context, bake: &Program, vao: glow::VertexArray) -> Result<EnvTextures, String> {
        unsafe {
            let specular = array_texture(gl, None)?;
            let diffuse = match flat_texture(gl, STUDIO_DIFFUSE.0, STUDIO_DIFFUSE.1, None) {
                Ok(d) => d,
                Err(e) => {
                    gl.delete_texture(specular);
                    return Err(e);
                }
            };
            let sharp = match flat_texture(gl, STUDIO_SHARP.0, STUDIO_SHARP.1, None) {
                Ok(d) => d,
                Err(e) => {
                    gl.delete_texture(specular);
                    gl.delete_texture(diffuse);
                    return Err(e);
                }
            };
            let env = EnvTextures { sharp, specular, diffuse, source: Source::Studio };
            if let Err(e) = env.bake_studio(gl, bake, vao) {
                env.free(gl);
                return Err(e);
            }
            Ok(env)
        }
    }

    unsafe fn bake_studio(&self, gl: &glow::Context, bake: &Program, vao: glow::VertexArray) -> Result<(), String> {
        unsafe {
            let fbo = gl.create_framebuffer()?;
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
            gl.disable(glow::SCISSOR_TEST);
            gl.disable(glow::BLEND);
            gl.disable(glow::DEPTH_TEST);
            gl.use_program(Some(bake.id));
            gl.bind_vertex_array(Some(vao));
            let mut result = Ok(());
            // the levels, then the diffuse image, then the sharp image
            for level in 0..LEVELS + 2 {
                let (w, h) = if level < LEVELS {
                    gl.framebuffer_texture_layer(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, Some(self.specular), 0, level as i32);
                    (SPECULAR_SIZE.0 as i32, SPECULAR_SIZE.1 as i32)
                } else if level == LEVELS {
                    gl.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, glow::TEXTURE_2D, Some(self.diffuse), 0);
                    STUDIO_DIFFUSE
                } else {
                    gl.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, glow::TEXTURE_2D, Some(self.sharp), 0);
                    STUDIO_SHARP
                };
                if gl.check_framebuffer_status(glow::FRAMEBUFFER) != glow::FRAMEBUFFER_COMPLETE {
                    result = Err("the studio environment can't be drawn (incomplete framebuffer)".to_string());
                    break;
                }
                gl.viewport(0, 0, w, h);
                gl.uniform_2_f32(bake.loc("uSize"), w as f32, h as f32);
                gl.uniform_1_f32(bake.loc("uRough"), level_roughness(level.min(LEVELS - 1)));
                gl.uniform_1_i32(bake.loc("uDiffuse"), (level == LEVELS) as i32);
                if level > LEVELS {
                    gl.uniform_1_f32(bake.loc("uRough"), 0.0);
                }
                gl.draw_arrays(glow::TRIANGLES, 0, 3);
            }
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            gl.delete_framebuffer(fbo);
            result
        }
    }

    /// A prefiltered environment image.
    pub unsafe fn image(gl: &glow::Context, env: &EnvMap) -> Result<EnvTextures, String> {
        unsafe {
            let n = SPECULAR_SIZE.0 * SPECULAR_SIZE.1;
            if env.specular.len() != LEVELS || env.specular.iter().any(|l| l.px.len() != n) {
                return Err("the environment's levels have the wrong size".into());
            }
            let levels: Vec<[f32; 3]> = env.specular.iter().flat_map(|l| l.px.iter().copied()).collect();
            let specular = array_texture(gl, Some(&rgb_bytes(&levels)))?;
            let d = &env.diffuse;
            if d.px.len() != d.w * d.h {
                gl.delete_texture(specular);
                return Err("the environment's diffuse image has the wrong size".into());
            }
            let diffuse = match flat_texture(gl, d.w as i32, d.h as i32, Some(&rgb_bytes(&d.px))) {
                Ok(t) => t,
                Err(e) => {
                    gl.delete_texture(specular);
                    return Err(e);
                }
            };
            let s = &env.sharp;
            let sharp = if s.px.len() == s.w * s.h {
                flat_texture(gl, s.w as i32, s.h as i32, Some(&rgb_bytes(&s.px)))
            } else {
                Err("the sharp image has the wrong size".into())
            };
            let sharp = match sharp {
                Ok(t) => t,
                Err(e) => {
                    gl.delete_texture(specular);
                    gl.delete_texture(diffuse);
                    return Err(e);
                }
            };
            Ok(EnvTextures { sharp, specular, diffuse, source: Source::Image(env.lights.clone()) })
        }
    }

    pub fn source(&self) -> &Source {
        &self.source
    }

    /// Binds the textures and sets the lookup uniforms of `prog` (the material or the background program).
    pub unsafe fn bind(&self, gl: &glow::Context, prog: &Program, lighting: &Lighting) {
        unsafe {
            gl.active_texture(glow::TEXTURE0 + SPEC_UNIT);
            gl.bind_texture(glow::TEXTURE_2D_ARRAY, Some(self.specular));
            gl.uniform_1_i32(prog.loc("uEnvSpec"), SPEC_UNIT as i32);
            gl.active_texture(glow::TEXTURE0 + DIFF_UNIT);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.diffuse));
            gl.uniform_1_i32(prog.loc("uEnvDiff"), DIFF_UNIT as i32);
            gl.active_texture(glow::TEXTURE0 + SHARP_UNIT);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.sharp));
            gl.uniform_1_i32(prog.loc("uEnvSharp"), SHARP_UNIT as i32);
            gl.uniform_1_f32(prog.loc("uEnvLevels"), LEVELS as f32);
            gl.uniform_2_f32(prog.loc("uEnvRot"), lighting.turn[0], lighting.turn[1]);
        }
    }

    /// Unbinds what `bind` bound.
    pub unsafe fn unbind(gl: &glow::Context) {
        unsafe {
            gl.active_texture(glow::TEXTURE0 + SPEC_UNIT);
            gl.bind_texture(glow::TEXTURE_2D_ARRAY, None);
            gl.active_texture(glow::TEXTURE0 + DIFF_UNIT);
            gl.bind_texture(glow::TEXTURE_2D, None);
            gl.active_texture(glow::TEXTURE0 + SHARP_UNIT);
            gl.bind_texture(glow::TEXTURE_2D, None);
        }
    }

    pub unsafe fn free(&self, gl: &glow::Context) {
        unsafe {
            gl.delete_texture(self.sharp);
            gl.delete_texture(self.specular);
            gl.delete_texture(self.diffuse);
        }
    }
}

/// A direction shadows are cast from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ShadowLight {
    /// Unit direction towards the light, world space.
    pub dir: [f32; 3],
    /// Relative weight (how often accumulation samples pick it).
    pub weight: f32,
    /// Angular radius (radians) samples spread over.
    pub spread: f32,
}

/// What one frame lights with.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Lighting {
    /// cos, sin of the environment's turn about +Y.
    pub turn: [f32; 2],
    /// The key light direction (also the height map's contact shadows).
    pub key: [f32; 3],
    pub fill: [f32; 3],
    /// Strength of the direct lights (0 for image environments).
    pub light_intensity: f32,
    pub ambient: f32,
    /// The studio's key softbox is added to reflections.
    pub studio_key: bool,
    /// Image environments: the dominant lights in world space and the irradiance they give.
    pub dominant: Vec<([f32; 3], [f32; 3])>,
    /// Shadow directions, brightest first; empty: nothing casts shadows.
    pub shadow_lights: Vec<ShadowLight>,
    /// 1: shadows darken the key light, 2: they darken the environment's dominant share.
    pub shadow_mode: i32,
}

/// `d` turned by angle (cos, sin) about +Y.
fn turn(d: [f32; 3], t: [f32; 2]) -> [f32; 3] {
    [d[0] * t[0] + d[2] * t[1], d[1], -d[0] * t[1] + d[2] * t[0]]
}

impl Lighting {
    pub fn of(view: &View, source: &Source) -> Lighting {
        let (key, fill) = light_dirs(view);
        match source {
            Source::Studio => {
                // the studio is baked with its key towards +Z: turn it to the key's azimuth
                let a = key[0].atan2(key[2]);
                Lighting {
                    turn: [a.cos(), a.sin()],
                    key,
                    fill,
                    light_intensity: view.light_intensity,
                    ambient: view.ambient,
                    studio_key: true,
                    dominant: Vec::new(),
                    shadow_lights: vec![ShadowLight { dir: key, weight: 1.0, spread: STUDIO_KEY_SPREAD }],
                    shadow_mode: 1,
                }
            }
            Source::Image(lights) => {
                let a = env_turn(view);
                let t = [a.cos(), a.sin()];
                let dominant: Vec<([f32; 3], [f32; 3])> = lights.iter().take(4).map(|l| (turn(l.dir, t), l.irradiance)).collect();
                let shadow_lights: Vec<ShadowLight> = lights
                    .iter()
                    .take(4)
                    .map(|l| ShadowLight { dir: turn(l.dir, t), weight: crate::envmap::latlong::luminance(l.irradiance).max(0.0), spread: l.spread })
                    .filter(|l| l.weight > 0.0)
                    .collect();
                Lighting {
                    turn: t,
                    key: shadow_lights.first().map_or(key, |l| l.dir),
                    fill,
                    light_intensity: 0.0,
                    ambient: 1.0,
                    studio_key: false,
                    dominant,
                    shadow_mode: if shadow_lights.is_empty() { 0 } else { 2 },
                    shadow_lights,
                }
            }
        }
    }

    /// The direct-light and environment uniforms of the material program (after `MaterialPass::begin`, whose
    /// view-based light uniforms these replace).
    pub unsafe fn apply(&self, gl: &glow::Context, prog: &Program, env_scale: f32) {
        unsafe {
            let loc = |n: &str| prog.loc(n);
            gl.uniform_3_f32(loc("uL"), self.key[0], self.key[1], self.key[2]);
            gl.uniform_3_f32(loc("uFill"), self.fill[0], self.fill[1], self.fill[2]);
            gl.uniform_1_f32(loc("uLightI"), self.light_intensity);
            gl.uniform_1_f32(loc("uAmbient"), self.ambient);
            gl.uniform_1_f32(loc("uEnvScale"), env_scale);
            gl.uniform_1_i32(loc("uStudioKey"), self.studio_key as i32);
            let n = self.dominant.len().min(4);
            gl.uniform_1_i32(loc("uDomCount"), n as i32);
            let dirs: Vec<f32> = self.dominant.iter().take(4).flat_map(|(d, _)| *d).collect();
            let es: Vec<f32> = self.dominant.iter().take(4).flat_map(|(_, e)| *e).collect();
            if n > 0 {
                gl.uniform_3_f32_slice(loc("uDomDir"), &dirs);
                gl.uniform_3_f32_slice(loc("uDomE"), &es);
            }
        }
    }
}

/// The turn of an image environment: the light azimuth, plus the camera's when the light follows the camera
/// (only its azimuth: the horizon stays level).
fn env_turn(view: &View) -> f32 {
    let level = View { pitch: 0.0, light_pitch: 0.0, ..view.clone() };
    let (d, _) = light_dirs(&level);
    d[0].atan2(d[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_studio_turns_to_its_key_and_images_follow_the_light_azimuth() {
        let view = View { light_follows_camera: false, light_yaw: 0.9, light_pitch: 0.7, ..View::default() };
        let l = Lighting::of(&view, &Source::Studio);
        // the baked key direction (+Z at the key's elevation) turned by the studio's turn is the key
        let baked = [0.0, l.key[1], (1.0 - l.key[1] * l.key[1]).sqrt()];
        let back = turn(baked, l.turn);
        assert!((0..3).all(|i| (back[i] - l.key[i]).abs() < 1e-4), "{back:?} {:?}", l.key);
        assert_eq!((l.shadow_mode, l.shadow_lights.len()), (1, 1));
        // an image's turn is the light azimuth in the world; its dominant light turns with it
        let sun = DominantLight { dir: [0.0, 0.6, 0.8], irradiance: [5.0; 3], spread: 0.01 };
        let i = Lighting::of(&view, &Source::Image(vec![sun]));
        assert!((i.turn[1].atan2(i.turn[0]) - 0.9).abs() < 1e-4);
        assert_eq!((i.shadow_mode, i.light_intensity), (2, 0.0));
        let expect = turn(sun.dir, i.turn);
        assert_eq!(i.key, expect);
        // following the camera adds the camera's azimuth only
        let a = Lighting::of(&View { light_follows_camera: true, yaw: 0.5, pitch: 0.4, ..view.clone() }, &Source::Image(vec![sun]));
        let b = Lighting::of(&View { light_follows_camera: true, yaw: 0.5, pitch: -0.2, ..view.clone() }, &Source::Image(vec![sun]));
        assert!((a.turn[0] - b.turn[0]).abs() < 1e-5 && (a.turn[0] - i.turn[0]).abs() > 1e-3);
        assert_eq!(Lighting::of(&view, &Source::Image(Vec::new())).shadow_mode, 0);
    }
}

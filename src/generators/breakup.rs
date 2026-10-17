//! [`Breakup`]: the noise generators use to break up their masks (where a smart mask's dirt or wear is
//! patchy), and [`Texture`]: one of MaterialCraft's procedural textures ([`crate::proc_textures`]: a grunge
//! map by number, or any texture by id) used where a generator takes a texture input. Both are tileable in UV and can be projected tri-planar from the
//! position map instead, which hides UV seams.

use super::maps::SetMaps;
use super::texel::Texel;
use crate::noise::Fbm;
use crate::proc_textures::{self, ProcTexture};

/// Lattice cells across the texture at scale 1.
const BASE_CELLS: f32 = 3.0;

/// How a 2D pattern is laid on the surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Placement {
    Uv,
    /// Three planar projections of the position map, blended by the world normal; `sharpness` 0..1.
    Triplanar {
        sharpness: f32,
    },
}

impl Placement {
    pub fn of(triplanar: bool, contrast: f32) -> Placement {
        if triplanar { Placement::Triplanar { sharpness: contrast.clamp(0.0, 1.0) } } else { Placement::Uv }
    }

    /// `pattern` (tileable over 0..1) read at the texel, `scale` repeats across the texture or the scene.
    pub fn read(self, t: &Texel, maps: &SetMaps, scale: f32, pattern: impl Fn(f32, f32) -> f32) -> f32 {
        let wrap = |a: f32| (a * scale).rem_euclid(1.0);
        match self {
            Placement::Uv => pattern(wrap(t.u), wrap(t.v)),
            Placement::Triplanar { sharpness } => {
                let p = maps.position(t.u, t.v);
                let n = maps.normal(t.u, t.v);
                let power = 1.0 + sharpness * 15.0;
                let w = n.map(|c| c.abs().powf(power));
                let sum = (w[0] + w[1] + w[2]).max(1e-6);
                let x = pattern(wrap(p[2]), wrap(p[1]));
                let y = pattern(wrap(p[0]), wrap(p[2]));
                let z = pattern(wrap(p[0]), wrap(p[1]));
                (x * w[0] + y * w[1] + z * w[2]) / sum
            }
        }
    }
}

/// Patchy fractal noise, 0..1 around 0.5.
pub struct Breakup {
    fbm: Fbm,
    scale: f32,
    placement: Placement,
}

impl Breakup {
    pub fn new(scale: f32, seed: u32, placement: Placement) -> Breakup {
        let cells = (BASE_CELLS * scale.clamp(0.05, 64.0)).round().max(1.0) as i32;
        Breakup { fbm: Fbm::new(cells, cells, 6, 0.55, seed.wrapping_mul(2_654_435_761).wrapping_add(17), 0), scale: 1.0, placement }
    }

    pub fn value(&self, t: &Texel, maps: &SetMaps) -> f32 {
        self.placement.read(t, maps, self.scale, |u, v| self.fbm.sample(u, v))
    }
}

/// One of our procedural textures as a generator's texture input.
pub struct Texture {
    look: ProcTexture,
    scale: f32,
    placement: Placement,
}

impl Texture {
    /// `look` 1..=15 selects the grunge map.
    pub fn new(look: u32, scale: f32, seed: u32, placement: Placement) -> Texture {
        Texture::of(proc_textures::grunge_look(look.clamp(1, 15), seed), scale, placement)
    }

    /// The procedural texture `id` with its default settings, if there is one.
    pub fn by_id(id: &str, scale: f32, seed: u32, placement: Placement) -> Option<Texture> {
        let params = vec![("seed".to_string(), seed.to_string())];
        Some(Texture::of(proc_textures::texture(id, &params)?, scale, placement))
    }

    fn of(look: ProcTexture, scale: f32, placement: Placement) -> Texture {
        Texture { look, scale: scale.clamp(0.05, 64.0), placement }
    }

    /// The texture's value; for a grunge map white where clean, black where dirty.
    pub fn value(&self, t: &Texel, maps: &SetMaps) -> f32 {
        self.placement.read(t, maps, self.scale, |u, v| self.look.value(u, v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::TextureSet;

    #[test]
    fn noise_and_textures_stay_in_range_and_vary() {
        let maps = SetMaps::of(&TextureSet::new("s", "S", (4, 4)));
        let b = Breakup::new(4.0, 3, Placement::Uv);
        let t = Texture::new(1, 2.0, 3, Placement::Triplanar { sharpness: 0.5 });
        let mut seen = Vec::new();
        for i in 0..64 {
            let texel = Texel::at(i % 8, i / 8, 8, 8);
            let (n, g) = (b.value(&texel, &maps), t.value(&texel, &maps));
            assert!((0.0..=1.0).contains(&n) && (0.0..=1.0).contains(&g));
            seen.push(n);
        }
        assert!(seen.iter().any(|v| (v - seen[0]).abs() > 0.01));
    }

    #[test]
    fn looks_by_number_are_the_grunge_maps_and_ids_resolve() {
        use crate::grunge::{GrungeP, T0, grunge_map};
        let maps = SetMaps::of(&TextureSet::new("s", "S", (4, 4)));
        for look in 1..=15u32 {
            let t = Texture::new(look, 1.0, 9, Placement::Uv);
            let p = GrungeP { map: look, seed: 9, balance: 0.5, contrast: 0.5, brush: 0.0, cracks: 1.0, disorder: 0.0 };
            for i in 0..16 {
                let texel = Texel::at(i % 4, i / 4, 4, 4);
                assert_eq!(t.value(&texel, &maps), grunge_map(&p, texel.u, texel.v, T0[look as usize - 1]));
            }
        }
        assert!(Texture::by_id("clouds_2", 1.0, 0, Placement::Uv).is_some());
        assert!(Texture::by_id("nope", 1.0, 0, Placement::Uv).is_none());
    }
}

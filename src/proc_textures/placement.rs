//! A texture's [`Projection`] as generator parameters, both ways: [`read`] (what [`super::TextureSource`]
//! evaluates) and [`params`] (what an importer or the UI writes). Generators carry only parameters, so the
//! placement of a texture fill lives among them.
//!
//! | Parameter | Meaning |
//! |---|---|
//! | `projection` | mode id (`texture_space`, `uv`, `tri_planar`, `planar`, `spherical`, `cylindrical`); without it UV when a placement is set, else texture space |
//! | `tiling`, `tiling_v` | repetitions (u; v defaults to u) |
//! | `rotation` | degrees |
//! | `offset_x`, `offset_y` | image offset |
//! | `repeat` | `none`, `u`, `v`, `both` (default) |
//! | `volume_offset`, `volume_rotation`, `volume_scale` | three comma-separated numbers each |
//! | `clip_to_volume` | 0/1 |
//! | `blend_hardness` | tri-planar blend hardness (0.5) |
//! | `depth_falloff`, `depth_hardness` | planar depth falloff on/off and hardness |
//! | `facing_falloff`, `facing_angle`, `facing_hardness` | facing falloff (planar, cylindrical) |
//! | `coverage_angle`, `hide_caps` | cylindrical |

use crate::paint::effect::Params;
use crate::projection::{ImagePlacement, Projection, ProjectionMode, Repeat, Volume};

const PLACEMENT_KEYS: [&str; 5] = ["tiling", "tiling_v", "rotation", "offset_x", "offset_y"];

fn triple(p: &Params, name: &str, default: [f32; 3]) -> [f32; 3] {
    let Some(text) = p.text(name) else { return default };
    let mut out = default;
    for (o, v) in out.iter_mut().zip(text.split(',')) {
        if let Ok(v) = v.trim().parse::<f32>()
            && v.is_finite()
        {
            *o = v.clamp(-1.0e4, 1.0e4);
        }
    }
    out
}

fn read_volume(p: &Params) -> Volume {
    let d = Volume::default();
    Volume {
        offset: triple(p, "volume_offset", d.offset),
        rotation: triple(p, "volume_rotation", d.rotation),
        scale: triple(p, "volume_scale", d.scale),
        clip: p.on("clip_to_volume", d.clip),
    }
}

/// The projection the parameters describe.
pub fn read(params: &[(String, String)]) -> Projection {
    let p = Params::new(params);
    let tiling = p.range("tiling", 1.0, -1024.0, 1024.0);
    let image = ImagePlacement {
        tiling: [tiling, p.range("tiling_v", tiling, -1024.0, 1024.0)],
        rotation: p.range("rotation", 0.0, -3600.0, 3600.0),
        offset: [p.range("offset_x", 0.0, -1024.0, 1024.0), p.range("offset_y", 0.0, -1024.0, 1024.0)],
        ..ImagePlacement::default()
    };
    let placed = PLACEMENT_KEYS.iter().any(|k| p.text(k).is_some()) && !image.is_identity();
    let fallback = || if placed { ProjectionMode::Uv } else { ProjectionMode::TextureSpace };
    let mut mode = p.text("projection").and_then(|id| ProjectionMode::default_of(id.trim())).unwrap_or_else(fallback);
    if matches!(mode, ProjectionMode::Warp(_)) {
        mode = fallback();
    }
    let volume = read_volume(&p);
    match &mut mode {
        ProjectionMode::TriPlanar(m) => {
            m.volume = volume;
            m.blend_hardness = p.range("blend_hardness", 0.5, 0.0, 1.0);
        }
        ProjectionMode::Planar(m) => {
            m.volume = volume;
            m.depth_falloff.on = p.on("depth_falloff", false);
            m.depth_falloff.hardness = p.range("depth_hardness", 0.5, 0.0, 1.0);
            m.facing_falloff.on = p.on("facing_falloff", false);
            m.facing_falloff.angle = p.range("facing_angle", 90.0, 0.0, 180.0);
            m.facing_falloff.hardness = p.range("facing_hardness", 0.5, 0.0, 1.0);
        }
        ProjectionMode::Spherical(m) => m.volume = volume,
        ProjectionMode::Cylindrical(m) => {
            m.volume = volume;
            m.facing_falloff.on = p.on("facing_falloff", false);
            m.facing_falloff.angle = p.range("facing_angle", 90.0, 0.0, 180.0);
            m.facing_falloff.hardness = p.range("facing_hardness", 0.5, 0.0, 1.0);
            m.coverage_angle = p.range("coverage_angle", 360.0, 0.001, 360.0);
            m.hide_caps = p.on("hide_caps", false);
        }
        _ => {}
    }
    let repeat = p.text("repeat").and_then(|r| Repeat::from_id(r.trim())).unwrap_or_default();
    Projection { mode, repeat, image, ..Projection::default() }
}

fn nums(v: &[f32]) -> String {
    v.iter().map(f32::to_string).collect::<Vec<_>>().join(",")
}

/// The parameters of `projection` (none for the default; the filter and fill symmetry are not carried).
pub fn params(projection: &Projection) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut put = |k: &str, v: String| out.push((k.to_string(), v));
    if projection.mode != ProjectionMode::TextureSpace {
        put("projection", projection.mode.id().to_string());
    }
    let image = &projection.image;
    if !image.is_identity() {
        put("tiling", image.tiling[0].to_string());
        put("tiling_v", image.tiling[1].to_string());
        put("rotation", image.rotation.to_string());
        put("offset_x", image.offset[0].to_string());
        put("offset_y", image.offset[1].to_string());
    }
    if projection.repeat != Repeat::Both {
        put("repeat", projection.repeat.id().to_string());
    }
    if let Some(v) = projection.mode.volume() {
        put("volume_offset", nums(&v.offset));
        put("volume_rotation", nums(&v.rotation));
        put("volume_scale", nums(&v.scale));
        put("clip_to_volume", u8::from(v.clip).to_string());
    }
    match &projection.mode {
        ProjectionMode::TriPlanar(m) => put("blend_hardness", m.blend_hardness.to_string()),
        ProjectionMode::Planar(m) => {
            put("depth_falloff", u8::from(m.depth_falloff.on).to_string());
            put("depth_hardness", m.depth_falloff.hardness.to_string());
            put("facing_falloff", u8::from(m.facing_falloff.on).to_string());
            put("facing_angle", m.facing_falloff.angle.to_string());
            put("facing_hardness", m.facing_falloff.hardness.to_string());
        }
        ProjectionMode::Cylindrical(m) => {
            put("facing_falloff", u8::from(m.facing_falloff.on).to_string());
            put("facing_angle", m.facing_falloff.angle.to_string());
            put("facing_hardness", m.facing_falloff.hardness.to_string());
            put("coverage_angle", m.coverage_angle.to_string());
            put("hide_caps", u8::from(m.hide_caps).to_string());
        }
        _ => {}
    }
    out
}

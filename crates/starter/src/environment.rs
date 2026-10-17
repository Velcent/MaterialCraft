//! The studio environment: the analytic studio the viewport lights and reflects with (`src/gpu3d/glsl/material.frag`,
//! `envSoft` and `env` at roughness 0: soft sky and floor, a key softbox, a side strip, a top panel, a horizon
//! line), baked into a lat-long OpenEXR so it can be picked as an environment asset. The formulas are the shader's,
//! restated here; keep them in step when the shader's studio changes (and bump the library version).

use crate::build::Options;
use crate::image::{THUMB_PX, rgb8};
use crate::recipe::sidecar;
use crate::{Made, Recipe, StarterError};
use materialcraft::assets::{Provenance, Usage};
use materialcraft::exr::{self, ExrChannel, ExrImage, SampleKind};
use materialcraft::json::Value;
use materialcraft::paint::working_space::linear_to_srgb;
use materialcraft::tex::Tex;
use std::f32::consts::{PI, TAU};

/// Width of the lat-long image (its height is half).
const WIDTH: usize = 1024;
/// Direction of the key light: yaw and pitch in radians (the viewport's default light).
const KEY_YAW: f32 = 0.9;
const KEY_PITCH: f32 = 0.7;

pub fn recipes() -> Vec<Box<dyn Recipe>> {
    vec![Box::new(StudioEnvironment)]
}

struct StudioEnvironment;

type V3 = [f32; 3];

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normalize(a: V3) -> V3 {
    let l = dot(a, a).sqrt().max(1e-12);
    [a[0] / l, a[1] / l, a[2] / l]
}

fn mix(a: V3, b: V3, t: f32) -> V3 {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The shader's studio at roughness 0, for the key light direction `key`.
fn radiance(d: V3, key: V3) -> V3 {
    let soft = {
        let (sky, hor, ground) = ([0.30, 0.34, 0.42], [0.52, 0.52, 0.53], [0.20, 0.19, 0.18]);
        if d[1] > 0.0 { mix(hor, sky, d[1].powf(0.6)) } else { mix(hor, ground, (-d[1]).powf(0.5)) }
    };
    let w = 0.015;
    let k = smoothstep(0.93 - w, 0.93 + 0.015, dot(d, key)) * 4.5;
    let side = normalize([-key[2], 0.0, key[0]]);
    let sdot = dot(normalize([d[0] + 1e-5, 1e-5, d[2] + 1e-5]), side);
    let strip = smoothstep(0.93 - w, 0.97, sdot) * smoothstep(0.02 - w, 0.12, d[1]) * (1.0 - smoothstep(0.55, 0.75 + w, d[1])) * 1.5;
    let top = smoothstep(0.90 - w, 0.97, d[1]);
    let horizon = (-(d[1] / 0.035).powi(2)).exp() * 0.25;
    let (kc, sc) = ([1.0, 0.96, 0.90], [0.85, 0.92, 1.0]);
    [0, 1, 2].map(|i| {
        let (s, kc, sc) = (soft.get(i).copied().unwrap_or(0.0), kc.get(i).copied().unwrap_or(1.0), sc.get(i).copied().unwrap_or(1.0));
        s + kc * k + sc * strip + top + 0.9 * horizon
    })
}

/// The direction of lat-long texel centre (u, v): u = 0.5 looks along −Z, v = 0 is straight up, +Y up.
fn direction(u: f32, v: f32) -> V3 {
    let (phi, theta) = ((u - 0.5) * TAU, v * PI);
    [theta.sin() * phi.sin(), theta.cos(), -theta.sin() * phi.cos()]
}

fn key() -> V3 {
    normalize([KEY_PITCH.cos() * KEY_YAW.sin(), KEY_PITCH.sin(), KEY_PITCH.cos() * KEY_YAW.cos()])
}

/// The environment `w` × `w / 2`, linear RGB, row by row.
fn render(w: usize) -> (usize, Vec<V3>) {
    let h = (w / 2).max(1);
    let key = key();
    let px = (0..w * h).map(|i| radiance(direction(((i % w) as f32 + 0.5) / w as f32, ((i / w) as f32 + 0.5) / h as f32), key)).collect();
    (h, px)
}

impl Recipe for StudioEnvironment {
    fn path(&self) -> String {
        format!("{}/studio.exr", Usage::Environment.folder())
    }

    fn make(&self, _: &Options) -> Result<Made, StarterError> {
        let (h, px) = render(WIDTH);
        let plane = |c: usize| px.iter().map(|p| p.get(c).copied().unwrap_or(0.0)).collect::<Vec<f32>>();
        let channels = [("B", 2), ("G", 1), ("R", 0)].iter().map(|(n, c)| ExrChannel::new(n, SampleKind::Half, plane(*c))).collect();
        let bytes = exr::encode(&ExrImage { width: WIDTH, height: h, channels }).map_err(|e| StarterError::recipe(&self.path(), e))?;
        let (th, small) = render(THUMB_PX);
        let tone = |x: f32| linear_to_srgb((x / (1.0 + x)).clamp(0.0, 1.0));
        let thumb = Tex { w: THUMB_PX, h: th, px: small.iter().map(|p| [tone(p[0]), tone(p[1]), tone(p[2]), 1.0]).collect() };
        let preset = Value::object()
            .with("key_yaw", KEY_YAW)
            .with("key_pitch", KEY_PITCH)
            .with("width", WIDTH)
            .with("height", h)
            .with("source", "src/gpu3d/glsl/material.frag env()");
        let description = format!(
            "MaterialCraft's studio (the viewport's built-in lighting) as a lat-long HDR environment, {WIDTH} × {h}, half float: soft sky and floor, a key softbox, a side strip and a top panel. +Y up; the image centre looks along -Z."
        );
        let provenance = Provenance::Generator { generator: "studio_environment".into(), preset: Some(preset) };
        Ok(Made {
            bytes,
            sidecar: sidecar("Studio", Usage::Environment, &["studio", "hdr", "lighting"], &description, provenance),
            thumbnail: Some(rgb8(&thumb)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_softbox_is_the_brightest_place_and_the_floor_is_dark() {
        let k = key();
        let at_key = radiance(k, k);
        let floor = radiance([0.0, -1.0, 0.0], k);
        assert!(at_key[0] > 4.0 && floor[0] < 0.3, "{at_key:?} {floor:?}");
        let d = direction(0.5, 0.5);
        assert!((d[2] + 1.0).abs() < 1e-3 && d[1].abs() < 1e-3);
    }
}

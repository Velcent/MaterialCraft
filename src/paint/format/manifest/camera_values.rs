//! A camera's pose and settings ([`CameraValues`]) as fields of a `view` camera object: `projection`,
//! `fov_y_degrees`, `yaw`, `pitch`, `distance`, `pan`, `roll` (since 0.14), `aspect` (since 0.14) and
//! `depth_of_field`. The same fields describe a camera's current values and, under `imported.original`, the values
//! it was imported with.

use super::extras::Extras;
use super::fields::{Obj, as_f32, num};
use super::{FormatError, ReadCtx};
use crate::camera::{CameraSettings, DepthOfField, Orbit, Projection};
use crate::json::Value;
use crate::paint::saved_camera::CameraValues;

/// `out` with the fields of `v` added; unknown fields kept at `here` are put back into the depth of field object.
pub fn write(mut out: Value, v: &CameraValues, here: &str, extras: &Extras) -> Value {
    let s = &v.settings;
    let d = &s.depth_of_field;
    let mut dof = Value::object().with("enabled", d.enabled).with("focus_distance", num(d.focus_distance)).with("aperture", num(d.aperture));
    extras.inject(&format!("{here}/depth_of_field"), &mut dof);
    out = out
        .with("projection", s.projection.id())
        .with("fov_y_degrees", num(s.fov_y.to_degrees()))
        .with("yaw", num(v.orbit.yaw))
        .with("pitch", num(v.orbit.pitch))
        .with("distance", num(v.orbit.distance))
        .with("pan", v.orbit.pan.iter().map(|v| num(*v)).collect::<Value>());
    if v.orbit.roll != 0.0 {
        out = out.with("roll", num(v.orbit.roll));
    }
    if let Some(a) = v.aspect {
        out = out.with("aspect", num(a));
    }
    out.with("depth_of_field", dof)
}

/// Reads the fields of `o` (missing ones get the default camera's values).
pub fn read(o: &mut Obj, here: &str, ctx: &mut ReadCtx) -> Result<CameraValues, FormatError> {
    let d = CameraValues::default();
    let projection = match o.opt_str("projection")? {
        None => Projection::Perspective,
        Some(id) => Projection::from_id(id).ok_or_else(|| o.error("projection", format!("unknown projection {id:?}")))?,
    };
    let mut settings = CameraSettings { projection, ..d.settings };
    settings.set_fov_y(o.f32_or("fov_y_degrees", d.settings.fov_y.to_degrees())?.to_radians());
    let pan = match o.opt("pan") {
        None => d.orbit.pan,
        Some(p) => p
            .as_array()
            .filter(|a| a.len() == 2)
            .and_then(|a| Some([as_f32(a.first()?)?, as_f32(a.get(1)?)?]))
            .ok_or_else(|| o.error("pan", "expected [x, y]"))?,
    };
    let orbit = Orbit {
        yaw: o.f32_or("yaw", d.orbit.yaw)?,
        pitch: o.f32_or("pitch", d.orbit.pitch)?,
        distance: o.f32_or("distance", d.orbit.distance)?,
        pan,
        roll: o.f32_or("roll", 0.0)?,
    };
    let aspect = match o.opt("aspect") {
        None => None,
        Some(a) => Some(as_f32(a).filter(|a| *a > 0.0).ok_or_else(|| o.error("aspect", "expected a positive number"))?),
    };
    if let Some(dof) = o.opt("depth_of_field") {
        let def = DepthOfField::default();
        let mut p = Obj::new(dof, o.path("depth_of_field"))?;
        settings.depth_of_field = DepthOfField {
            enabled: p.bool_or("enabled", def.enabled)?,
            focus_distance: p.f32_or("focus_distance", def.focus_distance)?,
            aperture: p.f32_or("aperture", def.aperture)?,
        };
        ctx.extras.record(&format!("{here}/depth_of_field"), &p);
    }
    Ok(CameraValues { orbit, settings, aspect })
}

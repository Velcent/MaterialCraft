//! `view` (since 0.12): the document's [`SavedView`]: camera list, active camera, exposure and environment.
//! Optional; absent when the document has none. Unknown fields are kept like everywhere else (locations `view`,
//! `view/cameras/<index>`, `…/depth_of_field`, `view/environment`). A view that cannot be read (an unknown
//! projection, a wrong type) is dropped with no error: it is display state, never a reason to refuse a document.

use super::extras::Extras;
use super::fields::{Obj, as_f32, num};
use super::{FormatError, ReadCtx};
use crate::camera::{CameraSettings, DepthOfField, Orbit, Projection};
use crate::json::Value;
use crate::paint::saved_view::{SavedCamera, SavedEnvironment, SavedView};

const HERE: &str = "view";

fn camera(i: usize, c: &SavedCamera, extras: &Extras) -> Value {
    let here = format!("{HERE}/cameras/{i}");
    let s = &c.settings;
    let d = &s.depth_of_field;
    let mut dof = Value::object().with("enabled", d.enabled).with("focus_distance", num(d.focus_distance)).with("aperture", num(d.aperture));
    extras.inject(&format!("{here}/depth_of_field"), &mut dof);
    let mut v = Value::object()
        .with("name", c.name.as_str())
        .with("projection", s.projection.id())
        .with("fov_y_degrees", num(s.fov_y.to_degrees()))
        .with("yaw", num(c.orbit.yaw))
        .with("pitch", num(c.orbit.pitch))
        .with("distance", num(c.orbit.distance))
        .with("pan", c.orbit.pan.iter().map(|v| num(*v)).collect::<Value>())
        .with("depth_of_field", dof);
    extras.inject(&here, &mut v);
    v
}

/// The field's value.
pub fn write(v: &SavedView, extras: &Extras) -> Value {
    let e = &v.environment;
    let mut env = Value::object()
        .with("kind", e.kind.as_str())
        .with("light_yaw", num(e.light_yaw))
        .with("light_pitch", num(e.light_pitch))
        .with("light_intensity", num(e.light_intensity))
        .with("light_follows_camera", e.light_follows_camera)
        .with("ambient", num(e.ambient));
    extras.inject(&format!("{HERE}/environment"), &mut env);
    let mut out = Value::object()
        .with("cameras", v.cameras.iter().enumerate().map(|(i, c)| camera(i, c, extras)).collect::<Value>())
        .with("active_camera", v.active_camera)
        .with("exposure", num(v.exposure_stops))
        .with("environment", env);
    extras.inject(HERE, &mut out);
    out
}

fn read_camera(i: usize, value: &Value, at: String, ctx: &mut ReadCtx) -> Result<SavedCamera, FormatError> {
    let here = format!("{HERE}/cameras/{i}");
    let d = SavedCamera::default();
    let mut o = Obj::new(value, at)?;
    let name = o.opt_str("name")?.unwrap_or(SavedCamera::DEFAULT_NAME).to_string();
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
    let orbit = Orbit { yaw: o.f32_or("yaw", d.orbit.yaw)?, pitch: o.f32_or("pitch", d.orbit.pitch)?, distance: o.f32_or("distance", d.orbit.distance)?, pan };
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
    ctx.extras.record(&here, &o);
    Ok(SavedCamera { name, orbit, settings })
}

fn read_environment(value: &Value, at: String, ctx: &mut ReadCtx) -> Result<SavedEnvironment, FormatError> {
    let d = SavedEnvironment::default();
    let mut o = Obj::new(value, at)?;
    let e = SavedEnvironment {
        kind: o.opt_str("kind")?.unwrap_or(SavedEnvironment::STUDIO).to_string(),
        light_yaw: o.f32_or("light_yaw", d.light_yaw)?,
        light_pitch: o.f32_or("light_pitch", d.light_pitch)?,
        light_intensity: o.f32_or("light_intensity", d.light_intensity)?,
        light_follows_camera: o.bool_or("light_follows_camera", d.light_follows_camera)?,
        ambient: o.f32_or("ambient", d.ambient)?,
    };
    ctx.extras.record(&format!("{HERE}/environment"), &o);
    Ok(e)
}

/// Reads the root's `view`: `None` when absent or unreadable.
pub fn read(root: &mut Obj, ctx: &mut ReadCtx) -> Option<SavedView> {
    let value = root.opt(HERE)?;
    read_view(value, root.path(HERE), ctx).ok()
}

fn read_view(value: &Value, at: String, ctx: &mut ReadCtx) -> Result<SavedView, FormatError> {
    let mut o = Obj::new(value, at)?;
    let at = o.path("cameras");
    let mut cameras = Vec::new();
    for (i, c) in o.array("cameras")?.iter().enumerate() {
        cameras.push(read_camera(i, c, format!("{at}[{i}]"), ctx)?);
    }
    if cameras.is_empty() {
        cameras.push(SavedCamera::default());
    }
    let active_camera = match o.opt("active_camera") {
        None => 0,
        Some(v) => v.as_usize().ok_or_else(|| o.error("active_camera", "expected a non-negative integer"))?,
    };
    let environment = match o.opt("environment") {
        None => SavedEnvironment::default(),
        Some(e) => read_environment(e, o.path("environment"), ctx)?,
    };
    let view = SavedView { cameras, active_camera, exposure_stops: o.f32_or("exposure", 0.0)?, environment };
    ctx.extras.record(HERE, &o);
    Ok(view)
}

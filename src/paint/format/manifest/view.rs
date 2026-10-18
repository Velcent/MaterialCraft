//! `view` (since 0.12): the document's [`SavedView`]: camera list, active camera, exposure and environment.
//! Since 0.14 a camera may be `locked` and carry `imported` (its source and original values, see
//! [`camera_values`]). Optional; absent when the document has none. Unknown fields are kept like everywhere else
//! (locations `view`, `view/cameras/<index>`, `…/depth_of_field`, `…/imported`, `…/imported/original`,
//! `…/imported/original/depth_of_field`, `view/environment`). A view that cannot be read (an unknown
//! projection, a wrong type) is dropped with no error: it is display state, never a reason to refuse a document.

use super::camera_values;
use super::extras::Extras;
use super::fields::{Obj, num};
use super::{FormatError, ReadCtx};
use crate::json::Value;
use crate::paint::saved_view::{CameraOrigin, SavedCamera, SavedEnvironment, SavedView};

const HERE: &str = "view";

fn camera(i: usize, c: &SavedCamera, extras: &Extras) -> Value {
    let here = format!("{HERE}/cameras/{i}");
    let mut v = camera_values::write(Value::object().with("name", c.name.as_str()), &c.values(), &here, extras);
    if c.locked {
        v = v.with("locked", true);
    }
    if let CameraOrigin::Imported { source, original } = &c.origin {
        let at = format!("{here}/imported");
        let mut o = camera_values::write(Value::object(), original, &format!("{at}/original"), extras);
        extras.inject(&format!("{at}/original"), &mut o);
        let mut imported = Value::object().with("source", source.as_str()).with("original", o);
        extras.inject(&at, &mut imported);
        v = v.with("imported", imported);
    }
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
    let mut o = Obj::new(value, at)?;
    let name = o.opt_str("name")?.unwrap_or(SavedCamera::DEFAULT_NAME).to_string();
    let values = camera_values::read(&mut o, &here, ctx)?;
    let locked = o.bool_or("locked", false)?;
    let origin = match o.opt("imported") {
        None => CameraOrigin::Own,
        Some(imported) => read_origin(imported, o.path("imported"), &format!("{here}/imported"), ctx)?,
    };
    ctx.extras.record(&here, &o);
    Ok(SavedCamera { locked, origin, ..SavedCamera::own(&name, values) })
}

/// `imported`: where the camera came from and the values it was imported with.
fn read_origin(value: &Value, at: String, here: &str, ctx: &mut ReadCtx) -> Result<CameraOrigin, FormatError> {
    let mut o = Obj::new(value, at)?;
    let source = o.str("source")?.to_string();
    let mut original = Obj::new(o.req("original")?, o.path("original"))?;
    let values = camera_values::read(&mut original, &format!("{here}/original"), ctx)?;
    ctx.extras.record(&format!("{here}/original"), &original);
    ctx.extras.record(here, &o);
    Ok(CameraOrigin::Imported { source, original: values })
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

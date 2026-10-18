//! Project settings (since 0.17): the root's `project`, written only when it differs from the defaults (so documents
//! that never chose them are written as before).
//!
//! ```json
//! "project": { "normal_map_format": "directx", "tangent_per_fragment": true, "fill_physical_size": false }
//! ```
//!
//! Unknown fields are kept at `project`.

use super::extras::Extras;
use super::fields::Obj;
use super::{FormatError, ReadCtx};
use crate::json::Value;
use crate::paint::project_settings::{NormalMapFormat, ProjectSettings};

const HERE: &str = "project";

/// The settings as a JSON object.
pub fn to_json(p: &ProjectSettings, extras: &Extras) -> Value {
    let mut v = Value::object()
        .with("normal_map_format", p.normal_map_format.id())
        .with("tangent_per_fragment", p.tangent_per_fragment)
        .with("fill_physical_size", p.fill_physical_size);
    extras.inject(HERE, &mut v);
    v
}

/// Adds `project` to `root` unless the settings are the defaults and no unknown fields were kept.
pub fn write(p: &ProjectSettings, root: &mut Value, extras: &Extras) {
    let v = to_json(p, extras);
    let kept_extras = v.as_object().is_some_and(|o| o.len() > 3);
    if !p.is_default() || kept_extras {
        root.insert(HERE, v);
    }
}

/// Reads a settings object (missing fields take their defaults).
pub fn from_json(v: &Value, at: String, ctx: &mut ReadCtx) -> Result<ProjectSettings, FormatError> {
    let d = ProjectSettings::default();
    let mut o = Obj::new(v, at)?;
    let normal_map_format = match o.opt_str("normal_map_format")? {
        None => d.normal_map_format,
        Some(id) => NormalMapFormat::from_id(id).ok_or_else(|| o.error("normal_map_format", format!("unknown normal map format {id:?}")))?,
    };
    let p = ProjectSettings {
        normal_map_format,
        tangent_per_fragment: o.bool_or("tangent_per_fragment", d.tangent_per_fragment)?,
        fill_physical_size: o.bool_or("fill_physical_size", d.fill_physical_size)?,
    };
    ctx.extras.record(HERE, &o);
    Ok(p)
}

/// Reads the root's `project`; the defaults when it is absent.
pub fn read(root: &mut Obj, ctx: &mut ReadCtx) -> Result<ProjectSettings, FormatError> {
    match root.opt(HERE) {
        None => Ok(ProjectSettings::default()),
        Some(v) => from_json(v, root.path(HERE), ctx),
    }
}

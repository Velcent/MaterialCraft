//! Project templates as library assets: `templates/<name>.mctemplate`, a small JSON file built from the same pieces as
//! `.mcpaint`'s manifest (a texture set without layers, the view, shader instances, the project settings):
//!
//! ```json
//! { "format": "materialcraft-template", "format_version": "1.0", "name": "Game prop",
//!   "texture_set": { "id": "template", "resolution": [2048, 2048], "channels": […], "bake": {…}, "shader": "pbr", … },
//!   "view": { "exposure": 0, "environment": {…}, … },
//!   "shader_instances": [ { "id": "pbr", … } ],
//!   "project": { "normal_map_format": "directx", … } }
//! ```
//!
//! Writing and reading back gives the same template. Unknown fields are ignored.

use super::FormatError;
use super::manifest::extras::Extras;
use super::manifest::fields::Obj;
use super::manifest::{ReadCtx, WriteCtx, project, shading, texture_set, view};
use super::paths::ImagePlan;
use super::strokes::StrokePlan;
use crate::json::{self, Value};
use crate::paint::Document;
use crate::paint::new_project::template::ProjectTemplate;
use crate::shading::ShaderSettings;

/// The file extension of a template asset.
pub const EXTENSION: &str = "mctemplate";
/// The format tag in every template file.
pub const FORMAT: &str = "materialcraft-template";
/// The version written; readers accept any `1.x`.
pub const VERSION: &str = "1.0";

/// The template as the text of a `.mctemplate` file (pretty JSON, ending in a newline).
pub fn to_text(t: &ProjectTemplate) -> String {
    let empty = Document::default();
    let plan = ImagePlan::new(&empty);
    let strokes = StrokePlan::new(&empty, &plan);
    let extras = Extras::default();
    let ctx = WriteCtx { plan: &plan, strokes: &strokes, extras: &extras };
    let mut set = t.set.clone();
    set.layers.clear();
    set.baked.clear();
    let mut sets = Value::Array(vec![texture_set::write(0, &set, &ctx)]);
    let mut shaders = ShaderSettings::default();
    if let Some(instance) = &t.shader {
        shaders.instances.push(instance.clone());
        shaders.assign(&set.id, Some(instance.id.clone()));
    }
    shading::assign_sets(&shaders, &mut sets);
    let set_json = sets.as_array_mut().and_then(|a| a.pop()).unwrap_or_else(Value::object);
    let mut v = Value::object()
        .with("format", FORMAT)
        .with("format_version", VERSION)
        .with("name", t.name.as_str())
        .with("texture_set", set_json)
        .with("view", view::write(&t.view, &extras));
    shading::write(&shaders, &mut v, &extras);
    v.insert("project", project::to_json(&t.project, &extras));
    format!("{}\n", v.to_string_pretty())
}

/// Reads a `.mctemplate` file's text.
pub fn from_text(text: &str) -> Result<ProjectTemplate, FormatError> {
    let root = json::parse(text).map_err(|e| FormatError::manifest("", format!("not JSON: {e}")))?;
    let mut o = Obj::new(&root, "")?;
    match o.opt_str("format")? {
        Some(FORMAT) => {}
        other => return Err(FormatError::manifest("format", format!("{other:?} is not a project template"))),
    }
    let version = o.str("format_version")?;
    if version.split('.').next() != Some("1") {
        return Err(FormatError::NewerVersion { found: version.to_string(), supported: "1.x".into() });
    }
    let mut ctx = ReadCtx::default();
    let name = o.str("name")?.to_string();
    let set = texture_set::read(0, o.req("texture_set")?, &mut ctx)?;
    let view = view::read(&mut o, &mut ctx).unwrap_or_default();
    let shaders = shading::read(&mut o, &mut ctx)?;
    let shader = shaders.instance_for(&set.id).cloned();
    let project = match o.opt("project") {
        None => Default::default(),
        Some(p) => project::from_json(p, o.path("project"), &mut ctx)?,
    };
    Ok(ProjectTemplate { name, set, view, shader, project })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::Channel;
    use crate::shading::{ParamValue, ShaderFile, ShaderInstance};

    #[test]
    fn templates_round_trip() {
        for mut t in ProjectTemplate::builtins() {
            assert_eq!(from_text(&to_text(&t)).unwrap(), t, "{}", t.name);
            let mut skin = ShaderInstance::new("skin", "Skin", &ShaderFile::builtin());
            skin.set_value("ao_strength", Some(ParamValue::Float(0.25)));
            t.shader = Some(skin);
            t.set.bake.common.samples = 4;
            t.view.exposure_stops = -1.0;
            t.view.environment.map = Some("environments/sky.hdr".into());
            t.view.environment.kind = "map".into();
            let text = to_text(&t);
            assert_eq!(from_text(&text).unwrap(), t, "{text}");
        }
    }

    #[test]
    fn other_files_are_refused() {
        assert!(from_text("not json").is_err());
        assert!(from_text("{\"format\": \"materialcraft-brush\"}").is_err());
        let t = ProjectTemplate::with_channels("x", &[Channel::BaseColor], 256);
        let newer = to_text(&t).replace("\"1.0\"", "\"2.0\"");
        assert!(matches!(from_text(&newer), Err(FormatError::NewerVersion { .. })));
    }
}

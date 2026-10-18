//! [`ProjectTemplate`]: a saved set of defaults for new projects: the texture set settings (channels and their
//! formats, resolution, bake settings), the display (exposure and environment, map included), the shader instance
//! every set gets and the project settings. Saved as `.mctemplate` files in an asset library
//! ([`crate::paint::format::template`]); two are built in.

use crate::paint::Channel;
use crate::paint::Document;
use crate::paint::TextureSet;
use crate::paint::project_settings::{NormalMapFormat, ProjectSettings};
use crate::paint::saved_view::SavedView;
use crate::shading::ShaderInstance;

/// Id of the texture set a template keeps its set defaults in.
pub const TEMPLATE_SET_ID: &str = "template";

/// Defaults for new projects.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectTemplate {
    pub name: String,
    /// The defaults of every texture set: channels, resolution, UV set, bake settings. Its layers and baked maps are
    /// not used (a template keeps none).
    pub set: TextureSet,
    /// Exposure and environment; its camera list is always the default one (cameras belong to a mesh).
    pub view: SavedView,
    /// The shader instance every texture set gets; `None` for the built-in shader's defaults.
    pub shader: Option<ShaderInstance>,
    pub project: ProjectSettings,
}

/// The channels of a metal/rough PBR material, in display order.
const PBR_CHANNELS: [Channel; 5] = [Channel::BaseColor, Channel::Roughness, Channel::Metallic, Channel::Normal, Channel::Height];

impl Default for ProjectTemplate {
    /// The first built-in template: metal/rough PBR, 2048 texels, OpenGL normal maps.
    fn default() -> ProjectTemplate {
        ProjectTemplate::with_channels("Metal/rough PBR", &PBR_CHANNELS, 2048)
    }
}

impl ProjectTemplate {
    /// A template named `name` with `channels` at `resolution`, the default bake settings and view.
    pub fn with_channels(name: &str, channels: &[Channel], resolution: u32) -> ProjectTemplate {
        let set = channels.iter().fold(TextureSet::new(TEMPLATE_SET_ID, name, (resolution, resolution)), |s, c| s.with_channel(*c));
        ProjectTemplate { name: name.to_string(), set, view: SavedView::default(), shader: None, project: ProjectSettings::default() }
    }

    /// The built-in templates: metal/rough PBR for OpenGL-style engines, and the same with opacity, DirectX normal maps
    /// and per-pixel bitangents for DirectX-style engines.
    pub fn builtins() -> Vec<ProjectTemplate> {
        let standard = ProjectTemplate::default();
        let mut channels = PBR_CHANNELS.to_vec();
        channels.push(Channel::Opacity);
        let mut directx = ProjectTemplate::with_channels("Metal/rough PBR with opacity, DirectX normals", &channels, 2048);
        directx.project = ProjectSettings { normal_map_format: NormalMapFormat::DirectX, tangent_per_fragment: true, fill_physical_size: false };
        vec![standard, directx]
    }

    /// The defaults `doc` uses: its first texture set's settings (without layers and baked maps), its exposure and
    /// environment, that set's shader instance and its project settings. `None` for a document without texture sets.
    pub fn from_document(doc: &Document, name: &str) -> Option<ProjectTemplate> {
        let first = doc.texture_sets.first()?;
        let mut set = TextureSet::new(TEMPLATE_SET_ID, name, first.resolution);
        set.channels = first.channels.clone();
        set.uv_set = first.uv_set;
        set.bake = first.bake.clone();
        // the bake's high-poly files belong to the document's mesh, not to new projects
        set.bake.common.high = crate::paint::bake_config::HighPoly::low_as_high();
        let view = doc.view.as_ref().map(|v| SavedView { cameras: SavedView::default().cameras, active_camera: 0, ..v.clone() }).unwrap_or_default();
        let shader = doc.shader_instance(&first.id).cloned();
        Some(ProjectTemplate { name: name.to_string(), set, view, shader, project: doc.project })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_differ_and_from_document_keeps_only_defaults() {
        let b = ProjectTemplate::builtins();
        assert_eq!(b.len(), 2);
        assert!(b.iter().all(|t| t.set.has_channel(Channel::BaseColor) && t.set.layers.is_empty()));
        assert_ne!(b[0].project, b[1].project);
        let mut doc = Document::default();
        assert!(ProjectTemplate::from_document(&doc, "x").is_none());
        let mut set = TextureSet::new("body", "Body", (512, 512)).with_channel(Channel::Roughness);
        set.bake.common.high.files.push("high.obj".into());
        doc.texture_sets.push(set);
        doc.project.tangent_per_fragment = true;
        doc.view = Some(SavedView { exposure_stops: 1.5, ..SavedView::default() });
        let t = ProjectTemplate::from_document(&doc, "Mine").unwrap();
        assert_eq!((t.name.as_str(), t.set.resolution, t.set.channels.len()), ("Mine", (512, 512), 1));
        assert!(t.set.bake.common.high.files.is_empty() && t.set.bake.common.high.low_as_high);
        assert!(t.view.cameras == SavedView::default().cameras && t.view.exposure_stops == 1.5);
        assert!(t.project.tangent_per_fragment);
    }
}

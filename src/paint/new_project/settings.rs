//! [`NewProject`]: the choices made in the New project dialog, with the defaults a template gives.

use super::template::ProjectTemplate;
use crate::mesh::LengthUnit;
use crate::paint::project_settings::{NormalMapFormat, ProjectSettings};
use std::path::PathBuf;

/// Texture set resolutions offered when painting (square, in texels). Exports may go higher later.
pub const RESOLUTIONS: [u32; 6] = [128, 256, 512, 1024, 2048, 4096];

/// What one mesh unit measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UnitScale {
    /// The unit the file states (glTF: metres); unknown for files that state none (OBJ).
    #[default]
    FromFile,
    /// This unit, whatever the file says.
    Custom(LengthUnit),
}

/// Everything needed to make a project from a mesh file.
#[derive(Debug, Clone, PartialEq)]
pub struct NewProject {
    /// The mesh file (OBJ or glTF binary).
    pub mesh: PathBuf,
    /// Texture set, display, baking and shader defaults.
    pub template: ProjectTemplate,
    /// Width and height of every texture set (one of [`RESOLUTIONS`]; others are snapped to the nearest).
    pub resolution: u32,
    pub normal_map_format: NormalMapFormat,
    pub tangent_per_fragment: bool,
    /// Add the file's cameras to the camera list.
    pub import_cameras: bool,
    /// Image files to use as baked maps, assigned by their names (`<texture set>_<map>`).
    pub baked_maps: Vec<PathBuf>,
    pub unit: UnitScale,
    /// Material drops make fills sized physically instead of tiled over the UVs.
    pub fill_physical_size: bool,
}

impl NewProject {
    /// Settings for `mesh` with `template`'s defaults.
    pub fn new(mesh: impl Into<PathBuf>, template: ProjectTemplate) -> NewProject {
        let p = template.project;
        NewProject {
            mesh: mesh.into(),
            resolution: snap_resolution(template.set.resolution.0.max(template.set.resolution.1)),
            normal_map_format: p.normal_map_format,
            tangent_per_fragment: p.tangent_per_fragment,
            fill_physical_size: p.fill_physical_size,
            template,
            import_cameras: true,
            baked_maps: Vec::new(),
            unit: UnitScale::FromFile,
        }
    }

    /// Takes over `template`'s defaults (resolution, normal map format, tangent frame, fill sizing), keeping the
    /// mesh, the baked map files and the import choices.
    pub fn use_template(&mut self, template: ProjectTemplate) {
        let fresh = NewProject::new(self.mesh.clone(), template);
        *self = NewProject { import_cameras: self.import_cameras, baked_maps: std::mem::take(&mut self.baked_maps), unit: self.unit, ..fresh };
    }

    /// The document-wide settings these choices give.
    pub fn project_settings(&self) -> ProjectSettings {
        ProjectSettings {
            normal_map_format: self.normal_map_format,
            tangent_per_fragment: self.tangent_per_fragment,
            fill_physical_size: self.fill_physical_size,
        }
    }

    /// The resolution, snapped to [`RESOLUTIONS`].
    pub fn texture_size(&self) -> u32 {
        snap_resolution(self.resolution)
    }
}

/// The entry of [`RESOLUTIONS`] nearest to `size` (in steps of powers of two).
pub fn snap_resolution(size: u32) -> u32 {
    let log = |v: u32| f64::from(v.max(1)).log2();
    RESOLUTIONS.into_iter().min_by(|a, b| (log(*a) - log(size)).abs().total_cmp(&(log(*b) - log(size)).abs())).unwrap_or(2048)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolutions_snap_into_the_painting_range() {
        assert_eq!(snap_resolution(2048), 2048);
        assert_eq!(snap_resolution(8192), 4096);
        assert_eq!(snap_resolution(1), 128);
        assert_eq!(snap_resolution(0), 128);
        assert_eq!(snap_resolution(1400), 1024);
        assert_eq!(snap_resolution(1600), 2048);
    }

    #[test]
    fn a_template_sets_the_defaults_and_keeps_the_imports() {
        let mut p = NewProject::new("a.obj", ProjectTemplate::builtins().remove(0));
        p.baked_maps.push("Body_normal.png".into());
        p.import_cameras = false;
        let mut dx = ProjectTemplate::builtins().remove(1);
        dx.set.resolution = (512, 512);
        p.use_template(dx.clone());
        assert_eq!((p.resolution, p.normal_map_format), (512, dx.project.normal_map_format));
        assert_eq!(p.baked_maps.len(), 1);
        assert!(!p.import_cameras);
    }
}

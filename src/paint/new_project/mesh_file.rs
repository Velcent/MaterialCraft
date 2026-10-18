//! Mesh files a project can start from: [`MeshFormat`] says what a file is and what it can carry; [`MeshSource`] is
//! what was read from one (the mesh, its stated unit, its cameras).

use super::error::NewProjectError;
use crate::camera::SceneCamera;
use crate::mesh::{LengthUnit, Mesh, gltf};
use std::path::Path;

/// The kind of a mesh file, by its extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshFormat {
    /// Wavefront OBJ: no unit, no cameras.
    Obj,
    /// glTF 2.0 binary: metres, cameras.
    Glb,
    /// glTF 2.0 as JSON with separate buffers (not read yet).
    GltfText,
    /// Universal Scene Description (not read yet; its scope, subdivision and frame settings depend on it).
    Usd,
    /// Autodesk FBX (not read yet).
    Fbx,
    /// Alembic (not read yet).
    Alembic,
}

impl MeshFormat {
    pub const ALL: [MeshFormat; 6] = [MeshFormat::Obj, MeshFormat::Glb, MeshFormat::GltfText, MeshFormat::Usd, MeshFormat::Fbx, MeshFormat::Alembic];

    /// The format of `path` by its extension (case-insensitive).
    pub fn of(path: &Path) -> Option<MeshFormat> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        MeshFormat::ALL.into_iter().find(|f| f.extensions().contains(&ext.as_str()))
    }

    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            MeshFormat::Obj => &["obj"],
            MeshFormat::Glb => &["glb"],
            MeshFormat::GltfText => &["gltf"],
            MeshFormat::Usd => &["usd", "usda", "usdc", "usdz"],
            MeshFormat::Fbx => &["fbx"],
            MeshFormat::Alembic => &["abc"],
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            MeshFormat::Obj => "Wavefront OBJ",
            MeshFormat::Glb => "glTF binary",
            MeshFormat::GltfText => "glTF (JSON)",
            MeshFormat::Usd => "USD",
            MeshFormat::Fbx => "FBX",
            MeshFormat::Alembic => "Alembic",
        }
    }

    /// True if MaterialCraft reads this format.
    pub fn is_readable(self) -> bool {
        matches!(self, MeshFormat::Obj | MeshFormat::Glb)
    }

    /// The formats that are read, for file dialogs.
    pub fn readable() -> impl Iterator<Item = MeshFormat> {
        MeshFormat::ALL.into_iter().filter(|f| f.is_readable())
    }

    /// True for USD files, which have their own settings (scope and variants, subdivision, frame).
    pub fn is_usd(self) -> bool {
        self == MeshFormat::Usd
    }

    /// True if files of this format can hold cameras that are read.
    pub fn has_cameras(self) -> bool {
        self == MeshFormat::Glb
    }

    /// The unit every file of this format is in; `None` if the format doesn't say.
    pub fn unit(self) -> Option<LengthUnit> {
        match self {
            MeshFormat::Glb | MeshFormat::GltfText => Some(LengthUnit::Metre),
            _ => None,
        }
    }
}

/// What was read from a mesh file.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MeshSource {
    pub mesh: Mesh,
    /// The unit the file states.
    pub unit: Option<LengthUnit>,
    /// Its cameras, in the mesh's coordinates (empty unless asked for and present).
    pub cameras: Vec<SceneCamera>,
    /// What was left out while reading (unreadable cameras).
    pub notes: Vec<String>,
}

impl MeshSource {
    /// Reads the mesh file at `path`; with `cameras`, also the cameras it holds.
    pub fn read(path: &Path, cameras: bool) -> Result<MeshSource, NewProjectError> {
        let format = MeshFormat::of(path).ok_or_else(|| NewProjectError::Unsupported { path: path.to_path_buf(), what: "not a mesh file".into() })?;
        if !format.is_readable() {
            let what = format!("{} files can't be read yet; use OBJ or glTF binary (.glb)", format.label());
            return Err(NewProjectError::Unsupported { path: path.to_path_buf(), what });
        }
        let bytes = std::fs::read(path).map_err(|e| NewProjectError::Io { path: path.to_path_buf(), message: e.to_string() })?;
        MeshSource::from_bytes(format, &bytes, cameras).map_err(|message| NewProjectError::Mesh { path: path.to_path_buf(), message })
    }

    /// Reads a file of `format` from its bytes.
    pub fn from_bytes(format: MeshFormat, bytes: &[u8], cameras: bool) -> Result<MeshSource, String> {
        let mut source = MeshSource { unit: format.unit(), ..MeshSource::default() };
        match format {
            MeshFormat::Obj => {
                let text = String::from_utf8_lossy(bytes);
                source.mesh = Mesh::read_obj(&text).map_err(|e| e.to_string())?.mesh;
            }
            MeshFormat::Glb => {
                source.mesh = Mesh::from_glb(bytes).map_err(|e| e.to_string())?;
                if cameras {
                    match gltf::read_cameras(bytes) {
                        Ok(c) => source.cameras = c,
                        Err(e) => source.notes.push(format!("cameras not read: {e}")),
                    }
                }
            }
            other => return Err(format!("{} files can't be read yet", other.label())),
        }
        Ok(source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_by_extension() {
        assert_eq!(MeshFormat::of(Path::new("a/B.OBJ")), Some(MeshFormat::Obj));
        assert_eq!(MeshFormat::of(Path::new("scene.usdz")), Some(MeshFormat::Usd));
        assert_eq!(MeshFormat::of(Path::new("x.png")), None);
        assert_eq!(MeshFormat::of(Path::new("noext")), None);
        assert!(MeshFormat::Usd.is_usd() && !MeshFormat::Usd.is_readable());
        assert_eq!(MeshFormat::readable().collect::<Vec<_>>(), vec![MeshFormat::Obj, MeshFormat::Glb]);
    }

    #[test]
    fn unreadable_formats_are_refused_with_their_name() {
        let e = MeshSource::read(Path::new("/nonexistent/thing.fbx"), true).unwrap_err();
        assert!(e.to_string().contains("FBX"), "{e}");
        assert!(matches!(MeshSource::read(Path::new("/nonexistent/thing.obj"), true), Err(NewProjectError::Io { .. })));
    }
}

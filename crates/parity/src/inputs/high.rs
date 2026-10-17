//! The mesh Painter baked from: [`HighSource::resolve`] decides where it comes from (cheap, no loading),
//! [`HighMesh::load`] reads it and moves it into Painter's normalised space, like the project's low mesh.

use super::finder::{MeshFinder, is_mesh_name};
use super::low::bounds;
use crate::settings::HighPolyRequest;
use crate::subject::CaseError;
use materialcraft::bake;
use materialcraft_substance::decode::assbin::Normalization;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// High-poly files above this size are skipped (the baker keeps every triangle in memory).
pub const MAX_HIGH_POLY_BYTES: u64 = 1 << 30;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HighSource {
    /// Painter baked with "use low poly mesh as high poly mesh": the whole project mesh.
    LowAsHigh,
    /// High-poly OBJ files found below the project folder.
    Files(Vec<PathBuf>),
}

impl HighSource {
    pub fn resolve(request: &HighPolyRequest, project_dir: &Path) -> Result<HighSource, CaseError> {
        if request.low_as_high {
            return Ok(HighSource::LowAsHigh);
        }
        if request.files.is_empty() {
            return Err(CaseError::skipped("no high-poly mesh recorded and low-as-high is off"));
        }
        let finder = MeshFinder::new(project_dir);
        let mut found = Vec::new();
        for name in &request.files {
            if !is_mesh_name(name) {
                return Err(CaseError::skipped(format!("high-poly '{name}' is not an OBJ/FBX file")));
            }
            let path = finder
                .find(name)
                .ok_or_else(|| CaseError::skipped(format!("high-poly '{name}' not found below the project folder")))?;
            if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("obj")) {
                return Err(CaseError::skipped(format!(
                    "high-poly '{name}' found, but FBX cannot be read yet"
                )));
            }
            found.push(path);
        }
        Ok(HighSource::Files(found))
    }

    pub fn label(&self) -> String {
        match self {
            HighSource::LowAsHigh => "low as high".into(),
            HighSource::Files(f) => f
                .iter()
                .filter_map(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join(", "),
        }
    }
}

pub struct HighMesh {
    pub mesh: bake::Mesh,
    pub source: HighSource,
    /// Notes about how it was placed (normalisation, bounding-box check).
    pub notes: Vec<String>,
}

impl HighMesh {
    /// `whole_low`: the project's whole mesh in Painter's normalised space.
    pub fn load(
        source: &HighSource,
        whole_low: &bake::Mesh,
        normalization: Option<Normalization>,
    ) -> Result<HighMesh, CaseError> {
        let HighSource::Files(files) = source else {
            return Ok(HighMesh {
                mesh: whole_low.clone(),
                source: source.clone(),
                notes: Vec::new(),
            });
        };
        let mut mesh = bake::Mesh::default();
        for path in files {
            append(&mut mesh, read_obj(path)?);
        }
        let mut notes = Vec::new();
        match normalization {
            Some(n) => mesh
                .tris
                .iter_mut()
                .flat_map(|t| t.p.iter_mut())
                .for_each(|p| *p = n.apply(*p)),
            None => notes.push("project has no normalisation record; high mesh used unmoved".into()),
        }
        if let (Some(high), Some(low)) = (bounds(&mesh), bounds(whole_low)) {
            let check = BoundsCheck::of(high, low);
            if !check.plausible() {
                notes.push(format!(
                    "high and low bounds differ (centre off by {:.1} %, size ratio {:.2}); placement may be wrong",
                    check.offset * 100.0,
                    check.ratio
                ));
            }
        }
        Ok(HighMesh {
            mesh,
            source: source.clone(),
            notes,
        })
    }
}

/// High meshes of one project, loaded once and shared by its texture sets (many sets bake from one file).
#[derive(Default)]
pub struct HighCache {
    loaded: RefCell<Vec<(HighSource, Rc<HighMesh>)>>,
}

impl HighCache {
    pub fn get_or_load(
        &self,
        source: &HighSource,
        whole_low: &bake::Mesh,
        normalization: Option<Normalization>,
    ) -> Result<Rc<HighMesh>, CaseError> {
        if let Some((_, mesh)) = self.loaded.borrow().iter().find(|(s, _)| s == source) {
            return Ok(Rc::clone(mesh));
        }
        let mesh = Rc::new(HighMesh::load(source, whole_low, normalization)?);
        self.loaded.borrow_mut().push((source.clone(), Rc::clone(&mesh)));
        Ok(mesh)
    }
}

fn read_obj(path: &Path) -> Result<bake::Mesh, CaseError> {
    let fail = |e: String| CaseError::failed(format!("{}: {e}", path.display()));
    let len = std::fs::metadata(path).map_err(|e| fail(e.to_string()))?.len();
    if len > MAX_HIGH_POLY_BYTES {
        return Err(CaseError::skipped(format!(
            "{} is too large ({len} bytes)",
            path.display()
        )));
    }
    let bytes = std::fs::read(path).map_err(|e| fail(e.to_string()))?;
    bake::parse_obj(&String::from_utf8_lossy(&bytes)).map_err(fail)
}

fn append(into: &mut bake::Mesh, mut more: bake::Mesh) {
    let offset = into.groups.len() as u32;
    more.tris.iter_mut().for_each(|t| t.group += offset);
    into.groups.append(&mut more.groups);
    into.tris.append(&mut more.tris);
}

/// How well two bounding boxes agree: centre offset over the low diagonal, and the diagonal ratio.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundsCheck {
    pub offset: f32,
    pub ratio: f32,
}

impl BoundsCheck {
    pub fn of(high: ([f32; 3], [f32; 3]), low: ([f32; 3], [f32; 3])) -> BoundsCheck {
        let diag = |b: ([f32; 3], [f32; 3])| (0..3).map(|k| (b.1[k] - b.0[k]).powi(2)).sum::<f32>().sqrt();
        let centre = |b: ([f32; 3], [f32; 3]), k: usize| 0.5 * (b.0[k] + b.1[k]);
        let low_diag = diag(low).max(1e-9);
        let offset = (0..3)
            .map(|k| (centre(high, k) - centre(low, k)).powi(2))
            .sum::<f32>()
            .sqrt();
        BoundsCheck {
            offset: offset / low_diag,
            ratio: diag(high) / low_diag,
        }
    }

    /// Within 10 % of the size and position (a high-poly mesh may stick out a little).
    pub fn plausible(&self) -> bool {
        self.offset < 0.1 && (0.9..1.1).contains(&self.ratio)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_low_as_high_and_explains_skips() {
        let dir = std::env::temp_dir();
        let low = HighPolyRequest {
            low_as_high: true,
            files: vec![],
        };
        assert_eq!(HighSource::resolve(&low, &dir), Ok(HighSource::LowAsHigh));
        let none = HighPolyRequest::default();
        assert!(matches!(HighSource::resolve(&none, &dir), Err(CaseError::Skipped(_))));
        let missing = HighPolyRequest {
            low_as_high: false,
            files: vec!["surely-missing-mcparity.obj".into()],
        };
        let err = HighSource::resolve(&missing, &dir.join("mcparity-none")).unwrap_err();
        assert!(err.reason().contains("not found"));
    }

    #[test]
    fn checks_bounds() {
        let unit = ([0.0; 3], [1.0; 3]);
        assert!(BoundsCheck::of(unit, unit).plausible());
        assert!(!BoundsCheck::of(([0.0; 3], [2.0; 3]), unit).plausible());
        assert!(!BoundsCheck::of(([0.5; 3], [1.5; 3]), unit).plausible());
    }
}

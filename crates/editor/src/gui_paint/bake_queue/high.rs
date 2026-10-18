//! [`HighScenes`]: the high meshes a queue bakes from, loaded once per distinct source and shared by every set that
//! uses it (many sets bake from one high-poly file).

use materialcraft::bake::{self, HighScene, MeshFinder, is_mesh_name};
use materialcraft::mesh::Mesh;
use materialcraft::paint::bake_config::HighPoly;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// High-poly files above this size are refused (the baker keeps every triangle in memory).
const MAX_FILE_BYTES: u64 = 1 << 30;

pub struct HighScenes {
    low: Arc<Mesh>,
    /// Where file names without a usable path are looked for (the document's folder).
    base_dir: Option<PathBuf>,
    loaded: Vec<(HighPoly, Arc<HighScene>)>,
}

impl HighScenes {
    pub fn new(low: Arc<Mesh>, base_dir: Option<PathBuf>) -> HighScenes {
        HighScenes { low, base_dir, loaded: Vec::new() }
    }

    /// The scene of `high`, loading it the first time.
    pub fn get(&mut self, high: &HighPoly) -> Result<Arc<HighScene>, String> {
        if let Some((_, scene)) = self.loaded.iter().find(|(h, _)| h == high) {
            return Ok(Arc::clone(scene));
        }
        let scene = Arc::new(self.load(high)?);
        self.loaded.push((high.clone(), Arc::clone(&scene)));
        Ok(scene)
    }

    fn load(&self, high: &HighPoly) -> Result<HighScene, String> {
        if high.low_as_high {
            return Ok(HighScene::from_mesh(&self.low));
        }
        if high.files.is_empty() {
            return Err("no high-poly mesh is set and \"use the low-poly mesh\" is off".into());
        }
        let mut mesh = bake::Mesh::default();
        for name in &high.files {
            mesh.append(read_obj(&self.locate(name)?)?);
        }
        if let Some(p) = high.placement {
            mesh.tris.iter_mut().flat_map(|t| t.p.iter_mut()).for_each(|v| *v = p.apply(*v));
        }
        Ok(mesh.to_high())
    }

    /// `name` as given if that file exists, else searched below the document's folder by its file name (and its
    /// folders, for a path recorded on another machine).
    fn locate(&self, name: &str) -> Result<PathBuf, String> {
        let given = Path::new(name.trim_start_matches("file://"));
        if given.is_file() {
            return Ok(given.to_path_buf());
        }
        let file = name.rsplit(['/', '\\']).next().unwrap_or(name);
        if !is_mesh_name(file) {
            return Err(format!("high-poly '{file}' is not an OBJ or FBX file"));
        }
        let dir = self.base_dir.as_deref().ok_or_else(|| format!("high-poly '{file}' not found (the document has no folder to search)"))?;
        let path = MeshFinder::new(dir).find(name).ok_or_else(|| format!("high-poly '{file}' not found below {}", dir.display()))?;
        if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("obj")) {
            return Err(format!("high-poly '{file}' found, but FBX files cannot be read yet"));
        }
        Ok(path)
    }
}

fn read_obj(path: &Path) -> Result<bake::Mesh, String> {
    let fail = |e: String| format!("{}: {e}", path.display());
    let len = std::fs::metadata(path).map_err(|e| fail(e.to_string()))?.len();
    if len > MAX_FILE_BYTES {
        return Err(fail(format!("too large ({len} bytes)")));
    }
    let bytes = std::fs::read(path).map_err(|e| fail(e.to_string()))?;
    bake::parse_obj(&String::from_utf8_lossy(&bytes)).map_err(fail)
}

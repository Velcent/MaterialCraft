//! [`ShaderFile`]: a declaration with its GLSL sources, and [`ShaderLibrary`]: the shader files a session offers
//! (the built-in default first, then files loaded from folders).

use super::decl::{DeclError, ShaderDecl};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The built-in default shader: the viewport's PBR program.
const BUILTIN_JSON: &str = include_str!("builtin/pbr.json");
const BUILTIN_VERT: &str = include_str!("builtin/pbr.vert");
const BUILTIN_FRAG: &str = include_str!("builtin/pbr.frag");

/// The id of the built-in default shader.
pub const DEFAULT_SHADER: &str = "materialcraft.pbr";

/// Extension of shader declarations in a folder.
pub const DECL_EXTENSION: &str = "mcshader";

/// A shader file: its declaration and GLSL sources, with a fingerprint of the declaration (instances made from a
/// different fingerprint are outdated, VS-05: their parameters may have changed; a GLSL-only change keeps every
/// value meaningful and outdates nothing).
#[derive(Debug, Clone, PartialEq)]
pub struct ShaderFile {
    pub decl: ShaderDecl,
    pub vertex_src: String,
    pub fragment_src: String,
    pub fingerprint: u64,
    /// A fingerprint of the declaration and both GLSL sources: equal for files that compile to the same program.
    pub program_key: u64,
    /// Where it was loaded from; `None` for the built-in shader.
    pub source: Option<PathBuf>,
}

/// Why a shader file could not be loaded.
#[derive(Debug)]
pub enum LoadError {
    Io(PathBuf, std::io::Error),
    Decl(PathBuf, DeclError),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            LoadError::Io(p, e) => write!(f, "{}: {e}", p.display()),
            LoadError::Decl(p, e) => write!(f, "{}: {e}", p.display()),
        }
    }
}

impl std::error::Error for LoadError {}

impl ShaderFile {
    /// A shader file from its parts.
    pub fn new(decl: ShaderDecl, vertex_src: String, fragment_src: String, source: Option<PathBuf>) -> ShaderFile {
        let mut h = Fnv::default();
        h.write(decl.to_json().to_string().as_bytes());
        let fingerprint = h.0;
        for src in [&vertex_src, &fragment_src] {
            h.write(&[0]);
            h.write(src.as_bytes());
        }
        ShaderFile { fingerprint, program_key: h.0, decl, vertex_src, fragment_src, source }
    }

    /// The built-in default shader (today's viewport program).
    pub fn builtin() -> ShaderFile {
        match ShaderDecl::parse(BUILTIN_JSON) {
            Ok(decl) => ShaderFile::new(decl, BUILTIN_VERT.to_string(), BUILTIN_FRAG.to_string(), None),
            // The built-in declaration is checked by a test; an empty one keeps the program working regardless.
            Err(_) => ShaderFile::new(
                ShaderDecl {
                    id: DEFAULT_SHADER.into(),
                    name: "PBR".into(),
                    revision: 1,
                    vertex: "pbr.vert".into(),
                    fragment: "pbr.frag".into(),
                    params: Vec::new(),
                },
                BUILTIN_VERT.to_string(),
                BUILTIN_FRAG.to_string(),
                None,
            ),
        }
    }

    /// Loads `<name>.mcshader` and the GLSL files it names (relative to it).
    pub fn load(path: &Path) -> Result<ShaderFile, LoadError> {
        let read = |p: &Path| std::fs::read_to_string(p).map_err(|e| LoadError::Io(p.to_path_buf(), e));
        let decl = ShaderDecl::parse(&read(path)?).map_err(|e| LoadError::Decl(path.to_path_buf(), e))?;
        let dir = path.parent().unwrap_or(Path::new(""));
        let vertex_src = read(&dir.join(&decl.vertex))?;
        let fragment_src = read(&dir.join(&decl.fragment))?;
        Ok(ShaderFile::new(decl, vertex_src, fragment_src, Some(path.to_path_buf())))
    }

    pub fn id(&self) -> &str {
        &self.decl.id
    }
}

/// The shader files to choose from. The built-in default is always first; a loaded file with the same id as
/// one already present replaces it, except the built-in one.
#[derive(Debug, Clone)]
pub struct ShaderLibrary {
    files: Vec<Arc<ShaderFile>>,
}

impl Default for ShaderLibrary {
    fn default() -> Self {
        ShaderLibrary { files: vec![Arc::new(ShaderFile::builtin())] }
    }
}

impl ShaderLibrary {
    pub fn files(&self) -> &[Arc<ShaderFile>] {
        &self.files
    }

    /// The built-in default shader.
    pub fn default_shader(&self) -> Arc<ShaderFile> {
        match self.files.first() {
            Some(f) => f.clone(),
            None => Arc::new(ShaderFile::builtin()),
        }
    }

    pub fn find(&self, id: &str) -> Option<&Arc<ShaderFile>> {
        self.files.iter().find(|f| f.id() == id)
    }

    /// The shader `id`, or the built-in default when it is not in the library (a document made with a shader
    /// file this machine doesn't have).
    pub fn find_or_default(&self, id: &str) -> Arc<ShaderFile> {
        self.find(id).cloned().unwrap_or_else(|| self.default_shader())
    }

    /// Adds `file`, replacing a loaded file with the same id. Returns false (and adds nothing) for the built-in
    /// id.
    pub fn add(&mut self, file: ShaderFile) -> bool {
        if file.id() == DEFAULT_SHADER {
            return false;
        }
        match self.files.iter_mut().find(|f| f.id() == file.id()) {
            Some(slot) => *slot = Arc::new(file),
            None => self.files.push(Arc::new(file)),
        }
        true
    }

    /// Loads every `*.mcshader` in `dir` (not recursive, in name order). A missing folder loads nothing; files
    /// that fail are skipped and their errors returned.
    pub fn load_folder(&mut self, dir: &Path) -> Vec<LoadError> {
        let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
        let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|e| e == DECL_EXTENSION)).collect();
        paths.sort();
        let mut errors = Vec::new();
        for p in paths {
            match ShaderFile::load(&p) {
                Ok(f) => {
                    self.add(f);
                }
                Err(e) => errors.push(e),
            }
        }
        errors
    }
}

/// FNV-1a, 64 bit: a stable fingerprint (not a security hash).
struct Fnv(u64);

impl Default for Fnv {
    fn default() -> Self {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
}

impl Fnv {
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 ^= u64::from(*b);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }
}

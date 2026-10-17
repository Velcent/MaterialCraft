//! [`TrayMaterials`]: the materials of the asset libraries (the tray's folders: the user library, the starter
//! library, further folders), each with its name, tags and its [`Look`] from a small render of its graph.

use super::look::{COMPARED, Look, LookBuilder};
use materialcraft::assets::{Catalogue, Format, LibraryRoot, Listing, RootKind, Usage};
use materialcraft::diag::NullSink;
use materialcraft::material_fill::{FolderResolver, Material, RenderedMaterial};
use std::path::PathBuf;

/// Side of the render a material's look is measured on.
pub const LOOK_RENDER: usize = 64;

/// One material of ours.
#[derive(Debug, Clone, PartialEq)]
pub struct TrayMaterial {
    /// The library reference a fill stores (`materials/iron-rusted.mca`).
    pub reference: String,
    pub name: String,
    pub tags: Vec<String>,
    /// `None` when the graph cannot be read or rendered.
    pub look: Option<Look>,
}

/// The materials of a list of library folders; a reference found in several folders is the first folder's,
/// as a [`FolderResolver`] over the same folders resolves it.
#[derive(Debug, Clone, Default)]
pub struct TrayMaterials {
    folders: Vec<PathBuf>,
    materials: Vec<TrayMaterial>,
}

impl TrayMaterials {
    /// Scans `folders` and renders every material graph found (a few milliseconds each).
    pub fn load(folders: &[PathBuf]) -> TrayMaterials {
        let mut catalogue = Catalogue::new();
        let mut materials: Vec<TrayMaterial> = Vec::new();
        for (i, folder) in folders.iter().enumerate() {
            let root = LibraryRoot::folder(
                format!("library-{i}"),
                folder.display().to_string(),
                RootKind::Extra,
                folder.clone(),
            );
            let id = root.id.clone();
            catalogue.add_root(root);
            catalogue.load(&id, &Listing::read(folder, &NullSink), &NullSink);
            for asset in catalogue
                .assets(&id)
                .filter(|a| a.usage == Usage::Material && a.format == Format::Graph)
            {
                if materials.iter().any(|m| m.reference == asset.path) {
                    continue;
                }
                materials.push(TrayMaterial {
                    reference: asset.path.clone(),
                    name: asset.name.clone(),
                    tags: asset.tags.clone(),
                    look: None,
                });
            }
        }
        let resolver = FolderResolver::new(folders.iter().cloned());
        render_looks(&mut materials, &resolver);
        materials.sort_by(|a, b| a.reference.cmp(&b.reference));
        TrayMaterials {
            folders: folders.to_vec(),
            materials,
        }
    }

    /// Materials given directly (tests, callers that measured them already).
    pub fn from_materials(folders: Vec<PathBuf>, materials: Vec<TrayMaterial>) -> TrayMaterials {
        TrayMaterials { folders, materials }
    }

    pub fn materials(&self) -> &[TrayMaterial] {
        &self.materials
    }

    pub fn is_empty(&self) -> bool {
        self.materials.is_empty()
    }

    pub fn folders(&self) -> &[PathBuf] {
        &self.folders
    }

    /// Finds the graphs of the references, for embedding them into a document.
    pub fn resolver(&self) -> FolderResolver {
        FolderResolver::new(self.folders.iter().cloned())
    }
}

/// Renders the looks on a few threads.
fn render_looks(materials: &mut [TrayMaterial], resolver: &FolderResolver) {
    let threads = std::thread::available_parallelism().map_or(2, |n| (n.get() / 2).clamp(1, 8));
    let chunk = materials.len().div_ceil(threads).max(1);
    std::thread::scope(|scope| {
        for part in materials.chunks_mut(chunk) {
            scope.spawn(move || {
                for m in part {
                    m.look = look_of(resolver, &m.reference);
                }
            });
        }
    });
}

/// The look of material `reference` rendered at [`LOOK_RENDER`] pixels.
pub fn look_of(resolver: &FolderResolver, reference: &str) -> Option<Look> {
    let material = Material::load(resolver, reference, &[]).ok()?;
    let rendered = RenderedMaterial::render(&material, LOOK_RENDER).ok()?;
    Some(rendered_look(&rendered))
}

/// The statistics of every compared channel of a render.
pub fn rendered_look(rendered: &RenderedMaterial) -> Look {
    let mut b = LookBuilder::default();
    for (channel, _) in COMPARED {
        let Some(texture) = rendered.channel(channel) else {
            continue;
        };
        let levels = texture.levels();
        let base = levels.level(0);
        let (w, h) = base.size();
        for y in 0..h {
            for x in 0..w {
                b.add(channel, base.texel(x, y), 1.0);
            }
        }
    }
    b.build()
}

//! [`import_spp`]: the translate stage's façade. Opens a project, binds texture sets, builds each one and keeps
//! the untranslated parts; the report goes into [`Document::import_report`].

use super::baked_maps::BakedMaps;
use super::cache_channels::CacheChannels;
use super::error::ImportError;
use super::foreign::ForeignFiles;
use super::layers::{BlendTable, LayerImport, ProjectImages};
use super::normal_conventions::NormalConventions;
use super::report::Reporter;
use super::set_builder::SetBuilder;
use super::set_layers::SetLayers;
use super::texture_sets::SetBinding;
use crate::decode::assbin::{AssbinGeometry, AssbinScene};
use crate::project::{SppProject, paths};
use materialcraft::mesh::Mesh;
use materialcraft::paint::{Document, Fidelity};
use std::collections::BTreeSet;
use std::path::Path;

/// Share of the progress bar spent on texture sets (the rest: mesh, document, foreign files).
const SETS_START: f32 = 0.1;
const SETS_END: f32 = 0.9;

/// Imports a `.spp` as a flattened document. `progress` gets a fraction 0..1 and a short message.
pub fn import_spp(path: &Path, progress: &mut dyn FnMut(f32, &str)) -> Result<Document, ImportError> {
    Importer::open(path)?.run(progress)
}

/// One import in progress.
pub struct Importer {
    project: SppProject,
}

impl Importer {
    pub fn open(path: &Path) -> Result<Importer, ImportError> {
        Ok(Importer {
            project: SppProject::open(path)?,
        })
    }

    pub fn run(&self, progress: &mut dyn FnMut(f32, &str)) -> Result<Document, ImportError> {
        let mut report = Reporter::new(self.source_label());
        progress(0.0, "reading mesh");
        let scene = self.project.read(paths::SCENE)?;
        let decode = |part| move |source| ImportError::Decode { part, source };
        let mut mesh = AssbinGeometry::parse(&scene).map_err(decode("mesh"))?;
        let materials = AssbinScene::parse(&scene).map_err(decode("mesh"))?.materials;
        report.mesh(mesh.parts.len(), mesh.parts.iter().map(|p| p.triangles.len()).sum());
        progress(0.05, "reading layer document");
        let records = self.project.document_materials()?;
        let normals = NormalConventions::from_settings(self.project.document_settings().ok().as_ref());
        report.normals(&normals);
        let model = match self.project.model() {
            Ok((document, _)) => Some(document),
            Err(e) => {
                let note = format!("layer document could not be read ({e}); layer stacks stay flattened");
                report.add("layer document", Fidelity::Approximated, note);
                None
            }
        };
        let table = BlendTable::painter();
        let images = ProjectImages::new(&self.project);
        let import = LayerImport {
            table: &table,
            images: &images,
        };
        let layers = SetLayers {
            import: &import,
            document: model.as_ref(),
        };
        let cache = CacheChannels::new(&self.project);
        let baked = BakedMaps::new(&self.project);
        let binding = SetBinding::bind(&materials, &records, &cache.sets());
        let builder = SetBuilder {
            cache: &cache,
            baked: &baked,
            normals: &normals,
            layers: &layers,
        };
        let mut translated = BTreeSet::new();
        let mut sets = Vec::with_capacity(binding.sets.len());
        let slot_names = mesh.materials.clone();
        for (i, source) in binding.sets.iter().enumerate() {
            let at = SETS_START + (SETS_END - SETS_START) * i as f32 / binding.sets.len() as f32;
            progress(at, &format!("texture set {}", source.record.name));
            merge_slots(
                &mut mesh,
                source.slot,
                &source.merged_slots,
                &slot_names,
                &mut report,
                &source.record.name,
            );
            sets.push(builder.build(source, &mut report, &mut translated));
        }
        binding.unused.iter().for_each(|r| report.unused_material(&r.name));
        progress(SETS_END, "keeping original files");
        let kept = ForeignFiles::new(&self.project).collect(&translated);
        report.foreign(&kept);
        let mut doc = Document::new(mesh);
        doc.texture_sets = sets;
        doc.height_filter = normals.height_filter;
        doc.foreign = kept.files;
        for problem in doc.problems() {
            report.add("document check", Fidelity::Approximated, problem.to_string());
        }
        doc.import_report = Some(report.finish());
        progress(1.0, "imported");
        Ok(doc)
    }

    /// `substance-painter <version>: <file name>`.
    fn source_label(&self) -> String {
        let version = self
            .project
            .settings()
            .ok()
            .flatten()
            .and_then(|s| s.last_saved_with.or(s.created_with))
            .map_or("(version unknown)".to_string(), |v| v.to_string());
        let file = self
            .project
            .path()
            .file_name()
            .map_or(String::new(), |n| n.to_string_lossy().into_owned());
        format!("substance-painter {version}: {file}")
    }
}

/// Points mesh parts of collapsed slots at the set's slot, so they show the shared texture set.
fn merge_slots(mesh: &mut Mesh, slot: usize, merged: &[usize], names: &[String], report: &mut Reporter, set: &str) {
    for &m in merged {
        mesh.parts
            .iter_mut()
            .filter(|p| p.material == m)
            .for_each(|p| p.material = slot);
        let from = names.get(m).map_or("?", |s| s.as_str());
        report.add(
            format!("mesh material \"{from}\""),
            Fidelity::Exact,
            format!("shares texture set \"{set}\" (collapsed in Painter); its parts now use that set's slot"),
        );
    }
}

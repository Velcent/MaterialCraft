//! [`build`]: every [`Recipe`] made (in parallel) and written into a fresh folder, which then replaces the target
//! in one rename, so a library is never seen half-built.

use crate::manifest;
use crate::{Made, Recipe, StarterError, alphas, brushes, environment, masks, materials, patterns, textures};
use materialcraft::assets::{AssetStore, Draft, LibraryRoot, RootKind, Usage, create_standard_layout};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// How to build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// Side of the grunge and noise textures, in pixels.
    pub texture_size: usize,
    /// Threads making assets at once (at least 1). The result doesn't depend on it.
    pub jobs: usize,
}

impl Default for Options {
    fn default() -> Options {
        Options { texture_size: 1024, jobs: 4 }
    }
}

/// What a build wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Built {
    pub dir: PathBuf,
    /// Root-relative path and usage of every asset, in build order.
    pub assets: Vec<(String, Usage)>,
}

/// Every starter asset, in build order: materials, smart masks, textures, alphas, brushes, the environment.
pub fn recipes() -> Vec<Box<dyn Recipe>> {
    let mut all = materials::recipes();
    all.extend(masks::recipes());
    all.extend(textures::recipes());
    all.extend(patterns::recipes());
    all.extend(alphas::recipes());
    all.extend(brushes::recipes());
    all.extend(environment::recipes());
    all
}

/// Builds the starter library into `dir`. An existing `dir` must be a starter library (it is replaced) or empty.
pub fn build(dir: &Path, options: &Options) -> Result<Built, StarterError> {
    if dir.exists() && !manifest::is_starter(dir) && !is_empty_dir(dir) {
        return Err(StarterError::NotStarter(dir.to_path_buf()));
    }
    let staging = sibling(dir, "building");
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|e| StarterError::io(&staging, e))?;
    }
    fs::create_dir_all(&staging).map_err(|e| StarterError::io(&staging, e))?;
    let root = LibraryRoot::folder("starter-build", "Starter (building)", RootKind::User, &staging);
    create_standard_layout(&root).map_err(|e| StarterError::io(&staging, e))?;
    let store = AssetStore::new();

    let recipes = recipes();
    let made = make_all(&recipes, options)?;
    let mut assets = Vec::with_capacity(made.len());
    for (recipe, made) in recipes.iter().zip(made) {
        let path = recipe.path();
        shippable(&path, &made)?;
        write(&store, &root, &path, &made)?;
        let usage = made.sidecar.usage.unwrap_or(Usage::Texture);
        assets.push((path, usage));
    }
    manifest::write(&staging, &assets)?;

    if dir.exists() {
        fs::remove_dir_all(dir).map_err(|e| StarterError::io(dir, e))?;
    }
    fs::rename(&staging, dir).map_err(|e| StarterError::io(dir, e))?;
    Ok(Built { dir: dir.to_path_buf(), assets })
}

/// Makes every recipe on `options.jobs` threads; the results come back in recipe order.
fn make_all(recipes: &[Box<dyn Recipe>], options: &Options) -> Result<Vec<Made>, StarterError> {
    let next = AtomicUsize::new(0);
    let slots: Vec<Mutex<Option<Result<Made, StarterError>>>> = recipes.iter().map(|_| Mutex::new(None)).collect();
    std::thread::scope(|s| {
        for _ in 0..options.jobs.clamp(1, 64) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let (Some(recipe), Some(slot)) = (recipes.get(i), slots.get(i)) else { break };
                    let result = recipe.make(options);
                    if let Ok(mut slot) = slot.lock() {
                        *slot = Some(result);
                    }
                }
            });
        }
    });
    recipes
        .iter()
        .zip(slots)
        .map(|(r, slot)| match slot.into_inner() {
            Ok(Some(made)) => made,
            _ => Err(StarterError::recipe(&r.path(), "was not made (a worker stopped)")),
        })
        .collect()
}

/// Refuses an asset taken from a user's imported project (roadmap step 46): those stay in the user's own library and
/// are never shipped, whatever their origin.
fn shippable(path: &str, made: &Made) -> Result<(), StarterError> {
    match made.sidecar.provenance.is_imported() {
        true => Err(StarterError::recipe(path, "comes from a user's imported project; the starter library never ships those")),
        false => Ok(()),
    }
}

/// Writes the asset, its sidecar and its thumbnail into `root` through the asset store.
fn write(store: &AssetStore, root: &LibraryRoot, path: &str, made: &Made) -> Result<(), StarterError> {
    let draft = Draft { bytes: &made.bytes, sidecar: Some(&made.sidecar), thumbnail: made.thumbnail.as_deref() };
    store.create(root, path, &draft).map(|_| ()).map_err(|e| StarterError::recipe(path, e))
}

/// `dir` with `.<suffix>` appended to its last component.
fn sibling(dir: &Path, suffix: &str) -> PathBuf {
    let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "starter".into());
    dir.with_file_name(format!("{name}.{suffix}"))
}

fn is_empty_dir(dir: &Path) -> bool {
    fs::read_dir(dir).is_ok_and(|mut d| d.next().is_none())
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft::assets::{Provenance, Sidecar};

    #[test]
    fn assets_from_imported_projects_are_never_shipped() {
        let made = |provenance| Made { bytes: vec![], sidecar: Sidecar { provenance, ..Sidecar::default() }, thumbnail: None };
        let imported = Provenance::Imported { project: "Cup.spp".into(), origin: "stored in the project".into(), shareable: true };
        assert!(shippable("alphas/x.png", &made(imported)).is_err());
        assert!(shippable("alphas/x.png", &made(Provenance::Generator { generator: "g".into(), preset: None })).is_ok());
    }
}

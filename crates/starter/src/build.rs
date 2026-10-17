//! [`build`]: every [`Recipe`] made (in parallel) and written into a fresh folder, which then replaces the target
//! in one rename, so a library is never seen half-built.

use crate::manifest;
use crate::{Made, Recipe, StarterError, alphas, environment, masks, materials, textures};
use materialcraft::assets::{LibraryRoot, RootKind, SIDECAR_SUFFIX, THUMBNAIL_SUFFIX, Usage, create_standard_layout};
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

/// Every starter asset, in build order: materials, smart masks, textures, alphas, the environment.
pub fn recipes() -> Vec<Box<dyn Recipe>> {
    let mut all = materials::recipes();
    all.extend(masks::recipes());
    all.extend(textures::recipes());
    all.extend(alphas::recipes());
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
    create_standard_layout(&LibraryRoot::folder("starter-build", "Starter (building)", RootKind::User, &staging)).map_err(|e| StarterError::io(&staging, e))?;

    let recipes = recipes();
    let made = make_all(&recipes, options)?;
    let mut assets = Vec::with_capacity(made.len());
    for (recipe, made) in recipes.iter().zip(made) {
        let path = recipe.path();
        write(&staging, &path, &made)?;
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

/// Writes the asset, its sidecar and its thumbnail under `root`.
fn write(root: &Path, path: &str, made: &Made) -> Result<(), StarterError> {
    let file = root.join(path);
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent).map_err(|e| StarterError::io(parent, e))?;
    }
    let put = |p: PathBuf, bytes: &[u8]| fs::write(&p, bytes).map_err(|e| StarterError::io(p, e));
    put(file, &made.bytes)?;
    put(root.join(format!("{path}{SIDECAR_SUFFIX}")), format!("{}\n", made.sidecar.to_json().to_string_pretty()).as_bytes())?;
    if let Some(png) = &made.thumbnail {
        put(root.join(format!("{path}{THUMBNAIL_SUFFIX}")), png)?;
    }
    Ok(())
}

/// `dir` with `.<suffix>` appended to its last component.
fn sibling(dir: &Path, suffix: &str) -> PathBuf {
    let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "starter".into());
    dir.with_file_name(format!("{name}.{suffix}"))
}

fn is_empty_dir(dir: &Path) -> bool {
    fs::read_dir(dir).is_ok_and(|mut d| d.next().is_none())
}

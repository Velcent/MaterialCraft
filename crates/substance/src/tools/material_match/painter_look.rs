//! [`PainterLooks`]: the look of each Substance material fill in Painter's saved result. Per texture set with a
//! texture cache, the layer stack is imported (Substance mask generators replaced by ours, so masks can be
//! computed), and every visible layer filling with the material gives a weight per texel: the product of its
//! mask and its folders' masks, on a small grid, on the mesh's texels. Painter's cached channels are sampled
//! with those weights. Layers above that cover the material are not taken out, and a mask that cannot be
//! computed counts as fully open (the evidence says so): the result is where the material is most likely seen.
//!
//! Only data in the project is read (the layer document, the texture cache, the mesh and baked maps); the
//! material's own graph is never evaluated.

use super::look::{COMPARED, Look, LookBuilder};
use crate::model::PainterDocument;
use crate::project::SppProject;
use crate::tools::verify_layers::prepared::PreparedSet;
use crate::tools::verify_layers::project_mesh::ProjectMesh;
use crate::translate::BakedMaps;
use crate::translate::cache_channels::CacheChannels;
use crate::translate::layers::effect_data::SUBSTANCE_PREFIX;
use crate::translate::layers::{BlendTable, ProjectImages};
use crate::translate::replacements::{Choice, ProceduralPlan};
use materialcraft::generators;
use materialcraft::paint::layer_preview::PreviewGrid;
use materialcraft::paint::{EffectKind, Layer, LayerKind, NO_FILL_CONTEXT, TextureSet};

/// Longer edge of the grid masks are evaluated on.
const GRID: u32 = 96;

/// Texels whose weight is below this are left out.
const MIN_WEIGHT: f32 = 0.05;

/// What Painter's saved result says about one material.
#[derive(Debug, Clone, PartialEq)]
pub struct PainterLook {
    pub look: Look,
    /// Layers sampled, and how many of them had a mask (of their own or a folder's) that could not be computed.
    pub layers: usize,
    pub open_masks: usize,
    /// Share of the sampled sets' mesh texels the material covers (mask-weighted).
    pub coverage: f32,
}

impl PainterLook {
    /// 0.25..1: how much the look says about the material. A layer whose mask could not be computed was
    /// sampled over its whole set, which mostly shows other layers.
    pub fn confidence(&self) -> f32 {
        let open = self.open_masks as f32 / self.layers.max(1) as f32;
        1.0 - 0.75 * open.clamp(0.0, 1.0)
    }
}

/// The looks of the material fills of one project, by Painter graph name.
#[derive(Debug, Clone, Default)]
pub struct PainterLooks {
    pub looks: Vec<(String, PainterLook)>,
}

impl PainterLooks {
    pub fn get(&self, name: &str) -> Option<&PainterLook> {
        self.looks.iter().find(|(n, _)| n == name).map(|(_, l)| l)
    }

    /// Measures materials `names` in `project` (whose layer document is `document`).
    pub fn measure(project: &SppProject, document: &PainterDocument, names: &[String]) -> PainterLooks {
        let mut acc: Vec<(String, Tally)> = names.iter().map(|n| (n.clone(), Tally::default())).collect();
        if names.is_empty() {
            return PainterLooks::default();
        }
        let cache = CacheChannels::new(project);
        let images = ProjectImages::new(project);
        let mesh = ProjectMesh::read(project);
        let table = BlendTable::painter();
        let baked = BakedMaps::new(project);
        let records = project.document_materials().unwrap_or_default();
        for painter in &document.texture_sets {
            let Some(mut set) = PreparedSet::new(painter, &cache, mesh.as_ref()) else {
                continue;
            };
            set.plan = ProceduralPlan::all(Choice::Replace);
            set.load_baked(&baked, &records);
            let stack = set.import(&table, &images);
            let uses = material_layers(&stack.layers, names);
            if uses.is_empty() {
                continue;
            }
            let loaded: Vec<_> = COMPARED
                .iter()
                .map(|(c, _)| *c)
                .filter(|c| set.load(*c, &cache))
                .collect();
            if loaded.is_empty() {
                continue;
            }
            let mut shell = TextureSet::new("look", painter.name.clone(), set.resolution);
            shell.channels = set.channels.specs().to_vec();
            shell.baked = set.baked.clone();
            let grid = PreviewGrid::new(&shell, GRID);
            let failed: Vec<&str> = stack.verdict.mask_failures.iter().map(|f| f.id.as_str()).collect();
            for u in uses {
                let Some((_, tally)) = acc.iter_mut().find(|(n, _)| *n == u.name) else {
                    continue;
                };
                let (weights, open) = weights(&grid, &u.chain, &failed);
                tally.layers += 1;
                tally.open_masks += usize::from(open);
                sample(&set, &grid, &weights, &loaded, tally);
            }
        }
        let looks = acc
            .into_iter()
            .filter(|(_, t)| !t.builder.is_empty())
            .map(|(n, t)| {
                let coverage = if t.texels > 0.0 {
                    (t.covered / t.texels) as f32
                } else {
                    0.0
                };
                (
                    n,
                    PainterLook {
                        look: t.builder.build(),
                        layers: t.layers,
                        open_masks: t.open_masks,
                        coverage,
                    },
                )
            })
            .collect();
        PainterLooks { looks }
    }
}

#[derive(Default)]
struct Tally {
    builder: LookBuilder,
    layers: usize,
    open_masks: usize,
    covered: f64,
    texels: f64,
}

/// A visible layer filling with a material, with its folders (outermost first, the layer last).
struct MaterialLayer<'a> {
    name: String,
    chain: Vec<&'a Layer>,
}

fn material_layers<'a>(layers: &'a [Layer], names: &[String]) -> Vec<MaterialLayer<'a>> {
    let mut out = Vec::new();
    walk(layers, &mut Vec::new(), names, &mut out);
    out
}

fn walk<'a>(layers: &'a [Layer], chain: &mut Vec<&'a Layer>, names: &[String], out: &mut Vec<MaterialLayer<'a>>) {
    for l in layers.iter().filter(|l| l.visible) {
        chain.push(l);
        match &l.kind {
            LayerKind::Group(children) => walk(children, chain, names, out),
            _ => {
                for e in l.effects.iter().filter(|e| e.enabled) {
                    if let EffectKind::Generator(p) = &e.kind
                        && let Some(name) = p.id.strip_prefix(SUBSTANCE_PREFIX)
                        && names.iter().any(|n| n == name)
                    {
                        out.push(MaterialLayer {
                            name: name.to_string(),
                            chain: chain.clone(),
                        });
                    }
                }
            }
        }
        chain.pop();
    }
}

/// The product of the switched-on masks along `chain` on the grid; true if one of them could not be computed
/// (it counts as open).
fn weights(grid: &PreviewGrid, chain: &[&Layer], failed: &[&str]) -> (Vec<f32>, bool) {
    let (w, h) = grid.size();
    let mut weights = vec![1.0f32; w as usize * h as usize];
    let mut open = false;
    for l in chain {
        if !l.mask.as_ref().is_some_and(|m| m.enabled) {
            continue;
        }
        if failed.contains(&l.id.as_str()) {
            open = true;
            continue;
        }
        let reduced = grid.reduce(l);
        let Some(tex) = grid.mask(&reduced, generators::builtin(), &NO_FILL_CONTEXT) else {
            continue;
        };
        for (wt, px) in weights.iter_mut().zip(&tex.px) {
            *wt *= px[0].clamp(0.0, 1.0);
        }
    }
    (weights, open)
}

/// Adds Painter's cached values under `weights` (on the mesh's texels) to the tally.
fn sample(
    set: &PreparedSet,
    grid: &PreviewGrid,
    weights: &[f32],
    channels: &[materialcraft::paint::Channel],
    tally: &mut Tally,
) {
    let (gw, gh) = grid.size();
    let (w, h) = set.resolution;
    let islands = set.islands();
    for gy in 0..gh {
        let y = ((u64::from(gy) * 2 + 1) * u64::from(h) / (u64::from(gh) * 2)) as u32;
        for gx in 0..gw {
            let x = ((u64::from(gx) * 2 + 1) * u64::from(w) / (u64::from(gw) * 2)) as u32;
            if islands.is_some_and(|i| !i.contains(x as usize, y as usize)) {
                continue;
            }
            tally.texels += 1.0;
            let weight = weights.get((gy * gw + gx) as usize).copied().unwrap_or(0.0);
            if weight < MIN_WEIGHT {
                continue;
            }
            tally.covered += f64::from(weight);
            for c in channels {
                let Some(image) = set.cache(*c) else { continue };
                let v = image.get(x, y);
                tally.builder.add(*c, v, weight * v[3].clamp(0.0, 1.0));
            }
        }
    }
}

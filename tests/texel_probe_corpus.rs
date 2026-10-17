//! The eyedropper's stored-value pick on real documents: every pick through the view projector and
//! [`TexelProbe`] must equal the full composite at that texel (the tile the compositor builds the channel from).
//!
//! Needs converted documents, so it is ignored by default:
//! `MC_PROBE_DOCS=a.mcpaint:b.mcpaint cargo test --release --test texel_probe_corpus -- --ignored --nocapture`

use materialcraft::camera::{Camera, Lens};
use materialcraft::material_fill::{FolderResolver, MaterialCache, MaterialFills};
use materialcraft::paint::probe::TexelProbe;
use materialcraft::paint::{Compositor, Document, FillContext, Rect, TILE_SIZE, format};
use materialcraft::projection::{ViewProjector, ViewScene};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// The scene as the viewport builds it: each part picks in its texture set's UV set.
fn scene(doc: &Document) -> ViewScene {
    let mut scene = ViewScene::new(&doc.mesh, doc.mesh.fit());
    for (i, part) in doc.mesh.parts.iter().enumerate() {
        if let Some(set) = doc.texture_sets.iter().find(|s| s.material_slot == part.material) {
            scene.set_uv_set(i, set.uv_set);
        }
    }
    scene
}

/// Picks on a grid from a few orbit positions; returns (picks compared, distinct texels).
fn check(path: &Path) -> (usize, usize) {
    let doc = format::load_path(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let materials = MaterialFills::new(Arc::new(FolderResolver::new(path.parent().map(Path::to_path_buf))), Arc::new(MaterialCache::new(256 << 20)));
    let fills = FillContext::with_sources(&materials);
    let probe = TexelProbe::new(&doc).with_fills(&fills);
    let scene = scene(&doc);
    let lens = Lens::Perspective { fov_y: 40f32.to_radians(), near: 0.03, far: 60.0 };
    let mut tiles: HashMap<(usize, String, u32, u32), (Rect, Vec<[f32; 4]>)> = HashMap::new();
    let (mut picks, mut texels) = (0, std::collections::BTreeSet::new());
    for (yaw, pitch) in [(0.0f32, 0.1f32), (1.9, 0.3), (3.6, -0.4), (-1.2, 0.9)] {
        let eye = [2.8 * yaw.sin() * pitch.cos(), 2.8 * pitch.sin(), 2.8 * yaw.cos() * pitch.cos()];
        let camera = Camera::looking_at(eye, [0.0; 3], [0.0, 1.0, 0.0], &lens, [96.0, 72.0]);
        let view = scene.projector(&doc.mesh, camera).expect("scene matches the mesh");
        for y in (0..72).step_by(6) {
            for x in (0..96).step_by(6) {
                let Some(hit) = view.pick([x as f32 + 0.5, y as f32 + 0.5]) else { continue };
                let Some(at) = probe.locate(&hit) else { continue };
                let set = &doc.texture_sets[at.set];
                for spec in &set.channels {
                    let got = probe.stored(at, spec.channel).expect("declared channel");
                    let (tx, ty) = (at.x / TILE_SIZE, at.y / TILE_SIZE);
                    let (r, px) = tiles.entry((at.set, spec.channel.id(), tx, ty)).or_insert_with(|| {
                        let r = Rect {
                            x: tx * TILE_SIZE,
                            y: ty * TILE_SIZE,
                            w: (set.resolution.0 - tx * TILE_SIZE).min(TILE_SIZE),
                            h: (set.resolution.1 - ty * TILE_SIZE).min(TILE_SIZE),
                        };
                        let mut px = vec![[0.0f32; 4]; r.area()];
                        Compositor::new(set).with_fills(&fills).composite_rect(spec.channel, r, &mut px);
                        (r, px)
                    });
                    let want = px[((at.y - r.y) * r.w + (at.x - r.x)) as usize];
                    assert_eq!(got.value, want, "{}: set {} texel {},{} channel {}", path.display(), set.id, at.x, at.y, spec.channel.id());
                }
                picks += 1;
                texels.insert((at.set, at.x, at.y));
            }
        }
    }
    (picks, texels.len())
}

#[test]
#[ignore = "needs converted corpus documents in MC_PROBE_DOCS"]
fn stored_picks_equal_the_composite_on_corpus_documents() {
    let list = std::env::var("MC_PROBE_DOCS").expect("MC_PROBE_DOCS: documents separated by ':'");
    for path in list.split(':').filter(|p| !p.is_empty()) {
        let (picks, texels) = check(Path::new(path));
        eprintln!("{path}: {picks} picks over {texels} texels equal the composite");
        assert!(picks > 50, "{path}: only {picks} picks hit the model");
    }
}

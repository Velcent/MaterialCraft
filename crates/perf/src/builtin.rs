//! The shipped scenario table. Its sources are the user's corpus (never part of the repository): the folder is
//! `$MC_PERF_CORPUS`, else [`CORPUS`]. Scenarios whose files are missing are skipped with a note.

use crate::scenario::{Action, EditKind, LayerChoice, PathShape, Scenario, Source, StrokeSpec, StrokeTarget, StrokeTool, Symmetry};
use std::path::PathBuf;

/// The corpus's default place.
pub const CORPUS: &str = "../Corpus";
pub const CORPUS_ENV: &str = "MC_PERF_CORPUS";

/// The new-project mesh (Capricon's low poly).
pub const MESH: &str = "Projekte/3D/Capricon/LP.obj";
/// A big imported stack (a converted and replayed project).
pub const BIG_DOC: &str = "Converted/AS-replaced-replayed.mcpaint";
/// A small Painter project, imported on open.
pub const SMALL_SPP: &str = "Projekte/3D/Auridion/AMULET/AMULET 3/AMULET 3.spp";

/// The step-51 stroke benchmark's id.
pub const STROKE: &str = "stroke-2k";
/// The step-51 edit benchmark's id.
pub const EDITS: &str = "edits-big";

/// `relative` in the corpus folder.
pub fn corpus(relative: &str) -> PathBuf {
    let root = std::env::var_os(CORPUS_ENV).map_or_else(|| PathBuf::from(CORPUS), PathBuf::from);
    root.join(relative)
}

fn new_project(resolution: u32) -> Source {
    Source::NewProject { mesh: corpus(MESH), resolution }
}

fn stroke(id: &str, about: &str, source: Source, spec: StrokeSpec) -> Scenario {
    Scenario { id: id.into(), about: about.into(), source, action: Action::Stroke(spec), repeat: 1 }
}

/// The step-51 edit list: opacity, visibility, blend mode, a mask edit, an effect parameter, then an undo.
pub fn step51_edits() -> Vec<EditKind> {
    use EditKind::*;
    vec![Opacity, Hide, Show, BlendMode, Mask, EffectParam, Undo]
}

/// Every built-in scenario, in table order.
pub fn scenarios() -> Vec<Scenario> {
    let paint = StrokeSpec::default();
    vec![
        stroke(STROKE, "Paint stroke on a new 2K project: 200 samples, 2 per frame, wave (step 51)", new_project(2048), paint.clone()),
        stroke(
            "erase-2k",
            "Eraser over a painted stroke on a new 2K project",
            new_project(2048),
            StrokeSpec { tool: StrokeTool::Erase, primed: true, ..paint.clone() },
        ),
        stroke(
            "mask-paint-2k",
            "Paint into the paint layer's mask (a white mask is added first), 2K",
            new_project(2048),
            StrokeSpec { target: StrokeTarget::Mask, ..paint.clone() },
        ),
        stroke(
            "symmetry-2k",
            "Paint stroke with X mirror symmetry, 2K",
            new_project(2048),
            StrokeSpec { symmetry: Symmetry::Mirror { axis: 0 }, ..paint.clone() },
        ),
        stroke(
            "large-brush-2k",
            "Paint stroke with a brush five times the default size, 2K",
            new_project(2048),
            StrokeSpec { size: Some(0.4), ..paint.clone() },
        ),
        stroke("stroke-4k", "Paint stroke on a new 4K project", new_project(4096), paint.clone()),
        stroke(
            "smudge-2k",
            "Smudge over a painted stroke, 100 samples, 2K",
            new_project(2048),
            StrokeSpec { tool: StrokeTool::Smudge, primed: true, samples: 100, ..paint.clone() },
        ),
        stroke(
            "spiral-undo-2k",
            "Spiral stroke, then undo and redo it, 2K",
            new_project(2048),
            StrokeSpec { path: PathShape::Spiral, undo_redo: true, ..paint.clone() },
        ),
        stroke(
            "stroke-big",
            "Paint stroke on a new layer on top of a big imported stack (the set under the stroke)",
            Source::Document(corpus(BIG_DOC)),
            StrokeSpec { layer: LayerChoice::New, ..paint },
        ),
        Scenario {
            id: EDITS.into(),
            about: "Layer edits on a big imported stack (step 51)".into(),
            source: Source::Document(corpus(BIG_DOC)),
            action: Action::Edits(step51_edits()),
            repeat: 1,
        },
        Scenario {
            id: "edits-small".into(),
            about: "Layer edits on a small Painter project, imported on open".into(),
            source: Source::Document(corpus(SMALL_SPP)),
            action: Action::Edits({
                use EditKind::*;
                vec![Opacity, Hide, Show, BlendMode, Mask, MaskToggle, EffectParam, EffectToggle, Undo, Redo]
            }),
            repeat: 3,
        },
        Scenario {
            id: "open-small".into(),
            about: "Import a small Painter project and composite every set, then twice more".into(),
            source: Source::Document(corpus(SMALL_SPP)),
            action: Action::Open,
            repeat: 2,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_round_trip() {
        let all = scenarios();
        let mut ids: Vec<&str> = all.iter().map(|s| s.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), all.len());
        for s in &all {
            assert_eq!(&Scenario::from_json(&s.to_json()).unwrap(), s);
        }
    }
}

//! (a) A paint stroke on a new project: a project made from a mesh at 2K (File > New project), the paint tool on its
//! paint layer, a stroke of 200 pointer samples across the view, two per frame. Every frame runs what the screen
//! runs: the brush tools' frame (stamping), the recomposite it asks for, the scene's poll (handing maps to the GPU
//! queue). Each recomposite is waited for, so its latency is measured on its own.

use super::{Bench, corpus_file, mean_max, ms};
use crate::gui_paint::input::{PointerSample, StrokeInput};
use crate::gui_paint::shell::tools::Tool;
use crate::gui_paint::tools::{PaintFrame, Painting};
use crate::gui_paint::view_pick::ViewPicking;
use crate::gui_paint::view_state::ViewState;
use materialcraft::paint::Channel;
use materialcraft::paint::edit::LayerRef;
use materialcraft::paint::new_project::{self, NewProject, ProjectTemplate};
use materialcraft_brush::{PaintTarget, TargetPart};
use std::time::{Duration, Instant};

const LP: &str = "../Corpus/Projekte/3D/Capricon/LP.obj";
const SAMPLES: usize = 200;
const PER_FRAME: usize = 2;
const VIEWPORT: [f32; 2] = [1600.0, 900.0];

#[test]
#[ignore]
fn perf_stroke_on_new_project() {
    let Some(mesh) = corpus_file("MC_PERF_MESH", LP) else { return };
    let mut settings = NewProject::new(&mesh, ProjectTemplate::builtins().remove(0));
    settings.resolution = 2048;
    let created = new_project::create(&settings, &mut |_, _| {}).expect("new project");
    let (mut b, opened) = Bench::open(created.document, mesh.clone());
    let set = 0;
    let s = &b.doc.doc().texture_sets[set];
    let layer = s.layers.last().expect("the new project's paint layer").id.clone();
    eprintln!("{}: {} parts, set {} at {:?}, opened in {:.0} ms", mesh.display(), b.doc.doc().mesh.parts.len(), s.name, s.resolution, ms(opened));
    let target = PaintTarget { layer: LayerRef::new(s.id.clone(), layer), part: TargetPart::Content };

    let mut picking = ViewPicking::default();
    let mut painting = Painting::default();
    painting.brush.value_mut(Channel::BaseColor).value = [0.85, 0.12, 0.08, 1.0];
    while !(picking.is_ready() && painting.surface_ready(&b.doc, set)) {
        picking.poll(Some(&b.doc));
        std::thread::sleep(Duration::from_millis(2));
    }
    let camera = {
        let state = ViewState::default();
        state.view.camera(&state.display.camera, VIEWPORT)
    };

    let sample = |i: usize| {
        let t = i as f32 / (SAMPLES - 1) as f32;
        PointerSample::mouse([VIEWPORT[0] * (0.3 + 0.4 * t), VIEWPORT[1] * (0.5 + 0.1 * (t * 9.0).sin())], f64::from(t) * 2.0)
    };
    let mut frames: Vec<Vec<StrokeInput>> = vec![vec![StrokeInput::Down(sample(0))]];
    for chunk in (1..SAMPLES).collect::<Vec<_>>().chunks(PER_FRAME) {
        frames.push(chunk.iter().map(|&i| StrokeInput::Move(sample(i))).collect());
    }
    frames.push(vec![StrokeInput::Up]);

    let before = b.scene.stats();
    let (mut stamp, mut poll, mut latency) = (Vec::new(), Vec::new(), Vec::new());
    let mut dabs = 0;
    let started = Instant::now();
    for input in frames {
        let t = Instant::now();
        let frame = PaintFrame { doc: &mut b.doc, picking: &picking, camera: Some(camera), tool: Tool::Paint, target: Some((target.clone(), set)), uv: None };
        let report = painting.frame(input, frame);
        stamp.push(t.elapsed());
        dabs = painting.stroke_dabs().unwrap_or(dabs);
        poll.push(b.poll());
        if let Some(s) = report.refresh {
            latency.push(b.refresh(vec![s]).0);
        }
    }
    let total = started.elapsed();
    let stats = super::delta(before, b.scene.stats());
    let refreshes = latency.len().max(1);
    let (stamp_mean, stamp_max) = mean_max(&stamp);
    let (poll_mean, poll_max) = mean_max(&poll);
    let (lat_mean, lat_max) = mean_max(&latency);
    eprintln!("stroke: {} frames, {dabs} dabs, {} recomposites, {:.0} ms in all", stamp.len(), latency.len(), ms(total));
    eprintln!(
        "  stamping (UI thread)     per frame {stamp_mean:.2} ms (max {stamp_max:.2}), per dab {:.3} ms",
        ms(stamp.iter().sum::<Duration>()) / dabs.max(1) as f64
    );
    eprintln!("  scene poll (UI thread)   per frame {poll_mean:.2} ms (max {poll_max:.2})");
    eprintln!("  recomposite latency      mean {lat_mean:.1} ms (max {lat_max:.1})");
    eprintln!(
        "  per recomposite: compositing {:.1} ms, preview maps {:.1} ms, GPU hand-off {:.2} ms, upload {:.1} MB",
        ms(stats.composite) / refreshes as f64,
        ms(stats.prepare) / refreshes as f64,
        ms(stats.handoff) / refreshes as f64,
        stats.upload_bytes as f64 / refreshes as f64 / 1e6
    );
    let history = b.doc.history().len();
    eprintln!("  history: {history} steps");
    assert!(dabs > 0, "the stroke painted nothing");
}

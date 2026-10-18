//! A small synthetic stack (no project files): a base fill, a fill masked by a mask generator, a paint layer
//! writing base colour, and (in the measured test) a painted layer over roughness whose bitmap mask covers the
//! left half. Painter's "cache" is made up from a known generator output.

use super::analysis::SetAnalysis;
use super::frames::FrameGrid;
use super::inverse::InversionSettings;
use super::measure::{Composer, Measurer};
use super::plan::{CaseStatus, plans};
use crate::model::*;
use crate::translate::channel_table::channel_spec;
use crate::translate::layers::images::LoadedImage;
use crate::translate::layers::{BlendTable, Computability, ImageSource, LayerImport, SetChannels, StackVerdict};
use crate::translate::replacements::ProceduralPlan;
use materialcraft::paint::{Channel, Compositor, PixelFormat, TextureSet, TiledImage};
use materialcraft::tex::Tex;

const BASE: u64 = 1;
const ROUGH: u64 = 128;
const SIZE: (u32, u32) = (4, 4);

fn header(channels: u64) -> EffectHeader {
    EffectHeader {
        uid: None,
        enabled: true,
        label: String::new(),
        channels: ChannelMask(channels),
        blending: Vec::new(),
    }
}

fn uniform(bit: u64, v: f32) -> PainterSource {
    PainterSource::Uniform(UniformSource {
        uid: None,
        channels: ChannelMask(bit),
        color: [v; 3],
        opacity: 1.0,
    })
}

fn fill(channels: u64, sources: Vec<PainterSource>) -> PainterEffect {
    PainterEffect::Fill(FillEffect {
        header: header(channels),
        sources,
        projection: 0,
        filtering: 2,
        uv_transform: None,
        placement: Default::default(),
    })
}

fn layer(name: &str, kind: LayerKind, content: Vec<PainterEffect>, mask: Option<PainterMask>) -> PainterLayer {
    PainterLayer {
        uid: None,
        name: name.into(),
        kind,
        visible: true,
        blending: Vec::new(),
        mask,
        content,
    }
}

fn mask(effects: Vec<PainterEffect>) -> Option<PainterMask> {
    Some(PainterMask {
        enabled: true,
        initial: MaskInitial::Black,
        effects,
    })
}

fn strokes(channels: u64) -> PainterEffect {
    let paint = PaintEffect {
        header: header(channels),
        tool: PaintTool::Brush,
        opacity: 1.0,
        strokes: Vec::new(),
        path_strokes: 1,
        brush_uid: None,
        colors: Vec::new(),
        alpha: Vec::new(),
        stencil: None,
    };
    PainterEffect::Group(EffectGroup {
        header: header(u64::MAX),
        effects: vec![PainterEffect::Paint(paint)],
    })
}

fn dirt() -> PainterEffect {
    let source = PainterSource::Procedural(ProceduralSource {
        uid: None,
        channels: ChannelMask(0),
        mode: 2,
        procedural: Some(Procedural {
            uid: None,
            url: "resource://shelf/mg_dirt?version=1.sbsar".into(),
            tweaks: Vec::new(),
            outputs: Vec::new(),
            inputs: Vec::new(),
        }),
    });
    fill(0, vec![source])
}

/// Base fill, a dirt-masked fill over both channels, a paint layer over base colour.
pub fn fixture() -> (PainterTextureSet, SetChannels) {
    let layers = vec![
        layer(
            "Base",
            LayerKind::Fill,
            vec![fill(u64::MAX, vec![uniform(BASE, 0.5), uniform(ROUGH, 0.2)])],
            None,
        ),
        layer(
            "Dirt",
            LayerKind::Fill,
            vec![fill(u64::MAX, vec![uniform(BASE, 0.1), uniform(ROUGH, 0.9)])],
            mask(vec![dirt()]),
        ),
        layer("Paint", LayerKind::Paint, vec![strokes(BASE)], None),
    ];
    let set = PainterTextureSet {
        uid: Some(1),
        material_uid: Some(2),
        name: "Set".into(),
        resolution_log2: Some([2, 2]),
        channels: Vec::new(),
        layers,
    };
    let channels = SetChannels::new(&[
        channel_spec(Channel::BaseColor, Some(0)),
        channel_spec(Channel::Roughness, Some(1)),
    ]);
    (set, channels)
}

/// The left half white: the scribble layer's bitmap mask.
struct HalfMask;

impl ImageSource for HalfMask {
    fn load(&self, _: &str) -> Result<LoadedImage, String> {
        let mut image = TiledImage::new(SIZE.0, SIZE.1, PixelFormat::from_id("graya8").ok_or("format")?)
            .map_err(|e| e.to_string())?;
        for y in 0..SIZE.1 {
            for x in 0..SIZE.0 {
                let v = if x < SIZE.0 / 2 { 1.0 } else { 0.0 };
                image.set(x, y, [v, v, v, 1.0]);
            }
        }
        Ok(LoadedImage { image, float: false })
    }
}

fn import(
    s: &PainterTextureSet,
    channels: &SetChannels,
    procedurals: &ProceduralPlan,
) -> crate::translate::layers::ImportedStack {
    let table = BlendTable::painter();
    LayerImport {
        table: &table,
        projections: &crate::translate::layers::ProjectionTable::painter(),
        images: &HalfMask,
        plan: procedurals,
    }
    .import(s, channels, SIZE)
}

pub fn import_verdict(s: &PainterTextureSet, channels: &SetChannels) -> StackVerdict {
    import(s, channels, &ProceduralPlan::default()).verdict
}

struct TestComposer<'a> {
    channels: &'a SetChannels,
}

impl Composer for TestComposer<'_> {
    fn composite(
        &self,
        painter: &PainterTextureSet,
        procedurals: &ProceduralPlan,
        channel: Channel,
    ) -> (Tex, Computability) {
        let stack = import(painter, self.channels, procedurals);
        let mut set = TextureSet::new("t", "T", SIZE);
        set.channels = self.channels.specs().to_vec();
        set.layers = stack.layers.clone();
        let tex = Compositor::with_effects(&set, materialcraft::generators::builtin())
            .composite_tex(channel)
            .0;
        (tex, stack.verdict.channel(channel))
    }
}

#[test]
fn a_generator_is_read_back_where_a_painted_layer_does_not_reach() {
    let (mut set, channels) = fixture();
    let scribble_mask = mask(vec![fill(
        0,
        vec![PainterSource::Bitmap(BitmapSource {
            uid: None,
            channels: ChannelMask(0),
            url: "/m?version=1".into(),
        })],
    )]);
    set.layers
        .push(layer("Scribble", LayerKind::Paint, vec![strokes(ROUGH)], scribble_mask));
    let analysis = SetAnalysis::new(&set, &channels, &|s| import_verdict(s, &channels));
    let plan = plans(&analysis)
        .into_iter()
        .find(|p| p.channel == Channel::Roughness)
        .expect("a roughness case");
    assert_eq!(plan.status, CaseStatus::WithNuisances);
    assert_eq!(
        (plan.candidate.ours, plan.nuisances.len(), plan.states()),
        (Some("dirt"), 1, 3)
    );
    // Painter's roughness: 0.2 under the dirt layer's 0.9, masked by a known generator output `t`; the scribble
    // wrote 0.55 on the left half.
    let truth = |i: usize| i as f32 / 15.0;
    let observed: Vec<[f32; 3]> = (0..16)
        .map(|i| match i % 4 < 2 {
            true => [0.55; 3],
            false => [0.2 + 0.7 * truth(i); 3],
        })
        .collect();
    let grid = FrameGrid::new((4, 4), 4, None);
    let composer = TestComposer { channels: &channels };
    let measurer = Measurer {
        composer: &composer,
        painter: &set,
        channels: &channels,
        grid: &grid,
        settings: InversionSettings::default(),
    };
    let m = measurer.measure(&plan, &observed, false);
    assert_eq!(
        (m.stats.island, m.stats.clean, m.stats.solved),
        (16, 8, 8),
        "{:?}",
        m.stats
    );
    assert_eq!(m.stats.composites, 5 + 2 * 2);
    for i in 0..16 {
        assert_eq!(m.mask[i], i % 4 >= 2, "texel {i}");
        if m.mask[i] {
            assert!(
                (m.theirs.px[i][0] - truth(i)).abs() < 1e-3,
                "texel {i}: {:?}",
                m.theirs.px[i]
            );
        }
    }
    assert!(m.ours.is_none() && m.components == 1);
}

#[test]
fn every_candidate_of_a_channel_gets_a_plan() {
    let (set, channels) = fixture();
    let analysis = SetAnalysis::new(&set, &channels, &|s| import_verdict(s, &channels));
    let all = plans(&analysis);
    let rough = all.iter().find(|p| p.channel == Channel::Roughness).unwrap();
    assert_eq!(rough.status, CaseStatus::Strict);
    let colour = all.iter().find(|p| p.channel == Channel::BaseColor).unwrap();
    assert_eq!(colour.status, CaseStatus::WithNuisances);
    assert_eq!(all.len(), 2);
}

impl super::joint_measure::MultiComposer for TestComposer<'_> {
    fn composite_channels(
        &self,
        painter: &PainterTextureSet,
        procedurals: &ProceduralPlan,
        channels: &[Channel],
    ) -> Vec<(Tex, Computability)> {
        channels
            .iter()
            .map(|c| self.composite(painter, procedurals, *c))
            .collect()
    }
}

#[test]
fn two_unknown_masks_are_solved_together_across_channels() {
    use super::joint_measure::{JointMeasurer, MultiComposer};
    use super::joint_plan::{JointStatus, joint_plans};
    use super::joint_solve::JointSettings;
    use super::slots::Nuisance;
    use super::substitute::{Swap, apply};
    let (mut set, channels) = fixture();
    // the paint layer becomes a fill over both channels whose mask is painted (unknown)
    set.layers[2] = layer(
        "Top",
        LayerKind::Fill,
        vec![fill(u64::MAX, vec![uniform(BASE, 0.8), uniform(ROUGH, 0.3)])],
        mask(vec![strokes(0)]),
    );
    let analysis = SetAnalysis::new(&set, &channels, &|s| import_verdict(s, &channels));
    let single = plans(&analysis);
    assert!(
        single.iter().all(|p| p.status == CaseStatus::WithNuisances),
        "{single:?}"
    );
    let joint = joint_plans(&single);
    assert_eq!(joint.len(), 1);
    let plan = &joint[0];
    assert_eq!(
        (plan.status, plan.unknowns(), plan.components()),
        (JointStatus::Feasible, 2, 4)
    );
    let candidate = plan.candidate.address.clone();
    let top = plan.masks[0].clone();
    assert!(matches!(top, Nuisance::MaskEffect(_)));
    // Painter's cache: per texel the dirt mask `a` and the painted mask `b`
    let truth = |i: usize| (i as f32 / 15.0, 0.4 * ((i * 7) % 16) as f32 / 15.0);
    let composer = TestComposer { channels: &channels };
    let mut observed = vec![vec![[0.0f32; 3]; 16]; plan.channels.len()];
    for i in 0..16 {
        let (a, b) = truth(i);
        let swaps = [Swap::Output(candidate.clone(), a), Swap::Grey(top.clone(), b)];
        let frames = composer.composite_channels(
            &apply(&set, &swaps, &channels),
            &ProceduralPlan::default(),
            &plan.channels,
        );
        for (frame, (tex, _)) in observed.iter_mut().zip(&frames) {
            let p = tex.px[i];
            frame[i] = [p[0], p[1], p[2]];
        }
    }
    let grid = FrameGrid::new((4, 4), 4, None);
    let measurer = JointMeasurer {
        composer: &composer,
        painter: &set,
        channels: &channels,
        grid: &grid,
        settings: JointSettings::default(),
    };
    let m = measurer.measure(plan, &observed, false);
    // none of the single-channel cases has a clean texel; jointly most texels are solved (the rest are where
    // the masks barely move the linear values)
    assert_eq!((m.stats.island, m.stats.clean), (16, 16));
    assert!(m.stats.solved >= 12, "{:?} {:?}", m.stats, m.notes);
    assert_eq!(m.stats.composites, 9 + 2);
    for i in (0..16).filter(|i| m.mask[*i]) {
        assert!(
            (m.theirs.px[i][0] - truth(i).0).abs() < 2e-3,
            "texel {i}: {:?}",
            m.theirs.px[i]
        );
    }
}

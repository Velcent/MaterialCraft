use super::*;
use crate::paint::effect::{ContentEffect, EffectKind, GreyBlend, MaskEffect};
use crate::paint::{Channel, Document, Fill, Layer, LayerKind, Mask, MaskPixels, PaintContent, PixelFormat, TextureSet, TiledImage};

fn record(samples: usize) -> StrokeRecord {
    let stroke = Stroke {
        tool: StrokeTool::Paint,
        preset: String::new(),
        view: ViewKind::Uv,
        camera: Camera::default(),
        seed: 1,
        symmetry: Symmetry::default(),
        samples: (0..samples).map(|i| StrokeSample::at([i as f32, 0.0], i as f32)).collect(),
    };
    StrokeRecord { strokes: vec![stroke.clone(), stroke] }
}

fn img() -> TiledImage {
    TiledImage::new(2, 2, PixelFormat::from_id("graya8").unwrap()).unwrap()
}

/// A paint layer, a fill layer with a paint effect and a masked layer, each with a record.
fn layers() -> Vec<Layer> {
    let mut paint = PaintContent::from_images(vec![(Channel::Roughness, img())]);
    paint.strokes = Some(record(3).into());
    let fx = PaintContent { strokes: Some(StrokeData::Unreadable(UnreadableStrokes { bytes: vec![1], reason: "newer".into() })), ..PaintContent::default() };
    let fill = Layer::new("f", "F", LayerKind::Fill(Fill::uniform(vec![]))).with_effect(ContentEffect::new(Vec::new(), EffectKind::Paint(fx)));
    let mut masked = Layer::new("m", "M", LayerKind::Paint(PaintContent::default()));
    let mut pixels = MaskPixels::new(img());
    pixels.strokes = Some(record(1).into());
    masked.mask = Some(
        Mask::new(0.0)
            .with_effect(MaskEffect::new(GreyBlend::default(), EffectKind::Fill(1.0)))
            .with_effect(MaskEffect::new(GreyBlend::default(), EffectKind::Paint(pixels))),
    );
    vec![Layer::new("p", "P", LayerKind::Paint(paint)), fill, Layer::new("g", "G", LayerKind::Group(vec![masked]))]
}

#[test]
fn records_are_found_per_owner() {
    let l = layers();
    let owners: Vec<_> = l.iter().map(|l| l.stroke_records().into_iter().map(|(o, _)| o).collect::<Vec<_>>()).collect();
    assert_eq!(owners, [vec![StrokeOwner::Paint], vec![StrokeOwner::Effect { effect: 0 }], vec![]]);
    let masked = &l[2].children()[0];
    assert_eq!(masked.stroke_records().into_iter().map(|(o, _)| o).collect::<Vec<_>>(), [StrokeOwner::Mask { effect: 1 }]);
    let mut masked = masked.clone();
    assert!(masked.strokes_mut(StrokeOwner::Mask { effect: 0 }).is_none(), "a fill has no record slot");
    assert!(masked.strokes_mut(StrokeOwner::Effect { effect: 0 }).is_none());
    assert!(masked.strokes_mut(StrokeOwner::Paint).is_some_and(|s| s.is_none()));
    *masked.strokes_mut(StrokeOwner::Mask { effect: 1 }).unwrap() = None;
    assert!(masked.stroke_records().is_empty());
}

#[test]
fn totals_and_problems_walk_the_document() {
    let mut doc = Document::default();
    let mut set = TextureSet::new("s", "S", (2, 2));
    set.layers = layers();
    doc.texture_sets.push(set);
    assert_eq!(totals(&doc), (4, 2 * 3 + 2));
    let problems = problems(&doc);
    assert_eq!(problems.len(), 1);
    assert_eq!((problems[0].layer.as_str(), problems[0].owner, problems[0].reason.as_str()), ("f", StrokeOwner::Effect { effect: 0 }, "newer"));
    assert_eq!(problems[0].to_string(), "texture set \"s\" / layer \"f\" effect 0: stroke record not readable (newer)");
    assert_eq!(StrokeData::from(record(0)).record().map(|r| r.sample_count()), Some(0));
}

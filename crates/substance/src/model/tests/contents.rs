//! Layer contents and masks: uniform values, bitmaps, procedurals with tweaks, strokes, filters.

use super::doc::*;
use crate::model::*;

fn only_layer(content: Vec<V>, mask: Option<Vec<V>>) -> (PainterLayer, ModelReport) {
    let bytes = single_set(vec![layer(10, "L", content, vec![], mask)]);
    let (mut doc, report) = PainterDocument::from_bytes(&bytes).expect("model");
    (doc.texture_sets.remove(0).layers.remove(0), report)
}

fn procedural_source(channels: u64, url: &'static str, tweaks: Vec<V>) -> V {
    let procedural = obj(
        "DataProcedural",
        vec![
            (
                "outputs",
                V::List(vec![obj(
                    "DataProceduralOutput",
                    vec![("channelTypes", V::Uid(1)), ("identifier", V::Str("basecolor"))],
                )]),
            ),
            ("tweaks", V::List(tweaks)),
            ("urlToSbsRes", V::Str(url)),
        ],
    );
    obj(
        "DataSourceProcedural",
        vec![
            ("channelTypes", V::UInt(channels as u32)),
            ("mode", V::Enum(1)),
            ("procedural", procedural),
        ],
    )
}

#[test]
fn uniform_values_are_kept_per_channel_as_stored() {
    let sources = vec![
        uniform(1, [0.27583286, 0.288816, 0.30663478]),
        uniform(128, [0.3; 3]),
        uniform(4194304, [0.5, 0.5, 1.0]),
    ];
    let (layer, _) = only_layer(vec![fill(20, sources)], None);
    assert_eq!(layer.kind, LayerKind::Fill);
    let PainterEffect::Fill(f) = &layer.content[0] else {
        panic!("fill")
    };
    assert!(f.is_uniform());
    let values: Vec<_> = f.uniform_values().collect();
    assert_eq!(
        values[0],
        (PainterChannel::BaseColor, [0.27583286, 0.288816, 0.30663478])
    );
    assert_eq!(values[1], (PainterChannel::Roughness, [0.3; 3]));
    assert_eq!(values[2].0, PainterChannel::Normal);
    assert_eq!(ContentKind::of(&layer.content), ContentKind::Uniform);
}

#[test]
fn bitmap_and_procedural_sources_keep_resources_and_tweaks() {
    let bitmap = obj(
        "DataSourceBitmap",
        vec![
            (
                "bitmap",
                obj("DataBitmap", vec![("urlToBitmapRes", V::Str("/Moth?version=abc"))]),
            ),
            ("channelTypes", V::Uid(2)),
        ],
    );
    let tweaks = vec![
        obj(
            "DataTweakFloat3",
            vec![("identifier", V::Str("color")), ("value", V::Vec3([0.5, 0.25, 0.0]))],
        ),
        obj(
            "DataTweakInt",
            vec![("identifier", V::Str("$randomseed")), ("value", V::Int(-3))],
        ),
        obj("DataTweakMystery", vec![("identifier", V::Str("x"))]),
    ];
    let procedural = procedural_source(
        1 | 128,
        "resource://starter_assets/concrete/concrete?version=1.sbsar",
        tweaks,
    );
    let (layer, report) = only_layer(vec![fill(20, vec![bitmap]), fill(30, vec![procedural])], None);
    let PainterEffect::Fill(f) = &layer.content[0] else {
        panic!("fill")
    };
    let PainterSource::Bitmap(b) = &f.sources[0] else {
        panic!("bitmap")
    };
    assert_eq!((b.url.as_str(), b.channels), ("/Moth?version=abc", ChannelMask(2)));
    let PainterEffect::Fill(f) = &layer.content[1] else {
        panic!("fill")
    };
    let PainterSource::Procedural(p) = &f.sources[0] else {
        panic!("procedural")
    };
    let proc = p.procedural.as_ref().expect("procedural");
    assert_eq!(proc.name(), "concrete");
    assert_eq!(proc.outputs[0].channels, ChannelMask(1));
    assert_eq!(proc.tweaks[0].value, TweakValue::Floats(vec![0.5, 0.25, 0.0]));
    assert_eq!(proc.tweaks[1].value, TweakValue::Int(-3));
    assert_eq!(proc.tweaks[2].value, TweakValue::Other("DataTweakMystery".into()));
    assert_eq!(report.raw(), 1);
    assert_eq!(
        ContentKind::of(&layer.content),
        ContentKind::Mixed,
        "bitmap and procedural"
    );
}

#[test]
fn references_point_at_anchors() {
    let anchor = obj(
        "DataActionAnchor",
        vec![
            ("enabled", V::Bool(true)),
            ("label", V::Str("Edges")),
            ("uid", V::Uid(90)),
        ],
    );
    let reference = obj(
        "DataSourceReference",
        vec![
            ("channelTypes", V::Uid(0)),
            ("levels", obj("DataLevels", vec![("gamma", V::Vec3([2.0; 3]))])),
            (
                "reference",
                obj(
                    "DataReference",
                    vec![("source", V::Ref(90)), ("sourceLabel", V::Str("Edges"))],
                ),
            ),
        ],
    );
    let (layer, report) = only_layer(vec![anchor], Some(vec![fill(20, vec![reference])]));
    let PainterEffect::Anchor(a) = &layer.content[0] else {
        panic!("anchor")
    };
    assert_eq!((a.label.as_str(), a.uid), ("Edges", Some(90)));
    let PainterEffect::Fill(f) = &layer.mask.as_ref().expect("mask").effects[0] else {
        panic!("fill")
    };
    let PainterSource::Reference(r) = &f.sources[0] else {
        panic!("reference")
    };
    assert_eq!((r.anchor_uid, r.anchor_label.as_str()), (Some(90), "Edges"));
    assert_eq!(report.not_modelled(), 0);
}

#[test]
fn masks_hold_generators_strokes_and_filters() {
    let stroke = obj("DataStroke", vec![("points", V::List(vec![]))]);
    let paint = obj(
        "DataActionPaint",
        vec![
            ("enabled", V::Bool(true)),
            ("opacity", V::Float(0.5)),
            ("sourceColor", V::List(vec![uniform(0, [1.0; 3])])),
            ("strokes", V::List(vec![stroke])),
        ],
    );
    let group = obj(
        "DataActionGroup",
        vec![
            ("enabled", V::Bool(true)),
            ("subStack", stack("DataStackActions", 50, vec![paint])),
        ],
    );
    let generator = fill(
        60,
        vec![procedural_source(
            0,
            "resource://generators/mg_dirt?version=1.sbsar",
            vec![],
        )],
    );
    let filter = obj(
        "DataActionFilterProcedural",
        vec![("enabled", V::Bool(false)), ("procedural", V::Absent)],
    );
    let (layer, report) = only_layer(vec![], Some(vec![group, generator, filter]));
    let mask = layer.mask.as_ref().expect("mask");
    assert_eq!(mask.initial, MaskInitial::Black);
    assert!(mask.enabled);
    let PainterEffect::Group(g) = &mask.effects[0] else {
        panic!("group")
    };
    let PainterEffect::Paint(p) = &g.effects[0] else {
        panic!("paint")
    };
    assert_eq!((p.tool, p.strokes, p.opacity), (PaintTool::Brush, 1, 0.5));
    assert!(p.colors[0].channels().is_empty(), "mask channel");
    let PainterEffect::Filter(f) = &mask.effects[2] else {
        panic!("filter")
    };
    assert!(!f.header.enabled && f.procedural.is_none());
    assert_eq!(ContentKind::of(&mask.effects), ContentKind::Mixed);
    assert_eq!(report.classes["DataStroke"].recognised, 1);
    assert_eq!(report.raw(), 0);
}

#[test]
fn stroke_points_are_counted_but_not_mirrored() {
    let points: Vec<V> = (0..3)
        .map(|_| obj("DataStrokeKey", vec![("type", V::Enum(0))]))
        .collect();
    let stroke = obj("DataStroke", vec![("points", V::List(points))]);
    let paint = obj("DataActionPaint", vec![("strokes", V::List(vec![stroke]))]);
    let (layer, report) = only_layer(vec![paint], None);
    assert_eq!(layer.kind, LayerKind::Paint);
    assert_eq!(report.classes["DataStrokeKey"].total, 3);
    assert_eq!(report.classes["DataStrokeKey"].not_modelled(), 3);
    assert_eq!(ContentKind::of(&layer.content), ContentKind::Strokes);
}

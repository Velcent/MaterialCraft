use super::*;
use crate::mesh::{Mesh, MeshPart};
use crate::paint::{BlendMode, Fill, ForeignFile, Layer, LayerKind, PaintContent, PixelFormat, TiledImage};

fn img(w: u32, h: u32) -> TiledImage {
    TiledImage::new(w, h, PixelFormat::from_id("graya8").unwrap()).unwrap()
}

fn valid_doc() -> Document {
    let mesh = Mesh { materials: vec!["body".into()], parts: vec![MeshPart { uv_sets: vec![vec![]], ..MeshPart::default() }] };
    let mut doc = Document::new(mesh);
    let mut set = TextureSet::new("body", "Body", (64, 64)).with_channel(Channel::BaseColor).with_channel(Channel::Roughness);
    let mut paint = PaintContent::default();
    paint.set_image(Channel::BaseColor, img(64, 64));
    set.layers.push(Layer::new("fill", "Fill", LayerKind::Fill(Fill::uniform(vec![(Channel::Roughness, [0.3; 4])]))).with_blend(
        Channel::Roughness,
        BlendMode::Normal,
        1.0,
    ));
    set.layers.push(Layer::new(
        "g",
        "Group",
        LayerKind::Group(vec![
            Layer::new("p", "Paint", LayerKind::Paint(paint)).with_blend(Channel::BaseColor, BlendMode::Multiply, 0.5).with_mask(img(64, 64)),
        ]),
    ));
    doc.texture_sets.push(set);
    doc
}

fn first_problem(f: impl FnOnce(&mut Document)) -> DocError {
    let mut doc = valid_doc();
    f(&mut doc);
    doc.validate().expect_err("expected a validation error")
}

#[test]
fn valid_document_passes() {
    let doc = valid_doc();
    assert_eq!(doc.validate(), Ok(()));
    assert!(doc.texture_set("body").is_some());
    assert!(doc.texture_set_for_slot(0).is_some());
}

#[test]
fn reports_each_broken_invariant() {
    let e = first_problem(|d| d.texture_sets.push(d.texture_sets[0].clone()));
    assert!(matches!(e, DocError::DuplicateTextureSet { .. }));
    let e = first_problem(|d| d.texture_sets[0].resolution = (0, 64));
    assert!(matches!(e, DocError::BadResolution { .. }));
    let e = first_problem(|d| d.texture_sets[0].material_slot = 3);
    assert!(matches!(e, DocError::MaterialSlotOutOfRange { slot: 3, .. }));
    let e = first_problem(|d| d.texture_sets[0].uv_set = 1);
    assert!(matches!(e, DocError::UvSetOutOfRange { uv_set: 1, .. }));
    let e = first_problem(|d| d.texture_sets[0].channels.push(crate::paint::ChannelSpec::new(Channel::BaseColor)));
    assert!(matches!(e, DocError::DuplicateChannel { channel: Channel::BaseColor, .. }));
    let e = first_problem(|d| d.texture_sets[0].layers[0].id = "p".into());
    assert_eq!(e, DocError::DuplicateLayer { set: "body".into(), layer: "p".into() });
    let e = first_problem(|d| d.texture_sets[0].layers[0].set_blend(Channel::Metallic, BlendMode::Normal, 1.0));
    assert!(matches!(e, DocError::UndeclaredChannel { channel: Channel::Metallic, .. }));
    let e = first_problem(|d| d.texture_sets[0].layers[0].opacity = 1.5);
    assert!(matches!(e, DocError::BadOpacity { .. }));
    let e = first_problem(|d| d.texture_sets[0].layers[0].opacity = f32::NAN);
    assert!(matches!(e, DocError::BadOpacity { .. }));
    let e = first_problem(|d| {
        if let Some(l) = d.texture_sets[0].layer_mut("p") {
            l.mask = Some(crate::paint::Mask::Image(img(32, 64)));
        }
    });
    assert_eq!(e, DocError::ImageSize { set: "body".into(), layer: "p".into(), channel: None, size: (32, 64), expected: (64, 64) });
    let e = first_problem(|d| {
        if let Some(LayerKind::Paint(p)) = d.texture_sets[0].layer_mut("p").map(|l| &mut l.kind) {
            p.images.push((Channel::BaseColor, img(64, 64)));
        }
    });
    assert!(matches!(e, DocError::DuplicateLayerChannel { channel: Channel::BaseColor, .. }));
    let e = first_problem(|d| d.foreign = vec![ForeignFile { path: "a".into(), bytes: vec![] }; 2]);
    assert!(matches!(e, DocError::DuplicateForeignFile { .. }));
    assert!(!e.to_string().is_empty());
}

#[test]
fn deep_groups_are_rejected() {
    let mut layer = Layer::new("leaf", "Leaf", LayerKind::Group(vec![]));
    for i in 0..(MAX_GROUP_DEPTH + 2) {
        layer = Layer::new(format!("g{i}"), "G", LayerKind::Group(vec![layer]));
    }
    let e = first_problem(|d| d.texture_sets[0].layers.push(layer));
    assert!(matches!(e, DocError::GroupTooDeep { .. }));
}

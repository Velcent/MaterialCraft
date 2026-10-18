//! Candidates and their export on synthetic projects (no corpus data).

use super::*;
use crate::model::*;
use crate::translate::layers::images::{ImageSource, LoadedImage};
use materialcraft::assets::{Library, LibraryRoot, RootId, RootKind, Sidecar, Usage};
use materialcraft::paint::brush::{BrushPreset, Tip};
use materialcraft::paint::{PixelFormat, PixelLayout, SampleType, TiledImage};
use std::path::PathBuf;

const SHAPE: &str = "resource://allegorithmic.1c86/shape/shape?version=f2.sbsar";

fn header() -> EffectHeader {
    EffectHeader {
        uid: None,
        enabled: true,
        label: String::new(),
        channels: ChannelMask(u64::MAX),
        blending: vec![],
    }
}

fn bitmap(url: &str) -> PainterSource {
    PainterSource::Bitmap(BitmapSource {
        uid: None,
        channels: ChannelMask(1),
        url: url.into(),
    })
}

fn shape(hardness: f32) -> PainterSource {
    PainterSource::Procedural(ProceduralSource {
        uid: None,
        channels: ChannelMask(0),
        mode: 0,
        procedural: Some(Procedural {
            uid: None,
            url: SHAPE.into(),
            tweaks: vec![Tweak {
                identifier: "hardness".into(),
                value: TweakValue::Floats(vec![hardness]),
            }],
            outputs: vec![],
            inputs: vec![],
        }),
    })
}

fn paint(size: f32, alpha: PainterSource, stencil: Option<PainterSource>, strokes: usize) -> PainterEffect {
    PainterEffect::Paint(PaintEffect {
        header: header(),
        tool: PaintTool::Brush,
        opacity: 1.0,
        strokes: vec![],
        path_strokes: strokes,
        brush_uid: None,
        brush: Some(PainterBrush {
            size,
            ..PainterBrush::default()
        }),
        stamp_blending: 0,
        colors: vec![],
        alpha: vec![alpha],
        stencil,
    })
}

fn fill(sources: Vec<PainterSource>) -> PainterEffect {
    PainterEffect::Fill(FillEffect {
        header: header(),
        sources,
        projection: 0,
        filtering: 2,
        placement: Default::default(),
        uv_transform: None,
    })
}

fn layer(content: Vec<PainterEffect>, mask: Vec<PainterEffect>) -> PainterLayer {
    PainterLayer {
        uid: None,
        name: "L".into(),
        kind: LayerKind::Paint,
        visible: true,
        blending: vec![],
        mask: (!mask.is_empty()).then_some(PainterMask {
            enabled: true,
            initial: MaskInitial::Black,
            effects: mask,
        }),
        content,
    }
}

fn document(layers: Vec<PainterLayer>) -> PainterDocument {
    let set = PainterTextureSet {
        uid: None,
        material_uid: None,
        name: "S".into(),
        resolution_log2: None,
        channels: vec![],
        layers,
    };
    PainterDocument {
        format_version: 78,
        texture_sets: vec![set],
    }
}

/// The synthetic project: two brushes (one used twice), a stored bitmap alpha and stencil from the user, a vendor
/// texture fill, a baked map, a bitmap that is not stored, and the vendor's environment.
fn project() -> PainterDocument {
    let stroke = paint(0.1, shape(0.9), None, 3);
    let content = vec![
        PainterEffect::Group(EffectGroup {
            header: header(),
            effects: vec![
                stroke.clone(),
                stroke,
                paint(0.3, bitmap("/Scratch?version=a1"), None, 1),
            ],
        }),
        paint(
            0.2,
            shape(0.0),
            Some(bitmap("resource://shelf/Logo?version=b2.image")),
            0,
        ),
        fill(vec![
            bitmap("resource://starter_assets/Rust/Rust_d?version=c3.image"),
            bitmap("/Ambient Occlusion Map from Mesh S?version=d4"),
        ]),
    ];
    let mask = vec![fill(vec![
        bitmap("/Missing?version=e5"),
        bitmap("/Baked elsewhere?version=f6"),
    ])];
    document(vec![layer(content, mask)])
}

fn candidates() -> LibraryCandidates {
    let stored = |url: &str| !url.contains("Missing");
    let facts = ProjectFacts {
        project: "/home/u/Cup.spp".into(),
        baked: ["/Baked elsewhere?version=f6".to_string()].into_iter().collect(),
        environment: Some("resource://viewer/panorama?version=g7.image".into()),
        stored: &stored,
        baked_name: &crate::translate::baked_maps::is_baked_name,
    };
    Collector::new(&facts).collect(&project())
}

#[test]
fn a_project_offers_its_brushes_bitmaps_and_environment_with_their_origin() {
    let c = candidates();
    let summary: Vec<(CandidateKind, &str, bool, usize)> = c
        .items
        .iter()
        .map(|i| (i.kind, i.name.as_str(), i.shareable(), i.uses))
        .collect();
    assert_eq!(
        summary,
        [
            (CandidateKind::Brush, "Cup brush 1 - hard round tip", true, 2),
            (CandidateKind::Brush, "Cup brush 2 - alpha tip", true, 1),
            (CandidateKind::Alpha, "Scratch", true, 1),
            (CandidateKind::Stencil, "Logo", true, 1),
            (CandidateKind::Texture, "Rust_d", false, 1),
            (CandidateKind::Environment, "panorama", false, 1),
        ]
    );
    // the effect without strokes offers no brush; baked maps are never offered; what is not stored is counted
    assert_eq!(c.left_out, ["1 × texture not stored in the project"]);
    assert_eq!(
        c.get("brush:2").and_then(|b| b.brings()),
        Some("alpha:/Scratch?version=a1")
    );
    let CandidateSource::Brush { preset, .. } = &c.items[0].source else {
        panic!("a brush")
    };
    assert_eq!((preset.tip.clone(), preset.hardness), (Tip::Round, 0.9));
    assert_eq!(
        c.get("texture:resource://starter_assets/Rust/Rust_d?version=c3.image")
            .map(|t| t.origin.clone()),
        Some(Origin::VendorShelf("starter_assets".into()))
    );
}

#[test]
fn a_brush_brings_its_alpha_and_images_come_first() {
    let c = candidates();
    let keys: Vec<&str> = c
        .closure(&["brush:2".to_string()])
        .iter()
        .map(|i| i.key.as_str())
        .collect();
    assert_eq!(keys, ["alpha:/Scratch?version=a1", "brush:2"]);
    assert!(c.closure(&[]).is_empty());
}

/// Every stored image is a small grey gradient (8-bit) or, for the environment, a float image.
struct Images;

impl ImageSource for Images {
    fn load(&self, url: &str) -> Result<LoadedImage, String> {
        let float = url.contains("panorama");
        let sample = if float { SampleType::F32 } else { SampleType::U8 };
        let mut image =
            TiledImage::new(4, 2, PixelFormat::new(PixelLayout::GrayAlpha, sample)).map_err(|e| e.to_string())?;
        image.fill([0.5, 0.5, 0.5, 1.0]);
        Ok(LoadedImage { image, float })
    }
}

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mc-library-export-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn ticked_items_are_written_with_provenance_and_show_in_the_tray() {
    let c = candidates();
    let dir = scratch("all");
    let keys = vec![
        "brush:2".to_string(),
        "texture:resource://starter_assets/Rust/Rust_d?version=c3.image".to_string(),
        "environment:resource://viewer/panorama?version=g7.image".to_string(),
    ];
    let written = LibraryExport::new(&c, &Images).write(&keys, &dir);
    let refs: Vec<&str> = written
        .iter()
        .map(|w| w.result.as_ref().map(|s| s.reference.as_str()).unwrap_or("failed"))
        .collect();
    assert_eq!(
        refs,
        [
            "alphas/Scratch.png",
            "textures/Rust_d.png",
            "environments/panorama.exr",
            "brushes/Cup brush 2 - alpha tip.mcbrush"
        ]
    );

    let preset = BrushPreset::from_file_text(
        &std::fs::read_to_string(dir.join("brushes/Cup brush 2 - alpha tip.mcbrush")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        preset.tip,
        Tip::Alpha("alphas/Scratch.png".into()),
        "the brush's tip is the alpha written with it"
    );
    let sidecar = Sidecar::parse(&std::fs::read_to_string(dir.join("textures/Rust_d.png.mcmeta")).unwrap()).unwrap();
    assert!(sidecar.tags.contains(&NOT_SHAREABLE_TAG.to_string()) && sidecar.provenance.is_imported());
    assert!(sidecar.description.unwrap().contains("not shareable"));

    let lib = Library::with_roots(
        [LibraryRoot::folder("user", "My assets", RootKind::User, &dir)],
        &materialcraft::diag::NullSink,
    );
    let root = RootId::new("user");
    let brush = lib
        .catalogue()
        .get(&root, "brushes/Cup brush 2 - alpha tip.mcbrush")
        .unwrap();
    assert_eq!(brush.usage, Usage::Brush);
    assert!(dir.join("brushes/Cup brush 2 - alpha tip.mcbrush.thumb.png").is_file());
    assert_eq!(
        lib.catalogue().get(&root, "environments/panorama.exr").map(|a| a.usage),
        Some(Usage::Environment)
    );
    assert_eq!(
        lib.catalogue()
            .get(&root, "alphas/Scratch.png")
            .map(|a| a.provenance.is_imported()),
        Some(true)
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn nothing_ticked_writes_nothing() {
    let dir = scratch("none");
    assert!(LibraryExport::new(&candidates(), &Images).write(&[], &dir).is_empty());
    assert!(!dir.exists());
}

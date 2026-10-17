//! Documents used by the format tests.

use crate::mesh::{Mesh, MeshPart};
use crate::paint::effect::Procedural;
use crate::paint::height_normal::HeightFilter;
use crate::paint::{
    BakedKind, BakedMap, BlendMode, Channel, ChannelSpec, ColorSpace, Document, Fidelity, Fill, FillSource, ForeignFile, ImportReport, Layer, LayerImage,
    LayerKind, PaintContent, PixelFormat, PixelLayout, SampleType, TextureSet, TiledImage,
};

/// Every pixel format.
pub fn all_formats() -> Vec<PixelFormat> {
    PixelLayout::ALL.into_iter().flat_map(|l| SampleType::ALL.into_iter().map(move |s| PixelFormat::new(l, s))).collect()
}

/// A pseudo-random generator, deterministic per seed.
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Rng {
        Rng(seed.wrapping_mul(2_654_435_761).wrapping_add(1))
    }

    pub fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.0 >> 8
    }

    pub fn unit(&mut self) -> f32 {
        (self.next() % 10_000) as f32 / 9_999.0
    }
}

/// An image with noise in some tiles (others stay unallocated); floats include negatives and
/// values above 1.
pub fn noisy(w: u32, h: u32, format: PixelFormat, seed: u32) -> TiledImage {
    let mut img = TiledImage::new(w, h, format).unwrap();
    let mut rng = Rng::new(seed);
    let float = format.sample.is_float();
    for (i, (tx, ty)) in img.all_tiles().into_iter().enumerate() {
        if i % 3 == 1 {
            continue;
        }
        let r = img.tile_rect(tx, ty).unwrap();
        let px: Vec<[f32; 4]> = (0..r.area())
            .map(|_| {
                std::array::from_fn(|_| {
                    let v = rng.unit();
                    if float { v * 8.0 - 2.0 } else { v }
                })
            })
            .collect();
        img.write_tile(tx, ty, &px);
    }
    img
}

fn quad(name: &str, material: usize, uv_sets: usize, z: f32) -> MeshPart {
    let uv = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
    MeshPart {
        name: name.into(),
        material,
        positions: vec![[-1.0, -1.0, z], [1.0, -1.0, z], [1.0, 1.0, z], [-1.0, 1.0, z]],
        normals: vec![[0.0, 0.0, 1.0]; 4],
        tangents: vec![[1.0, 0.0, 0.0, 1.0]; 4],
        uv_sets: (0..uv_sets).map(|s| uv.iter().map(|p| [p[0] * 0.5 + s as f32 * 0.5, p[1]]).collect()).collect(),
        triangles: vec![[0, 1, 2], [0, 2, 3]],
        // one quad: the source polygons are kept (since 0.9)
        faces: crate::mesh::FaceGroups::from_polygon_sizes([4], 2),
    }
}

fn paint(id: &str, images: Vec<(Channel, TiledImage)>) -> Layer {
    let mut l = Layer::new(id, format!("Paint {id}"), LayerKind::Paint(PaintContent::from_images(images)));
    let channels: Vec<Channel> = l.images().into_iter().filter_map(|(c, _)| if let LayerImage::Paint(c) = c { Some(c) } else { None }).collect();
    for c in channels {
        l.set_blend(c, BlendMode::Normal, 1.0);
    }
    l
}

/// A texture set using every pixel format, every layer kind, every effect kind, masks, geometry masks, nested
/// groups and baked maps.
fn body_set() -> TextureSet {
    let (w, h) = (150, 130);
    let mut set = TextureSet::new("body", "Body / Main", (w, h));
    set.channels = Channel::STANDARD.into_iter().map(ChannelSpec::new).collect();
    set.channels.push(ChannelSpec { channel: Channel::User(3), format: PixelFormat::from_id("rgb32f").unwrap(), color_space: ColorSpace::Linear });
    for (i, f) in all_formats().into_iter().enumerate() {
        let c = [Channel::BaseColor, Channel::Height, Channel::User(3), Channel::Emissive][i % 4];
        let mut l = paint(&format!("p{i}"), vec![(c, noisy(w, h, f, i as u32))]);
        if i % 5 == 0 {
            let mask_format = all_formats()[(i * 7) % 16];
            l = l.with_mask(noisy(w, h, mask_format, 100 + i as u32));
        }
        set.layers.push(l);
    }
    let base = Fill::uniform(vec![
        (Channel::BaseColor, [0.62, 0.48, 0.33, 1.0]),
        (Channel::Roughness, [0.7, 0.7, 0.7, 1.0]),
        (Channel::Height, [-0.0, 1e-7, 3.5, 0.1]),
    ]);
    let mut fill = Layer::new("fill", "Base", LayerKind::Fill(base)).with_blend(Channel::BaseColor, BlendMode::Multiply, 0.8).with_blend(
        Channel::Height,
        BlendMode::SignedAdd,
        0.25,
    );
    fill.opacity = 0.37;
    let material = Fill::new(FillSource::Material {
        path: "materials/rust.mca".into(),
        params: vec![("Amount".into(), "0.4".into()), ("Farbe \"äöü\"".into(), "#ff0000".into())],
    })
    .replacing(Procedural::new("substance:rust_fine").with_params(vec![("Rust_Amount".into(), "0.5".into())]));
    let mut rust = Layer::new("rust", "Rust", LayerKind::Fill(material)).with_blend(Channel::Metallic, BlendMode::Overlay, 1.0).with_mask(noisy(
        w,
        h,
        PixelFormat::from_id("gray8").unwrap(),
        7,
    ));
    rust.visible = false;
    let inner = Layer::new(
        "inner",
        "Inner",
        LayerKind::Group(vec![paint("deep", vec![(Channel::Normal, noisy(w, h, ChannelSpec::new(Channel::Normal).layer_format(), 9))])]),
    )
    .with_blend(Channel::Normal, BlendMode::NormalMapDetail, 0.5);
    let group = Layer::new("group", "Group", LayerKind::Group(vec![rust, inner, Layer::new("empty-group", "", LayerKind::Group(vec![]))])).with_blend(
        Channel::BaseColor,
        BlendMode::Passthrough,
        1.0,
    );
    set.layers.insert(0, fill);
    set.layers.push(group);
    set.layers.push(super::effect_fixtures::effects_layer(w, h));
    set.layers.push(super::effect_fixtures::effects_group());
    for (kind, size, f) in [
        (BakedKind::AmbientOcclusion, (150, 130), "gray8"),
        (BakedKind::Normal, (64, 64), "rgb16"),
        (BakedKind::Position, (75, 65), "rgb32f"),
        (BakedKind::Curvature, (20, 10), "gray16f"),
        (BakedKind::Curvature, (20, 10), "gray16"),
        (BakedKind::Other("bent normal/x".into()), (3, 3), "rgba8"),
    ] {
        let f = PixelFormat::from_id(f).unwrap();
        set.baked.push(BakedMap { kind, image: noisy(size.0, size.1, f, size.0) });
    }
    set
}

/// The full test document: two material slots, a second UV set, an empty texture set, ids that
/// collide once made path-safe, foreign files and an import report.
pub fn full_document() -> Document {
    let mesh = Mesh { materials: vec!["Body".into(), "Trim".into()], parts: vec![quad("body", 0, 1, 0.0), quad("trim", 1, 2, 1.0), quad("trim 2", 1, 3, 2.0)] };
    let mut doc = Document::new(mesh);
    doc.height_filter = HeightFilter::Sobel;
    doc.texture_sets.push(body_set());
    let mut trim = TextureSet::new("trim", "Trim", (32, 32));
    trim.material_slot = 1;
    trim.uv_set = 1;
    doc.texture_sets.push(trim);
    let mut odd = TextureSet::new("odd/../id", "Odd", (17, 300)).with_channel(Channel::Roughness);
    let fmt = ChannelSpec::new(Channel::Roughness).layer_format();
    odd.layers.push(paint("x y", vec![(Channel::Roughness, noisy(17, 300, fmt, 1))]));
    odd.layers.push(paint("x_y", vec![(Channel::Roughness, noisy(17, 300, fmt, 2))]));
    doc.texture_sets.push(odd);
    let mut rng = Rng::new(3);
    doc.foreign.push(ForeignFile { path: "foreign/substance/document.bin".into(), bytes: (0..50_000).map(|_| rng.next() as u8).collect() });
    doc.foreign.push(ForeignFile { path: "foreign/substance/resources/0a1b.sbsar".into(), bytes: b"sbsar".repeat(1000) });
    doc.foreign.push(ForeignFile { path: "foreign/empty".into(), bytes: vec![] });
    let mut report = ImportReport::new("substance-painter 9.0.0: robot.spp");
    report.add("texture set \"Body\" / layer \"Rust\"", Fidelity::Approximated, "blend mode");
    report.add("layer \"Dirt\"", Fidelity::Baked, "");
    report.add("layer \"Exact\"", Fidelity::Exact, "ok");
    report.add("anchor", Fidelity::KeptRaw, "kept in foreign/");
    doc.import_report = Some(report);
    doc.validate().unwrap();
    doc
}

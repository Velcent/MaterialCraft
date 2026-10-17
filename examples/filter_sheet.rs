//! Dev helper: runs every filter (`materialcraft::filters`) on a test pattern and writes out_dir/filters.png,
//! a contact sheet (input first, then each filter at settings that show it clearly), and out_dir/<id>.png per
//! filter. The texture is cut into two UV islands that touch along the vertical middle line (two quads far
//! apart on the mesh): neighbourhood filters must leave that line crisp, nothing bleeds from one half into the
//! other. Masks are run as masks (grey), the colour filters on base colour. For judging the filters by eye.
use materialcraft::filters::{FILTERS, filter};
use materialcraft::mesh::{Mesh, MeshPart, SurfaceKey, SurfaceMap};
use materialcraft::paint::effect::{EvalRequest, EvalTarget, FilterInput};
use materialcraft::paint::{Channel, Rect, TextureSet};
use materialcraft::tex::Tex;

const SIZE: u32 = 256;

fn quad(name: &str, x: f32, u0: f32, u1: f32) -> MeshPart {
    MeshPart {
        name: name.into(),
        material: 0,
        positions: vec![[x, -1.0, 0.0], [x + 1.0, -1.0, 0.0], [x + 1.0, 1.0, 0.0], [x, 1.0, 0.0]],
        normals: vec![[0.0, 0.0, 1.0]; 4],
        tangents: vec![[1.0, 0.0, 0.0, 1.0]; 4],
        uv_sets: vec![vec![[u0, 1.0], [u1, 1.0], [u1, 0.0], [u0, 0.0]]],
        triangles: vec![[0, 1, 2], [0, 2, 3]],
    }
}

/// Shapes for the mask filters (a disc crossing the island border, a ring, a bar) on a soft gradient, and a
/// colour version for the colour filters.
fn pattern(color: bool) -> Vec<[f32; 4]> {
    let n = SIZE as f32;
    (0..SIZE * SIZE)
        .map(|i| {
            let (x, y) = ((i % SIZE) as f32 + 0.5, (i / SIZE) as f32 + 0.5);
            let disc = (x - 0.45 * n).hypot(y - 0.3 * n) < 0.18 * n;
            let ring = ((x - 0.25 * n).hypot(y - 0.75 * n) - 0.12 * n).abs() < 0.03 * n;
            let bar = (x - 0.75 * n).abs() < 0.08 * n && y > 0.55 * n && y < 0.92 * n;
            let shape = if disc || ring || bar { 1.0 } else { 0.15 * y / n };
            if color {
                let (u, v) = (x / n, y / n);
                [0.2 + 0.6 * u * shape.max(0.3), 0.15 + 0.5 * v, 0.6 - 0.4 * shape * u, 1.0]
            } else {
                [shape, shape, shape, 1.0]
            }
        })
        .collect()
}

/// Settings that make each filter visible on the pattern.
fn settings(id: &str) -> Vec<(String, String)> {
    let p: &[(&str, &str)] = match id {
        "hsl" => &[("hue", "0.3"), ("saturation", "0.5")],
        "blur" => &[("radius", "0.02")],
        "sharpen" => &[("radius", "0.01"), ("amount", "2")],
        "bevel" => &[("distance", "0.05"), ("round", "1")],
        "mask_outline" => &[("width", "0.03"), ("curve", "0.5")],
        "slope_blur" => &[("intensity", "0.04"), ("mode", "2"), ("scale", "2")],
        "warp" => &[("intensity", "0.03"), ("scale", "2")],
        "height_adjust" => &[("multiplier", "0.5"), ("offset", "0.3")],
        "finish_rough" => &[("intensity", "0.4")],
        "clamp" => &[("min", "0.1"), ("max", "0.6")],
        "histogram_scan" => &[("position", "0.6"), ("contrast", "0.8")],
        "histogram_shift" => &[("position", "0.3")],
        "contrast_luminosity" => &[("contrast", "0.5"), ("luminosity", "0.1")],
        _ => &[],
    };
    p.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "filter_sheet".into());
    std::fs::create_dir_all(&dir).unwrap();
    let mesh = Mesh { materials: vec!["m".into()], parts: vec![quad("a", 0.0, 0.0, 0.5), quad("b", 5.0, 0.5, 1.0)] };
    let surface = SurfaceMap::build(&mesh, SurfaceKey::square(&mesh, 0, SIZE)).unwrap();
    let set = TextureSet::new("s", "S", (SIZE, SIZE)).with_channel(Channel::BaseColor).with_channel(Channel::Roughness);
    let rect = Rect { x: 0, y: 0, w: SIZE, h: SIZE };
    let mut cells: Vec<(String, Vec<[f32; 4]>)> = vec![("input".into(), pattern(false))];
    for info in FILTERS {
        let target = match info.id {
            "hsl" => EvalTarget::Channel(Channel::BaseColor),
            "finish_rough" => EvalTarget::Channel(Channel::Roughness),
            _ => EvalTarget::Mask,
        };
        let input = pattern(info.id == "hsl");
        let params = settings(info.id);
        let request = EvalRequest { set: &set, target, rect, params: &params, stack: None, surface: Some(&surface) };
        let mut out = vec![[0.0; 4]; rect.area()];
        filter(info.id).unwrap().apply(&request, &FilterInput { rect, pixels: &input }, &mut out).unwrap();
        // colour values are linear light in the paint model; show them encoded
        if info.id == "hsl" {
            for p in &mut out {
                for c in &mut p[..3] {
                    *c = materialcraft::paint::working_space::linear_to_srgb(c.clamp(0.0, 1.0));
                }
            }
        }
        std::fs::write(format!("{dir}/{}.png", info.id), materialcraft::png::encode(SIZE as usize, SIZE as usize, &out, 8)).unwrap();
        println!("{:<20} {}", info.id, info.label);
        cells.push((info.id.to_string(), out));
    }
    let cols = 5usize;
    let s = SIZE as usize;
    let rows = cells.len().div_ceil(cols);
    let mut sheet = Tex::solid(cols * (s + 8), rows * (s + 8), [0.2, 0.2, 0.25, 1.0]);
    for (i, (_, px)) in cells.iter().enumerate() {
        let (ox, oy) = ((i % cols) * (s + 8) + 4, (i / cols) * (s + 8) + 4);
        for y in 0..s {
            for x in 0..s {
                sheet.px[(oy + y) * sheet.w + ox + x] = px[y * s + x];
            }
        }
    }
    std::fs::write(format!("{dir}/filters.png"), materialcraft::png::encode(sheet.w, sheet.h, &sheet.px, 8)).unwrap();
    println!("sheet: {dir}/filters.png (input, then the filters in this order, {cols} per row)");
}

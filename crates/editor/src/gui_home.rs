// SPDX-License-Identifier: MIT OR Apache-2.0
// ======================================================================= Home screen, dialogs, panels (included into gui.rs)

#[derive(PartialEq, Clone, Copy)]
enum Screen {
    Home,
    Editor,
    Paint,
}

struct NewDlg {
    open: bool,
    sel: usize,
    name: String,
    size: usize,
    /// 0 = linked to the width (square), otherwise the separate height
    height: usize,
    depth16: bool,
    /// Package the graph is added to; None = a new package.
    dest: Option<usize>,
}

struct Template {
    name: &'static str,
    desc: &'static str,
    outs: &'static [(&'static str, [f32; 3])],
}

const G5: [f32; 3] = [0.5, 0.5, 0.5];
const TEMPLATES: &[Template] = &[
    Template { name: "Empty", desc: "An empty graph with no nodes and no outputs.", outs: &[] },
    Template {
        name: "Metallic Roughness",
        desc: "PBR metal/roughness workflow. Outputs: basecolor, normal, roughness, metallic, height, ambient_occlusion.",
        outs: &[("basecolor", G5), ("normal", [0.5, 0.5, 1.0]), ("roughness", G5), ("metallic", [0.0; 3]), ("height", G5), ("ambient_occlusion", [1.0; 3])],
    },
    Template {
        name: "Metallic Roughness - Emissive",
        desc: "Metal/roughness plus an emissive output.",
        outs: &[
            ("basecolor", G5),
            ("normal", [0.5, 0.5, 1.0]),
            ("roughness", G5),
            ("metallic", [0.0; 3]),
            ("height", G5),
            ("ambient_occlusion", [1.0; 3]),
            ("emissive", [0.0; 3]),
        ],
    },
    Template {
        name: "Metallic Roughness - Opacity",
        desc: "Metal/roughness plus an opacity output.",
        outs: &[
            ("basecolor", G5),
            ("normal", [0.5, 0.5, 1.0]),
            ("roughness", G5),
            ("metallic", [0.0; 3]),
            ("height", G5),
            ("ambient_occlusion", [1.0; 3]),
            ("opacity", [1.0; 3]),
        ],
    },
    Template {
        name: "Specular Glossiness",
        desc: "Specular/glossiness workflow. Outputs: diffuse, specular, glossiness, normal, height, ambient_occlusion.",
        outs: &[("diffuse", G5), ("specular", [0.04; 3]), ("glossiness", G5), ("normal", [0.5, 0.5, 1.0]), ("height", G5), ("ambient_occlusion", [1.0; 3])],
    },
    Template { name: "Generator (grayscale)", desc: "A single grayscale output called 'output'.", outs: &[("output", G5)] },
    Template { name: "Generator (color)", desc: "A single color output called 'output'.", outs: &[("output", [0.6, 0.4, 0.3])] },
];

fn template_graph(t: &Template, name: &str, size: usize, height: usize, depth16: bool) -> Graph {
    let mut g = Graph { size, height: if height == size { 0 } else { height }, ..Default::default() };
    g.meta.insert("identifier".into(), name.to_string());
    g.meta.insert("output_format".into(), if depth16 { "16" } else { "8" }.into());
    for (i, (out, _)) in t.outs.iter().enumerate() {
        let id = i as u32 + 1;
        let mut n = Node { kind: "output".into(), pos: [(i / 3) as f32 * (CARD + 80.0), (i % 3) as f32 * (CARD_H + 44.0)], ..Default::default() };
        n.strs.insert("usage".into(), out.to_string());
        g.nodes.insert(id, n);
    }
    g
}

// ---------------------------------------------------------------- config / recent files
fn config_dir() -> PathBuf {
    if let Ok(a) = std::env::var("APPDATA") {
        return PathBuf::from(a).join("MaterialCraft");
    }
    if let Ok(x) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(x).join("materialcraft");
    }
    if let Ok(h) = std::env::var("HOME") {
        let base = PathBuf::from(h);
        return if cfg!(target_os = "macos") { base.join("Library/Application Support/MaterialCraft") } else { base.join(".config/materialcraft") };
    }
    std::env::temp_dir().join("materialcraft")
}

fn read_lines(name: &str) -> Vec<String> {
    std::fs::read_to_string(config_dir().join(name)).map(|t| t.lines().map(String::from).filter(|l| !l.is_empty()).collect()).unwrap_or_default()
}

/// Accent colour of the interface (selection, active tabs, primary buttons).
const ACCENT: Color32 = Color32::from_rgb(60, 200, 170);

/// A small abstract picture generated from a file name (no two names look alike, the same name always does).
fn swatch(p: &egui::Painter, r: Rect, name: &str, alive: bool) {
    let mut h: u32 = 2166136261;
    for b in name.bytes() {
        h = (h ^ b as u32).wrapping_mul(16777619);
    }
    let mut next = || {
        h ^= h << 13;
        h ^= h >> 17;
        h ^= h << 5;
        (h & 0xffff) as f32 / 65535.0
    };
    let hue = |v: f32, s: f32, l: f32| {
        let c = egui::ecolor::Hsva::new(v, s, l, 1.0);
        let [r, g, b, _] = c.to_srgba_unmultiplied();
        Color32::from_rgb(r, g, b)
    };
    let (h0, h1) = (next(), next());
    let dimf = if alive { 1.0 } else { 0.35 };
    p.rect_filled(r, 8.0, hue(h0, 0.45 * dimf, 0.32));
    for _ in 0..4 {
        let c = pos2(r.min.x + next() * r.width(), r.min.y + next() * r.height());
        p.circle_filled(c, 6.0 + next() * 18.0, hue((h1 + next() * 0.2).fract(), 0.5 * dimf, 0.45 + next() * 0.25).gamma_multiply(0.85));
    }
    let y = r.min.y + r.height() * (0.3 + next() * 0.5);
    p.rect_filled(Rect::from_min_max(pos2(r.min.x, y), pos2(r.max.x, y + 5.0)), 0.0, hue(h1, 0.3 * dimf, 0.78).gamma_multiply(0.5));
}

fn fmt_time(t: std::time::SystemTime) -> String {
    let secs = t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let (days, sod) = (secs.div_euclid(86400), secs.rem_euclid(86400));
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}", sod / 3600, (sod % 3600) / 60)
}

fn fmt_size(b: u64) -> String {
    if b >= 1 << 20 { format!("{:.2} MB", b as f64 / (1u64 << 20) as f64) } else { format!("{} KB", (b + 1023) / 1024) }
}

// ---------------------------------------------------------------- background asset generation
fn spawn_banner() -> Receiver<ColorImage> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let src = "size 512\nnode 1 fbm scale=3 octaves=6 gain=0.55 seed=11\nnode 2 fbm scale=9 octaves=5 seed=4\nnode 3 warp in=@1 warp=@2 intensity=0.14\nnode 4 voronoi scale=6 jitter=0.9 seed=5 mode=2\nnode 5 blend fg=@4 bg=@3 mode=5 opacity=0.4\nnode 6 colorize in=@5 p0=0 r0=0.25 g0=0.17 b0=0.12 p1=0.5 r1=0.62 g1=0.45 b1=0.32 p2=1 r2=0.92 g2=0.78 b2=0.6\noutput o 6\n";
        if let Ok(g) = Graph::parse(src) {
            if let Ok(m) = g.evaluate(Some(512)) {
                if let Some(t) = m.get("o") {
                    let _ = tx.send(qimg(t));
                }
            }
        }
    });
    rx
}

fn spawn_library_thumbs() -> Receiver<HashMap<String, ColorImage>> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let mut out = HashMap::new();
        for s in SPECS {
            if let Some(text) = materialcraft::graph::demo_graph_for(s.kind) {
                if let Ok(g) = Graph::parse(&text) {
                    if let Ok(m) = g.evaluate(Some(64)) {
                        if let Some(tex) = m.get("o") {
                            out.insert(s.kind.to_string(), qimg(tex));
                        }
                    }
                }
            }
        }
        let _ = tx.send(out);
    });
    rx
}

// ---------------------------------------------------------------- icons
#[derive(Clone, Copy)]
enum Ic {
    New,
    Open,
    Save,
    Undo,
    Redo,
    Frame,
    Layout,
    Comment,
    Subgraph,
    Link,
    Grid,
    Star,
    Sphere,
    Dot,
    Input,
    Package,
    GraphIcon,
}

fn draw_icon(p: &egui::Painter, c: Pos2, ic: Ic, col: Color32) {
    let st = Stroke::new(1.4_f32, col);
    let r = |dx: f32, dy: f32, w: f32, h: f32| Rect::from_min_size(pos2(c.x + dx, c.y + dy), vec2(w, h));
    let l = |a: (f32, f32), b: (f32, f32)| p.line_segment([pos2(c.x + a.0, c.y + a.1), pos2(c.x + b.0, c.y + b.1)], st);
    match ic {
        Ic::New => {
            p.rect_stroke(r(-5.0, -7.0, 10.0, 14.0), 1.0, st, egui::StrokeKind::Inside);
            l((-2.0, 0.0), (2.0, 0.0));
            l((0.0, -2.0), (0.0, 2.0));
        }
        Ic::Open => {
            p.rect_stroke(r(-7.0, -4.0, 14.0, 10.0), 1.0, st, egui::StrokeKind::Inside);
            l((-7.0, -4.0), (-7.0, -7.0));
            l((-7.0, -7.0), (-2.0, -7.0));
            l((-2.0, -7.0), (0.0, -4.0));
        }
        Ic::Save => {
            p.rect_stroke(r(-6.0, -6.0, 12.0, 12.0), 1.0, st, egui::StrokeKind::Inside);
            p.rect_stroke(r(-3.5, -6.0, 7.0, 4.0), 0.0, st, egui::StrokeKind::Inside);
            p.rect_stroke(r(-3.5, 1.0, 7.0, 5.0), 0.0, st, egui::StrokeKind::Inside);
        }
        Ic::Undo | Ic::Redo => {
            let s = if matches!(ic, Ic::Undo) { 1.0 } else { -1.0 };
            let mut prev = pos2(c.x - 5.0 * s, c.y - 1.0);
            for i in 1..=8 {
                let a = std::f32::consts::PI * (1.0 + i as f32 / 8.0 * 1.15);
                let q = pos2(c.x + 1.0 * s + a.cos() * 6.0 * s, c.y + 2.0 + a.sin() * 5.0);
                p.line_segment([prev, q], st);
                prev = q;
            }
            l((-5.0 * s, -1.0), (-7.5 * s, -4.5));
            l((-5.0 * s, -1.0), (-1.5 * s, -2.5));
        }
        Ic::Frame => {
            for (sx, sy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                l((sx * 7.0, sy * 7.0), (sx * 7.0, sy * 3.5));
                l((sx * 7.0, sy * 7.0), (sx * 3.5, sy * 7.0));
            }
        }
        Ic::Layout => {
            p.rect_stroke(r(-7.0, -6.0, 4.0, 4.0), 0.0, st, egui::StrokeKind::Inside);
            p.rect_stroke(r(-7.0, 2.0, 4.0, 4.0), 0.0, st, egui::StrokeKind::Inside);
            p.rect_stroke(r(3.0, -2.0, 4.0, 4.0), 0.0, st, egui::StrokeKind::Inside);
            l((-3.0, -4.0), (0.0, 0.0));
            l((-3.0, 4.0), (0.0, 0.0));
            l((0.0, 0.0), (3.0, 0.0));
        }
        Ic::Comment => {
            p.rect_stroke(r(-7.0, -6.0, 14.0, 12.0), 1.0, st, egui::StrokeKind::Inside);
            l((-7.0, -2.5), (7.0, -2.5));
        }
        Ic::Subgraph => {
            p.rect_stroke(r(-7.0, -7.0, 14.0, 14.0), 1.0, st, egui::StrokeKind::Inside);
            p.rect_stroke(r(-3.0, -3.0, 6.0, 6.0), 0.0, st, egui::StrokeKind::Inside);
        }
        Ic::Link => {
            p.rect_stroke(r(-8.0, -3.0, 9.0, 6.0), 3.0, st, egui::StrokeKind::Inside);
            p.rect_stroke(r(-1.0, -3.0, 9.0, 6.0), 3.0, st, egui::StrokeKind::Inside);
        }
        Ic::Grid => {
            for k in [-3.0f32, 3.0] {
                l((k, -7.0), (k, 7.0));
                l((-7.0, k), (7.0, k));
            }
        }
        Ic::Sphere => {
            p.circle_stroke(c, 6.0, st);
            let pts: Vec<Pos2> = (0..=20)
                .map(|k| {
                    let a = k as f32 / 20.0 * std::f32::consts::TAU;
                    pos2(c.x + a.cos() * 6.0, c.y + a.sin() * 2.4)
                })
                .collect();
            p.add(egui::Shape::line(pts, Stroke::new(1.0_f32, col)));
        }
        Ic::Dot => {
            p.circle_stroke(c, 5.0, st);
            p.circle_filled(c, 1.8, col);
        }
        Ic::Input => {
            p.rect_stroke(r(-2.0, -6.0, 8.0, 12.0), 1.0, st, egui::StrokeKind::Inside);
            l((-8.0, 0.0), (1.0, 0.0));
            l((-3.5, -3.5), (1.0, 0.0));
            l((-3.5, 3.5), (1.0, 0.0));
        }
        Ic::Package => {
            let top = [pos2(c.x, c.y - 7.0), pos2(c.x + 6.5, c.y - 3.5), pos2(c.x, c.y), pos2(c.x - 6.5, c.y - 3.5)];
            p.add(egui::Shape::closed_line(top.to_vec(), st));
            l((-6.5, -3.5), (-6.5, 3.5));
            l((6.5, -3.5), (6.5, 3.5));
            l((0.0, 0.0), (0.0, 7.0));
            l((-6.5, 3.5), (0.0, 7.0));
            l((6.5, 3.5), (0.0, 7.0));
        }
        Ic::GraphIcon => {
            p.rect_stroke(r(-7.0, -6.0, 5.0, 4.0), 1.0, st, egui::StrokeKind::Inside);
            p.rect_stroke(r(-7.0, 2.0, 5.0, 4.0), 1.0, st, egui::StrokeKind::Inside);
            p.rect_stroke(r(3.0, -2.0, 5.0, 4.0), 1.0, st, egui::StrokeKind::Inside);
            l((-2.0, -4.0), (0.5, 0.0));
            l((-2.0, 4.0), (0.5, 0.0));
            l((0.5, 0.0), (3.0, 0.0));
        }
        Ic::Star => {
            let mut pts = Vec::new();
            for i in 0..10 {
                let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
                let rad = if i % 2 == 0 { 7.0 } else { 3.0 };
                pts.push(pos2(c.x + a.cos() * rad, c.y + a.sin() * rad));
            }
            p.add(egui::Shape::closed_line(pts, st));
        }
    }
}

fn explorer_icon(ui: &mut egui::Ui, ic: Ic) {
    let (rect, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
    draw_icon(ui.painter(), rect.center(), ic, Color32::from_gray(200));
}

/// A graph row in the Explorer: graph icon, name, and (expandable) its outputs. Returns the name's response.
fn graph_row(ui: &mut egui::Ui, name: &str, outs: &[String], selected: bool, key: (&str, String)) -> egui::Response {
    let id = ui.make_persistent_id(("grow", key.0, key.1));
    let mut resp = None;
    let head = |ui: &mut egui::Ui, resp: &mut Option<egui::Response>| {
        explorer_icon(ui, Ic::GraphIcon);
        let txt = if selected { egui::RichText::new(name).strong().color(Color32::WHITE) } else { egui::RichText::new(name) };
        *resp = Some(ui.add(egui::Label::new(txt).selectable(false).sense(Sense::click_and_drag())));
    };
    if outs.is_empty() {
        ui.horizontal(|ui| {
            ui.add_space(18.0);
            head(ui, &mut resp);
        });
    } else {
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false).show_header(ui, |ui| head(ui, &mut resp)).body(|ui| {
            for o in outs {
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    ui.small(usage_label(o));
                });
            }
        });
    }
    resp.unwrap_or_else(|| ui.label(""))
}

fn entry_icon(kind: &str) -> Ic {
    match kind {
        "output" => Ic::Sphere,
        "comment" => Ic::Comment,
        "frame" => Ic::Frame,
        "dot" => Ic::Dot,
        "input" => Ic::Input,
        "subgraph" => Ic::Subgraph,
        _ => match materialcraft::graph::category(kind) {
            "Generators" => Ic::Grid,
            "Material" => Ic::Subgraph,
            _ => Ic::Layout,
        },
    }
}

fn icon_btn(ui: &mut egui::Ui, ic: Ic, tip: &str, enabled: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(26.0, 24.0), if enabled { Sense::click() } else { Sense::hover() });
    if resp.hovered() && enabled {
        ui.painter().rect_filled(rect, 3.0, Color32::from_rgb(72, 72, 72));
    }
    draw_icon(ui.painter(), rect.center(), ic, if enabled { Color32::from_gray(215) } else { Color32::from_gray(95) });
    resp.on_hover_text(tip)
}

/// Dockable-panel style header. Returns true when the close button was clicked.
fn panel_header(ui: &mut egui::Ui, title: &str, _closable: bool) -> bool {
    // closing and moving panels is done from the tab's panel menu (see the dock area); the tab already names these panels
    if matches!(title, "EXPLORER" | "LIBRARY" | "2D VIEW") {
        return false;
    }
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(30, 30, 30));
    ui.painter().text(rect.left_center() + vec2(8.0, 0.0), Align2::LEFT_CENTER, title, FontId::proportional(12.0), Color32::from_gray(225));
    false
}

fn lib_group(kind: &str) -> (&'static str, &'static str) {
    match kind {
        "white_noise" | "gaussian_noise" | "perlin" | "fbm" | "clouds" | "voronoi" | "voronoi_fractal" | "bnw_spots" | "crystal" | "directional_noise"
        | "grunge" | "grunge_map" | "plasma" | "cells" | "fractal_sum" | "waveform" | "gaussian_spots" => ("Generators", "Noises"),
        "triangle_grid" | "starburst" | "gradient_reflected" => ("Generators", "Patterns"),
        "color" | "gradient" | "gradient_axial" | "gradient_circular" | "checker" | "bricks" | "stripes" | "weave" | "weave_generator" | "tile_sampler"
        | "tile_generator" | "splatter" | "splatter_circular" | "pixel_processor" => ("Generators", "Patterns"),
        "shape" | "circle" | "rect" | "polygon" | "star" => ("Generators", "Shapes"),
        "sdf_shape" | "sdf_op" | "sdf_mask" | "sdf_form" => ("Generators", "SDF"),
        "levels"
        | "adjust"
        | "invert"
        | "threshold"
        | "histogram_scan"
        | "histogram_range"
        | "quantize"
        | "quantize_color"
        | "clamp"
        | "hsl"
        | "colorize"
        | "tint"
        | "to_gray"
        | "rgba_split"
        | "alpha_split"
        | "channel"
        | "merge"
        | "switch"
        | "color_match"
        | "highpass"
        | "auto_levels"
        | "channel_mixer"
        | "alpha_merge"
        | "color_to_mask"
        | "replace_color"
        | "pow"
        | "histogram_shift"
        | "grayscale_conversion"
        | "chrominance_extract" => ("Filters", "Adjustments"),
        "blur"
        | "blur_hq"
        | "directional_blur"
        | "sharpen"
        | "warp"
        | "directional_warp"
        | "vector_warp"
        | "slope_blur"
        | "swirl"
        | "non_uniform_blur"
        | "radial_blur"
        | "anisotropic_blur"
        | "non_uniform_directional_warp"
        | "multi_directional_warp" => ("Filters", "Blur and warp"),
        "transform" | "mirror" | "make_it_tile_photo" | "make_it_tile_patch" | "symmetry" | "skew" => ("Filters", "Transform"),
        "glow" | "mosaic" | "shape_drop_shadow" | "shape_stroke" | "shape_glow" | "sobel" => ("Filters", "Effects"),
        "distance" | "bevel" | "edge_detect" | "emboss" | "flood_fill" | "flood_fill_random" | "flood_fill_color" | "flood_fill_gradient" => {
            ("Filters", "Masks and regions")
        }
        "normal"
        | "normal_blend"
        | "normal_combine"
        | "normal_invert"
        | "normal_normalize"
        | "normal_vector_rotation"
        | "normal_to_height"
        | "ao"
        | "curvature"
        | "curvature_smooth"
        | "height_blend" => ("Material tools", ""),
        "edge_wear" | "dirt" | "dust" => ("Mesh-driven", ""),
        "bitmap" | "input" | "subgraph" | "comment" | "frame" | "dot" | "output" => ("Structure", ""),
        _ => ("Filters", "Adjustments"),
    }
}

const LIB_TREE: &[(&str, &[&str])] = &[
    ("Favorites", &[]),
    ("Structure", &[]),
    ("Core nodes", &[]),
    ("Pattern builders", &[]),
    ("Logic nodes", &[]),
    ("Generators", &["Noises", "Patterns", "Shapes", "SDF"]),
    ("Filters", &["Adjustments", "Blur and warp", "Transform", "Effects", "Masks and regions"]),
    ("Material tools", &[]),
    ("Mesh-driven", &[]),
];

fn meta_field(ui: &mut egui::Ui, meta: &mut BTreeMap<String, String>, label: &str, key: &str, multi: bool) {
    ui.strong(label);
    let mut v = meta.get(key).cloned().unwrap_or_default();
    let resp = if multi {
        ui.add(egui::TextEdit::multiline(&mut v).desired_rows(4).desired_width(f32::INFINITY))
    } else {
        ui.add(egui::TextEdit::singleline(&mut v).desired_width(f32::INFINITY))
    };
    if resp.changed() {
        meta.insert(key.to_string(), v);
    }
    ui.add_space(4.0);
}

fn two_lines(label: &str) -> (String, String) {
    let t = label.replace('_', " ").replace("subgraph: ", "");
    if t.len() <= 11 {
        return (t, String::new());
    }
    let mid = t.len() / 2;
    let split = t.char_indices().filter(|(_, c)| *c == ' ').map(|(i, _)| i).min_by_key(|i| (*i as i32 - mid as i32).abs());
    match split {
        Some(i) => (t[..i].to_string(), t[i + 1..].to_string()),
        None => (t.chars().take(11).collect(), String::new()),
    }
}

impl App {
    fn gname(&self) -> String {
        let root = self.parents.first().map(|p| &p.0).unwrap_or(&self.g);
        root.meta.get("identifier").cloned().filter(|s| !s.is_empty()).unwrap_or_else(|| "Graph".into())
    }

    fn package_label(&self) -> String {
        match &self.file {
            Some(p) => p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
            None => self.unsaved_name.clone(),
        }
    }

    fn push_recent(&mut self, p: &std::path::Path) {
        self.recent.retain(|r| r != p);
        self.recent.insert(0, p.to_path_buf());
        self.recent.truncate(20);
        let files: Vec<String> = self.recent.iter().map(|r| r.to_string_lossy().to_string()).collect();
        self.prefs.update(|v| v.recent.files = files);
    }

    fn save_settings(&mut self) {
        let (skip, tip, fav) = (self.skip_home, self.hide_suggestions, self.favorites.iter().cloned().collect::<Vec<_>>());
        self.prefs.update(|v| {
            v.general.skip_home = skip;
            v.general.hide_tip = tip;
            v.library.favorites = fav;
        });
    }

    fn open_path(&mut self, p: PathBuf) {
        let link_note: String;
        match std::fs::read_to_string(&p).map_err(|e| e.to_string()).and_then(|t| Graph::parse_unchecked(&t).map_err(|e| e.to_string())) {
            Ok(mut g) => {
                g.migrate_outputs();
                let issues = g.resolve_links(p.parent().unwrap_or_else(|| std::path::Path::new(".")));
                link_note = if issues.is_empty() { String::new() } else { format!("  -  {}", issues.join("; ")) };
                auto_layout_if_unplaced(&mut g);
                if !g.meta.contains_key("identifier") {
                    g.meta.insert("identifier".into(), p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "graph".into()));
                }
                self.ws_fresh();
                self.parents.clear();
                self.g = g;
                self.clear_views();
                self.push_recent(&p);
                self.file = Some(p.clone());
                self.clear_history();
                self.thumbs.clear();
                self.modified = false;
                self.screen = Screen::Editor;
                self.fit_view();
                let ok = link_note.is_empty();
                self.status(format!("Opened {}{link_note}", p.display()), ok);
            }
            Err(e) => self.status(format!("Open failed: {e}"), false),
        }
    }

    fn open_new_dialog(&mut self, dest: Option<usize>) {
        self.ws_init();
        self.new_dlg.dest = if self.screen == Screen::Editor { dest } else { None };
        self.new_dlg.name = if self.new_dlg.dest.is_some() { "Material_Graph".into() } else { "Graph".into() };
        self.new_dlg.open = true;
    }

    fn create_from_dialog(&mut self) {
        let t = &TEMPLATES[self.new_dlg.sel.min(TEMPLATES.len() - 1)];
        let mut name: String = self.new_dlg.name.trim().replace(' ', "_");
        if name.is_empty() {
            name = if self.new_dlg.dest.is_some() { "Material_Graph".into() } else { "Graph".into() };
        }
        let g = template_graph(t, &name, self.new_dlg.size, self.new_dlg.height, self.new_dlg.depth16);
        self.new_dlg.open = false;
        match self.new_dlg.dest.filter(|_| self.screen == Screen::Editor) {
            Some(dest) => self.add_graph_to_pkg(dest, g, &name),
            None => {
                let unsaved = if self.screen == Screen::Editor { self.unique_unsaved_name() } else { "Unsaved Package".to_string() };
                self.add_pkg_active(Pkg { path: None, name: unsaved, g, modified: true, tabs: Vec::new() });
                self.need_fit = true;
                self.status(format!("Created package with '{name}' from template '{}'", t.name), true);
            }
        }
    }

    fn open_example(&mut self) {
        if let Ok(mut g) = Graph::parse(EXAMPLE) {
            g.migrate_outputs();
            auto_layout(&mut g);
            g.meta.insert("identifier".into(), "Brick_wall".into());
            self.ws_fresh();
            self.parents.clear();
            self.g = g;
            self.clear_views();
            self.file = None;
            self.clear_history();
            self.thumbs.clear();
            self.modified = false;
            self.screen = Screen::Editor;
            self.fit_view();
        }
    }

    // ------------------------------------------------------------ home screen
    /// Start screen: a centred launch pad (big action cards, then recent files as a card grid) with a slim top bar
    /// and a footer strip. Recent files get a small generated swatch derived from their name.
    fn home_ui(&mut self, root: &mut egui::Ui) {
        let bg = Color32::from_rgb(21, 23, 26);
        let card = Color32::from_rgb(31, 34, 39);
        let card_hot = Color32::from_rgb(40, 45, 52);
        let dim = Color32::from_gray(140);
        egui::Panel::top("home_top")
            .exact_size(46.0)
            .frame(egui::Frame::NONE.fill(Color32::from_rgb(26, 28, 32)).inner_margin(egui::Margin::symmetric(18, 0)))
            .show(root, |ui| {
                ui.horizontal_centered(|ui| {
                    if let Some(t) = &self.logo {
                        let sz = t.size_vec2();
                        let hgt = 28.0;
                        ui.add(egui::Image::new((t.id(), vec2(hgt * sz.x / sz.y.max(1.0), hgt))));
                    }
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("MaterialCraft").size(15.0).strong().color(Color32::from_gray(225)));
                    ui.add_space(24.0);
                    for (i, label) in ["Start", "Learn"].iter().enumerate() {
                        let sel = self.home_page == i as u8;
                        let txt = egui::RichText::new(*label).size(13.0).color(if sel { ACCENT } else { Color32::from_gray(170) });
                        let r = ui.add(egui::Button::new(txt).frame(false).min_size(vec2(56.0, 30.0)));
                        if sel {
                            let y = r.rect.bottom() - 1.0;
                            ui.painter().line_segment([pos2(r.rect.left() + 8.0, y), pos2(r.rect.right() - 8.0, y)], Stroke::new(2.0_f32, ACCENT));
                        }
                        if r.clicked() {
                            self.home_page = i as u8;
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION"))).size(11.0).color(Color32::from_gray(110)));
                    });
                });
            });
        egui::Panel::bottom("home_foot")
            .exact_size(38.0)
            .frame(egui::Frame::NONE.fill(Color32::from_rgb(26, 28, 32)).inner_margin(egui::Margin::symmetric(18, 0)))
            .show(root, |ui| {
                ui.horizontal_centered(|ui| {
                    if ui.checkbox(&mut self.skip_home, "Skip this screen on startup").changed() {
                        self.save_settings();
                    }
                    ui.add_space(10.0);
                    if ui.add(egui::Button::new("Keyboard shortcuts").frame(false)).clicked() {
                        self.show_about = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(egui::RichText::new("Go to editor").color(Color32::BLACK).strong())
                                    .fill(ACCENT)
                                    .corner_radius(6.0)
                                    .min_size(vec2(110.0, 24.0)),
                            )
                            .clicked()
                        {
                            self.screen = Screen::Editor;
                        }
                    });
                });
            });
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(bg).inner_margin(egui::Margin::symmetric(24, 0))).show(root, |ui| {
            egui::ScrollArea::vertical().id_salt("home_scroll").auto_shrink([false, false]).show(ui, |ui| {
                let full = ui.available_width();
                let col_w = full.min(1020.0);
                ui.add_space(30.0);
                ui.horizontal(|ui| {
                    ui.add_space(((full - col_w) * 0.5).max(0.0));
                    ui.vertical(|ui| {
                        ui.set_width(col_w);
                        if self.home_page == 1 {
                            self.learn_page(ui);
                            return;
                        }
                        ui.label(egui::RichText::new("What will you make today?").size(28.0).strong().color(Color32::from_gray(238)));
                        ui.add_space(2.0);
                        ui.label(egui::RichText::new("Seamless materials, patterns and filters from nodes - every result tiles.").size(13.5).color(dim));
                        ui.add_space(22.0);

                        // ---- action cards
                        let actions: [(&str, &str, Ic); 6] = [
                            ("New graph", "Start from a template", Ic::New),
                            ("Open package", "Load a .mcg or .mca file", Ic::Open),
                            ("Brick wall example", "Explore a finished material", Ic::Layout),
                            ("Mesh map baker", "Bake maps from OBJ meshes", Ic::Sphere),
                            ("Open in Paint mode", "Paint a .mcpaint document or a mesh", Ic::Package),
                            ("Import Painter project", "Convert a Substance 3D Painter .spp", Ic::Input),
                        ];
                        let gap = 14.0;
                        let per_row = (((col_w + gap) / (230.0 + gap)).floor() as usize).clamp(1, 4);
                        let cw = (col_w - gap * (per_row as f32 - 1.0)) / per_row as f32;
                        let mut chosen: Option<usize> = None;
                        for row in actions.chunks(per_row).enumerate() {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = gap;
                                for (k, (title, sub, ic)) in row.1.iter().enumerate() {
                                    let (rect, resp) = ui.allocate_exact_size(vec2(cw, 84.0), Sense::click());
                                    let hot = resp.hovered();
                                    let pr = ui.painter();
                                    pr.rect_filled(rect, 10.0, if hot { card_hot } else { card });
                                    pr.rect_stroke(rect, 10.0, Stroke::new(if hot { 1.5_f32 } else { 1.0_f32 }, if hot { ACCENT } else { Color32::from_rgb(48, 52, 59) }), egui::StrokeKind::Inside);
                                    let badge = Rect::from_min_size(rect.left_top() + vec2(16.0, 18.0), vec2(48.0, 48.0));
                                    pr.rect_filled(badge, 12.0, ACCENT.gamma_multiply(if hot { 0.34 } else { 0.2 }));
                                    draw_icon(pr, badge.center(), *ic, ACCENT);
                                    pr.text(rect.left_top() + vec2(78.0, 24.0), Align2::LEFT_TOP, *title, FontId::proportional(14.5), Color32::from_gray(238));
                                    pr.text(rect.left_top() + vec2(78.0, 46.0), Align2::LEFT_TOP, *sub, FontId::proportional(11.5), dim);
                                    if resp.clicked() {
                                        chosen = Some(row.0 * per_row + k);
                                    }
                                }
                            });
                            ui.add_space(gap);
                        }
                        match chosen {
                            Some(0) => self.new_dlg.open = true,
                            Some(1) => self.open_file(),
                            Some(2) => self.open_example(),
                            Some(3) => {
                                self.screen = Screen::Editor;
                                self.bake.open = true;
                            }
                            Some(4) if self.paint.pick(gui_paint::Pick::Any) => self.screen = Screen::Paint,
                            Some(5) if self.paint.pick(gui_paint::Pick::Substance) => self.screen = Screen::Paint,
                            _ => {}
                        }

                        // ---- tip strip
                        if !self.hide_suggestions {
                            ui.add_space(4.0);
                            egui::Frame::NONE.fill(Color32::from_rgb(27, 36, 38)).stroke(Stroke::new(1.0_f32, Color32::from_rgb(40, 66, 66))).corner_radius(8.0).inner_margin(egui::Margin::symmetric(14, 9)).show(ui, |ui| {
                                ui.set_width(col_w - 30.0);
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new("Tip").strong().color(ACCENT));
                                    ui.label(egui::RichText::new("Press Tab in the graph to add a node at the cursor, or drag a link into empty space to add one and connect it.").color(Color32::from_gray(200)));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.add(egui::Button::new("Hide").frame(false)).clicked() {
                                            self.hide_suggestions = true;
                                            self.save_settings();
                                        }
                                    });
                                });
                            });
                        } else if ui.add(egui::Button::new(egui::RichText::new("Show tip").color(dim)).frame(false)).clicked() {
                            self.hide_suggestions = false;
                            self.save_settings();
                        }
                        ui.add_space(20.0);

                        // ---- recent files
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Recent").size(18.0).strong().color(Color32::from_gray(235)));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.add(egui::TextEdit::singleline(&mut self.home_filter).hint_text("Search recent files").desired_width(230.0));
                            });
                        });
                        ui.add_space(10.0);
                        let f = self.home_filter.to_lowercase();
                        let mut open: Option<PathBuf> = None;
                        let mut remove: Option<PathBuf> = None;
                        let files: Vec<PathBuf> = self.recent.iter().filter(|p| f.is_empty() || p.file_name().map(|s| s.to_string_lossy().to_lowercase().contains(&f)).unwrap_or(false)).cloned().collect();
                        if files.is_empty() {
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new(if self.recent.is_empty() { "Nothing here yet - create a graph or open a package and it will show up here." } else { "No recent files match the search." }).color(dim));
                        }
                        let rw = 330.0;
                        let rcols = (((col_w + gap) / (rw + gap)).floor() as usize).clamp(1, 6);
                        let rcw = (col_w - gap * (rcols as f32 - 1.0)) / rcols as f32;
                        for chunk in files.chunks(rcols) {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = gap;
                                for p in chunk {
                                    let name = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                                    let folder = p.parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_default();
                                    let meta = std::fs::metadata(p).ok();
                                    let (rect, resp) = ui.allocate_exact_size(vec2(rcw, 76.0), Sense::click());
                                    let hot = resp.hovered();
                                    let pr = ui.painter();
                                    pr.rect_filled(rect, 10.0, if hot { card_hot } else { card });
                                    pr.rect_stroke(rect, 10.0, Stroke::new(1.0_f32, if hot && meta.is_some() { ACCENT } else { Color32::from_rgb(44, 48, 55) }), egui::StrokeKind::Inside);
                                    let sw = Rect::from_min_size(rect.left_top() + vec2(10.0, 10.0), vec2(56.0, 56.0));
                                    swatch(&pr.with_clip_rect(sw), sw, &name, meta.is_some());
                                    let col = if meta.is_some() { Color32::from_gray(228) } else { Color32::from_gray(110) };
                                    let tx = rect.min.x + 78.0;
                                    let clipped = pr.with_clip_rect(Rect::from_min_max(pos2(tx, rect.min.y), pos2(rect.max.x - 8.0, rect.max.y)));
                                    clipped.text(pos2(tx, rect.min.y + 12.0), Align2::LEFT_TOP, &name, FontId::proportional(13.5), col);
                                    clipped.text(pos2(tx, rect.min.y + 32.0), Align2::LEFT_TOP, &folder, FontId::proportional(10.5), Color32::from_gray(110));
                                    let line = match &meta {
                                        Some(m) => format!("{}  -  {}", m.modified().map(fmt_time).unwrap_or_default(), fmt_size(m.len())),
                                        None => "file not found".to_string(),
                                    };
                                    clipped.text(pos2(tx, rect.min.y + 50.0), Align2::LEFT_TOP, line, FontId::proportional(10.5), dim);
                                    let resp = resp.on_hover_text(p.display().to_string());
                                    if resp.clicked() && meta.is_some() {
                                        open = Some(p.clone());
                                    }
                                    resp.context_menu(|ui| {
                                        if ui.button("Remove from recent").clicked() {
                                            remove = Some(p.clone());
                                            ui.close();
                                        }
                                    });
                                }
                            });
                            ui.add_space(gap);
                        }
                        if let Some(p) = remove {
                            self.recent.retain(|r| *r != p);
                            let files: Vec<String> = self.recent.iter().map(|r| r.to_string_lossy().to_string()).collect();
                            self.prefs.update(|v| v.recent.files = files);
                        }
                        if let Some(p) = open {
                            self.open_path(p);
                        }
                    });
                });
                ui.add_space(24.0);
            });
        });
    }

    fn learn_page(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Learn").size(19.0).strong().color(Color32::from_gray(235)));
        ui.add_space(6.0);
        ui.separator();
        ui.add_space(14.0);
        ui.label(egui::RichText::new("Getting started").size(15.0).strong());
        for t in [
            "Create a graph from the 'New graph...' button and pick a template (Metallic Roughness gives you all PBR outputs).",
            "Press Tab over the graph to add a node at the cursor, or drag a link into empty space to add one and connect it.",
            "Drag nodes from the Library onto the graph. Right-click a library item to favorite it.",
            "Select nodes and use Edit > Create subgraph to build reusable pieces. Double-click a subgraph to edit it.",
            "Click the 'P' button next to a parameter to publish it, then use File > Publish package to produce a .mca asset.",
            "Tools > Mesh map baker bakes normal, AO, curvature, thickness, position and ID maps from OBJ meshes.",
        ] {
            ui.label(format!("  -  {t}"));
            ui.add_space(3.0);
        }
        ui.add_space(14.0);
        if ui.button("Open the brick wall example").clicked() {
            self.open_example();
        }
        ui.add_space(8.0);
        if ui.button("Show keyboard shortcuts").clicked() {
            self.show_about = true;
        }
    }

    // ------------------------------------------------------------ new graph dialog
    fn new_dialog(&mut self, ctx: &egui::Context) {
        if !self.new_dlg.open {
            return;
        }
        let mut open = true;
        let mut create = false;
        let mut cancel = false;
        egui::Window::new(if self.new_dlg.dest.is_some() { "New Material Graph" } else { "New Package" })
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .fixed_size(vec2(780.0, 470.0))
            .show(ctx, |ui| {
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(vec2(340.0, 400.0), egui::Layout::top_down(egui::Align::LEFT), |ui| {
                        ui.label(egui::RichText::new("TEMPLATES").size(12.0).color(Color32::from_gray(200)));
                        ui.add_space(4.0);
                        egui::Frame::NONE.fill(Color32::from_rgb(32, 32, 32)).stroke(Stroke::new(1.0_f32, Color32::from_rgb(60, 60, 60))).show(ui, |ui| {
                            ui.set_min_size(vec2(334.0, 366.0));
                            egui::ScrollArea::vertical().id_salt("tmpl").max_height(366.0).show(ui, |ui| {
                                for (i, t) in TEMPLATES.iter().enumerate() {
                                    let sel = self.new_dlg.sel == i;
                                    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 21.0), Sense::click());
                                    if sel {
                                        ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(40, 62, 100));
                                        ui.painter().rect_stroke(rect, 0.0, Stroke::new(1.0_f32, Color32::from_rgb(60, 120, 230)), egui::StrokeKind::Inside);
                                    } else if resp.hovered() {
                                        ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(44, 44, 44));
                                    }
                                    draw_icon(ui.painter(), rect.left_center() + vec2(14.0, 0.0), Ic::Layout, Color32::from_gray(190));
                                    ui.painter().text(
                                        rect.left_center() + vec2(30.0, 0.0),
                                        Align2::LEFT_CENTER,
                                        t.name,
                                        FontId::proportional(12.5),
                                        Color32::from_gray(225),
                                    );
                                    if resp.clicked() {
                                        self.new_dlg.sel = i;
                                    }
                                    if resp.double_clicked() {
                                        self.new_dlg.sel = i;
                                    }
                                }
                            });
                        });
                    });
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.set_width(380.0);
                        ui.label(egui::RichText::new("PROPERTIES").size(12.0).color(Color32::from_gray(200)));
                        ui.add_space(8.0);
                        ui.strong("Graph Name:");
                        ui.add(egui::TextEdit::singleline(&mut self.new_dlg.name).desired_width(f32::INFINITY));
                        ui.add_space(8.0);
                        ui.strong("Parent size (initial working resolution)");
                        ui.horizontal(|ui| {
                            egui::ComboBox::from_id_salt("nd_w").width(150.0).selected_text(format!("{} px", self.new_dlg.size)).show_ui(ui, |ui| {
                                for s in [128usize, 256, 512, 1024, 2048, 4096, 8192] {
                                    ui.selectable_value(&mut self.new_dlg.size, s, format!("{s} px"));
                                }
                            });
                            let linked = self.new_dlg.height == 0;
                            let (r, resp) = ui.allocate_exact_size(vec2(26.0, 24.0), Sense::click());
                            ui.painter().rect_filled(r, 3.0, Color32::from_rgb(60, 60, 60));
                            draw_icon(ui.painter(), r.center(), Ic::Link, if linked { Color32::WHITE } else { Color32::from_gray(110) });
                            if resp
                                .on_hover_text(if linked {
                                    "Linked: the graph is square (click to set a separate height)"
                                } else {
                                    "Unlinked: click to make the graph square again"
                                })
                                .clicked()
                            {
                                self.new_dlg.height = if linked { self.new_dlg.size } else { 0 };
                            }
                            let shown = if linked { self.new_dlg.size } else { self.new_dlg.height };
                            ui.add_enabled_ui(!linked, |ui| {
                                egui::ComboBox::from_id_salt("nd_h").width(150.0).selected_text(format!("{shown} px")).show_ui(ui, |ui| {
                                    for s in [128usize, 256, 512, 1024, 2048, 4096, 8192] {
                                        ui.selectable_value(&mut self.new_dlg.height, s, format!("{s} px"));
                                    }
                                });
                            });
                        });
                        ui.add_space(8.0);
                        ui.strong("Output format");
                        egui::ComboBox::from_id_salt("nd_fmt")
                            .width(380.0)
                            .selected_text(if self.new_dlg.depth16 { "16 bits per channel" } else { "8 bits per channel" })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.new_dlg.depth16, false, "8 bits per channel");
                                ui.selectable_value(&mut self.new_dlg.depth16, true, "16 bits per channel");
                            });
                        ui.add_space(8.0);
                        ui.strong("Create graph in package");
                        let names: Vec<String> =
                            if self.screen == Screen::Editor { (0..self.pkgs.len()).map(|i| self.pkg_title(i)).collect() } else { Vec::new() };
                        let shown = match self.new_dlg.dest {
                            Some(i) => names.get(i).cloned().unwrap_or_else(|| "New package".into()),
                            None => "New package".into(),
                        };
                        egui::ComboBox::from_id_salt("nd_pkg").width(380.0).selected_text(shown).show_ui(ui, |ui| {
                            for (i, n) in names.iter().enumerate() {
                                if ui.selectable_label(self.new_dlg.dest == Some(i), n).clicked() {
                                    self.new_dlg.dest = Some(i);
                                }
                            }
                            if ui.selectable_label(self.new_dlg.dest.is_none(), "New package").clicked() {
                                self.new_dlg.dest = None;
                            }
                        });
                        ui.add_space(12.0);
                        egui::CollapsingHeader::new(egui::RichText::new("Template details").strong()).default_open(true).show(ui, |ui| {
                            let t = &TEMPLATES[self.new_dlg.sel.min(TEMPLATES.len() - 1)];
                            ui.label(t.desc);
                            if !t.outs.is_empty() {
                                ui.add_space(4.0);
                                ui.small(format!("Outputs: {}", t.outs.iter().map(|o| o.0).collect::<Vec<_>>().join(", ")));
                            }
                        });
                    });
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("RESET").clicked() {
                        self.new_dlg.name = "Graph".into();
                        self.new_dlg.size = 2048;
                        self.new_dlg.sel = 1;
                        self.new_dlg.depth16 = false;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::Button::new("CANCEL").min_size(vec2(90.0, 24.0))).clicked() {
                            cancel = true;
                        }
                        if ui
                            .add(egui::Button::new(egui::RichText::new("OK").strong()).fill(Color32::from_rgb(20, 90, 190)).min_size(vec2(90.0, 24.0)))
                            .clicked()
                        {
                            create = true;
                        }
                    });
                });
            });
        if create {
            self.create_from_dialog();
        }
        if cancel || !open {
            self.new_dlg.open = false;
        }
    }

    /// Ask before closing a package that has unsaved changes.
    fn confirm_close_ui(&mut self, ctx: &egui::Context) {
        let Some(i) = self.confirm_close else { return };
        if i >= self.pkgs.len() {
            self.confirm_close = None;
            return;
        }
        let name = self.pkg_title(i);
        let (mut save, mut discard, mut cancel) = (false, false, false);
        egui::Window::new("Close package").collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
            ui.label(format!("'{name}' has unsaved changes."));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                save = ui.button("Save").clicked();
                discard = ui.button("Discard changes").clicked();
                cancel = ui.button("Cancel").clicked();
            });
        });
        if save {
            self.confirm_close = None;
            self.switch_pkg(i);
            self.save();
            if !self.modified {
                let i = self.pkg_active;
                self.close_pkg(i);
            }
        } else if discard {
            self.confirm_close = None;
            self.close_pkg(i);
        } else if cancel {
            self.confirm_close = None;
        }
    }

    // ------------------------------------------------------------ explorer
    fn explorer_ui(&mut self, ui: &mut egui::Ui) {
        self.ws_init();
        if panel_header(ui, "EXPLORER", true) {
            self.show[0] = false;
        }
        ui.horizontal(|ui| {
            if icon_btn(ui, Ic::Save, "Save package", true).clicked() {
                self.acts.push(Act::Save);
            }
            if icon_btn(ui, Ic::Open, "Open package(s)", true).clicked() {
                self.acts.push(Act::Open);
            }
        });
        // right-click on empty space: package actions
        let area = ui.available_rect_before_wrap();
        let bg = ui.interact(area, Id::new("explorer_bg"), Sense::click());
        bg.context_menu(|ui| {
            if ui.button("New Package").clicked() {
                self.acts.push(Act::NewPackage);
                ui.close();
            }
            if ui.button("New Material Graph").clicked() {
                self.acts.push(Act::NewGraphIn(self.pkg_sel));
                ui.close();
            }
            ui.separator();
            if ui.button("Open Package(s)...").clicked() {
                self.acts.push(Act::Open);
                ui.close();
            }
        });
        let current = self.parents.last().map(|(_, n)| n.clone());
        let n_pkgs = self.pkgs.len();
        egui::ScrollArea::vertical().id_salt("explorer").auto_shrink([false, false]).show(ui, |ui| {
            for i in 0..n_pkgs {
                let is_active = i == self.pkg_active;
                let label = format!("{}{}", self.pkg_title(i), if self.pkg_modified(i) { "*" } else { "" });
                // graphs of this package: (name, outputs, is the package's main graph)
                let (main_name, graphs): (String, Vec<(String, Vec<String>, bool)>) = {
                    let root: &Graph = match self.pkgs.get(i) {
                        Some(Some(p)) => &p.g,
                        _ => self.parents.first().map(|p| &p.0).unwrap_or(&self.g),
                    };
                    let main = root.meta.get("identifier").cloned().filter(|s| !s.is_empty()).unwrap_or_else(|| "Graph".into());
                    let mut v = vec![(main.clone(), root.output_idents(), true)];
                    v.extend(root.subgraphs.iter().filter(|(_, s)| !s.meta.contains_key("embedded")).map(|(k, s)| (k.clone(), s.output_idents(), false)));
                    (main, v)
                };
                let _ = main_name;
                let id = ui.make_persistent_id(("pkg", i));
                let selected = self.pkg_sel == i;
                egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true)
                    .show_header(ui, |ui| {
                        explorer_icon(ui, Ic::Package);
                        let mut txt = egui::RichText::new(&label).italics().strong();
                        if selected {
                            txt = txt.color(Color32::WHITE);
                        }
                        let r = ui.add(egui::Label::new(txt).selectable(false).sense(Sense::click()));
                        if r.clicked() {
                            self.pkg_sel = i;
                        }
                        r.on_hover_text(match self.pkgs.get(i) {
                            Some(Some(p)) => p.path.as_ref().map(|x| x.display().to_string()).unwrap_or_else(|| "Not saved yet".into()),
                            _ => self.file.as_ref().map(|x| x.display().to_string()).unwrap_or_else(|| "Not saved yet".into()),
                        })
                        .context_menu(|ui| {
                            self.pkg_sel = i;
                            ui.menu_button("New", |ui| {
                                if ui.button("Material Graph").clicked() {
                                    self.acts.push(Act::NewGraphIn(i));
                                    ui.close();
                                }
                            });
                            if ui.button("Save Package").clicked() {
                                self.acts.push(Act::SavePkg(i));
                                ui.close();
                            }
                            if ui.button("Close Package").clicked() {
                                self.acts.push(Act::ClosePkg(i));
                                ui.close();
                            }
                        });
                    })
                    .body(|ui| {
                        for (name, outs, is_main) in &graphs {
                            let on = is_active && if *is_main { current.is_none() } else { current.as_deref() == Some(name.as_str()) };
                            let r = graph_row(ui, name, outs, on, ("pkg", format!("{i}/{is_main}/{name}")));
                            let sub = if *is_main { None } else { Some(name.clone()) };
                            if r.dragged() {
                                if is_active {
                                    if let Some(s) = &sub {
                                        r.dnd_set_drag_payload(format!("subgraph: {s}"));
                                    }
                                } else {
                                    r.dnd_set_drag_payload(GraphDrag(i, name.clone(), sub.clone()));
                                }
                            }
                            if r.clicked() {
                                self.pkg_sel = i;
                                if !is_active {
                                    self.acts.push(Act::SwitchPkg(i));
                                } else {
                                    for _ in 0..self.parents.len() {
                                        self.acts.push(Act::ExitSub);
                                    }
                                }
                                if let Some(s) = sub.clone() {
                                    self.acts.push(Act::EnterSub(s));
                                }
                            }
                            r.on_hover_text("Click to open - drag onto the canvas to use it as a node");
                        }
                        if is_active {
                            let root = self.parents.first().map(|p| &p.0).unwrap_or(&self.g);
                            let files: BTreeSet<String> = root
                                .nodes
                                .values()
                                .filter(|n| n.kind == "bitmap")
                                .filter_map(|n| n.strs.get("file"))
                                .map(|f| std::path::Path::new(f).file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| f.clone()))
                                .collect();
                            if !files.is_empty() {
                                ui.add_space(4.0);
                                ui.small("Resources");
                                for f in files {
                                    ui.label(format!("      {f}"));
                                }
                            }
                        }
                    });
            }
        });
    }

    // ------------------------------------------------------------ library
    fn lib_items(&self, search: &str) -> Vec<Entry> {
        let all = self.entries();
        let s = search.to_lowercase();
        if !s.is_empty() {
            return all.into_iter().filter(|e| e.label.to_lowercase().contains(&s) || e.doc.to_lowercase().contains(&s)).collect();
        }
        let cat = self.lib_cat.clone();
        all.into_iter()
            .filter(|e| {
                let (m, sub) = lib_group(&e.kind);
                match cat.as_str() {
                    "Favorites" => self.favorites.contains(&e.label),
                    "Core nodes" => !matches!(e.kind.as_str(), "subgraph" | "comment" | "frame"),
                    c => match c.split_once('/') {
                        Some((cm, cs)) => m == cm && sub == cs,
                        None => m == c,
                    },
                }
            })
            .collect()
    }

    fn library_ui(&mut self, ui: &mut egui::Ui, search: &mut String) {
        if panel_header(ui, "LIBRARY", true) {
            self.show[1] = false;
        }
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.add(egui::TextEdit::singleline(search).hint_text("Search").desired_width(170.0));
            if !search.is_empty() && ui.small_button("x").clicked() {
                search.clear();
            }
        });
        let (h, w) = (ui.available_height(), ui.available_width());
        let items = self.lib_items(search);
        let mut pick: Option<Entry> = None;
        let mut fav_toggle: Option<String> = None;
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(vec2(158.0, h), egui::Layout::top_down(egui::Align::LEFT), |ui| {
                egui::ScrollArea::vertical().id_salt("libtree").auto_shrink([false, false]).show(ui, |ui| {
                    for (name, subs) in LIB_TREE {
                        let sel = self.lib_cat == *name;
                        let icon = if *name == "Favorites" { "*  " } else { "" };
                        if subs.is_empty() {
                            if ui.selectable_label(sel, format!("{icon}{name}")).clicked() {
                                self.lib_cat = name.to_string();
                            }
                        } else {
                            let open = self.lib_cat.starts_with(name);
                            egui::CollapsingHeader::new(egui::RichText::new(*name).strong())
                                .default_open(true)
                                .open(if open { Some(true) } else { None })
                                .show(ui, |ui| {
                                    for sub in *subs {
                                        let full = format!("{name}/{sub}");
                                        if ui.selectable_label(self.lib_cat == full, format!("   {sub}")).clicked() {
                                            self.lib_cat = full;
                                        }
                                    }
                                });
                            let hdr_resp = ui.interact(ui.min_rect(), Id::new(("libcat", *name)), Sense::hover());
                            let _ = hdr_resp;
                            if sel {
                                // clicking the header text selects the whole group via the first sub-category
                            }
                        }
                    }
                });
            });
            ui.separator();
            ui.allocate_ui_with_layout(vec2((w - 168.0).max(80.0), h), egui::Layout::top_down(egui::Align::LEFT), |ui| {
                egui::ScrollArea::vertical().id_salt("libgrid").auto_shrink([false, false]).show(ui, |ui| {
                    if items.is_empty() {
                        ui.add_space(10.0);
                        ui.label(
                            egui::RichText::new(match self.lib_cat.as_str() {
                                "Pattern builders" | "Logic nodes" => "Not available in this version.",
                                "Favorites" => "Right-click a node to add it to favorites.",
                                _ => "Nothing here.",
                            })
                            .color(Color32::from_gray(130)),
                        );
                    }
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(4.0, 6.0);
                        for e in &items {
                            let (rect, resp) = ui.allocate_exact_size(vec2(68.0, 94.0), Sense::click_and_drag());
                            let p = ui.painter();
                            if resp.hovered() {
                                p.rect_filled(rect, 3.0, Color32::from_rgb(58, 58, 58));
                            }
                            let tr = Rect::from_min_size(rect.min + vec2(2.0, 2.0), vec2(64.0, 64.0));
                            match self.lib_thumbs.get(&e.kind) {
                                Some(t) if e.kind != "subgraph" => {
                                    p.image(t.id(), tr, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                                }
                                _ => {
                                    p.rect_filled(tr, 3.0, kind_color(&e.kind).gamma_multiply(0.55));
                                    draw_icon(p, tr.center(), entry_icon(&e.kind), Color32::WHITE);
                                }
                            }
                            if self.favorites.contains(&e.label) {
                                draw_icon(p, tr.right_top() + vec2(-8.0, 8.0), Ic::Star, Color32::from_rgb(90, 220, 190));
                            }
                            let (l1, l2) = two_lines(&e.label);
                            p.text(pos2(rect.center().x, rect.min.y + 70.0), Align2::CENTER_TOP, l1, FontId::proportional(11.0), Color32::from_gray(225));
                            if !l2.is_empty() {
                                p.text(pos2(rect.center().x, rect.min.y + 82.0), Align2::CENTER_TOP, l2, FontId::proportional(11.0), Color32::from_gray(225));
                            }
                            if resp.drag_started() {
                                resp.dnd_set_drag_payload(e.label.clone());
                            }
                            let resp = resp.on_hover_text(&e.doc);
                            if resp.clicked() {
                                pick = Some(e.clone());
                            }
                            resp.context_menu(|ui| {
                                let fav = self.favorites.contains(&e.label);
                                if ui.button(if fav { "Remove from favorites" } else { "Add to favorites" }).clicked() {
                                    fav_toggle = Some(e.label.clone());
                                    ui.close();
                                }
                            });
                        }
                    });
                });
            });
        });
        if let Some(e) = pick {
            let p = self.view_center_world();
            self.spawn(&e, p, None);
        }
        if let Some(l) = fav_toggle {
            if !self.favorites.remove(&l) {
                self.favorites.insert(l);
            }
            self.save_settings();
        }
    }

    // ------------------------------------------------------------ 2D view
    fn view2d_panel(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
        let title = match self.active.and_then(|a| self.g.nodes.get(&a).map(|n| (a, n.kind.replace('_', " ")))) {
            Some((id, k)) => format!("{k} #{id} - 2D VIEW"),
            None => "2D VIEW".into(),
        };
        if panel_header(ui, &title, true) {
            self.show[3] = false;
        }
        let bottom_h = 24.0;
        let avail = vec2(ui.available_width(), (ui.available_height() - bottom_h).max(40.0));
        let (rect, resp) = ui.allocate_exact_size(avail, Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::from_rgb(34, 34, 34));
        let fit = rect.width().min(rect.height()) - 20.0;
        if resp.hovered() {
            let sc = ui.input(|i| crate::gui_input::raw_scroll_delta(i).y);
            if sc != 0.0 {
                self.z2d = (self.z2d * (1.0 + sc * 0.0015)).clamp(0.1, 16.0);
            }
        }
        if resp.dragged() {
            self.pan2d += resp.drag_delta();
        }
        if resp.double_clicked() {
            self.z2d = 1.0;
            self.pan2d = Vec2::ZERO;
        }
        let side = fit * self.z2d;
        // the image keeps the texture's own aspect ratio; `side` is its longer edge
        let aspect = if self.tex2d_size.0 > 0 && self.tex2d_size.1 > 0 { self.tex2d_size.1 as f32 / self.tex2d_size.0 as f32 } else { 1.0 };
        let img_size = if aspect <= 1.0 { vec2(side, side * aspect) } else { vec2(side / aspect, side) };
        let img = Rect::from_center_size(rect.center() + self.pan2d, img_size);
        if let Some(t) = &self.tex2d {
            let uv = if self.tile_preview { Rect::from_min_max(pos2(-0.5, -0.5), pos2(1.5, 1.5)) } else { Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)) };
            if self.tile_preview {
                let big = Rect::from_center_size(img.center(), img.size() * 2.0);
                painter.image(t.id(), big, Rect::from_min_max(pos2(0.0, 0.0), pos2(2.0, 2.0)), Color32::WHITE);
            } else {
                painter.image(t.id(), img, uv, Color32::WHITE);
            }
            painter.rect_stroke(img, 0.0, Stroke::new(1.0_f32, Color32::from_gray(70)), egui::StrokeKind::Inside);
        } else {
            painter.text(rect.center(), Align2::CENTER_CENTER, "select a node to view its result", FontId::proportional(12.5), Color32::from_gray(120));
        }
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.small(format!(
                "{} x {} (RGBA, {}bpc)",
                self.tex2d_size.0,
                self.tex2d_size.1,
                if self.g.meta.get("output_format").map(|s| s == "16").unwrap_or(false) { 16 } else { 8 }
            ));
            if let Some((lo, hi)) = self.tex2d_range {
                if lo <= hi {
                    let hdr = hi > 1.0001 || lo < -0.0001;
                    let txt = egui::RichText::new(format!("range {lo:.3} .. {hi:.3}")).small().color(if hdr {
                        Color32::from_rgb(255, 190, 90)
                    } else {
                        Color32::from_gray(150)
                    });
                    ui.label(txt).on_hover_text(if hdr {
                        "Values outside 0..1 (HDR). The view clips them; exports keep them in float formats (EXR, HDR, 32-bit TIFF)."
                    } else {
                        "Smallest and largest channel value of this result"
                    });
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(6.0);
                let px = if self.tex2d_size.0 > 0 { img_size.x / self.tex2d_size.0 as f32 * 100.0 } else { 100.0 };
                ui.small(format!("{px:.2}%"));
                if icon_btn(ui, Ic::Frame, "Fit to view (double-click)", true).clicked() {
                    self.z2d = 1.0;
                    self.pan2d = Vec2::ZERO;
                }
                let before = self.tile_preview;
                if icon_btn(ui, Ic::Grid, "Tile 2x2 preview", true).clicked() {
                    self.tile_preview = !before;
                }
                let shown = if self.prev_size == 0 { format!("{}px (graph)", self.preview_full()) } else { format!("{}px", self.prev_size) };
                egui::ComboBox::from_id_salt("psize").width(110.0).selected_text(shown).show_ui(ui, |ui| {
                    let b = self.prev_size;
                    ui.selectable_value(&mut self.prev_size, 0, "Graph size");
                    for s in [128usize, 256, 512, 1024, 2048, 4096] {
                        ui.selectable_value(&mut self.prev_size, s, format!("{s}px"));
                    }
                    if b != self.prev_size {
                        self.dirty = true;
                    }
                });
            });
        });
    }

    // ------------------------------------------------------------ 3D view
    fn view3d_panel(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
        let shape = render3d::shape_name(self.view3d.shape);
        let engine = if self.gpu_ready() { "GPU" } else { "CPU" };
        if panel_header(ui, &format!("{shape} - {engine} - 3D VIEW"), true) {
            self.show[2] = false;
        }
        let mut ch = false;
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("Scene", |ui| {
                for (k, (n, id)) in render3d::SHAPES.iter().enumerate() {
                    if k == 7 {
                        ui.separator();
                    }
                    if ui.selectable_label(self.view3d.shape == *id, *n).clicked() {
                        self.view3d.shape = *id;
                        ch = true;
                        ui.close();
                    }
                }
                ui.separator();
                ch |= ui.add(egui::Slider::new(&mut self.view3d.tiling, 0.25..=16.0).text("UV tiling").logarithmic(true)).changed();
                ui.separator();
                ui.label("Mesh resolution (triangles per side)");
                let tess_list: &[u32] = if self.gpu_ready() { &[64, 128, 256, 384, 512, 768] } else { &[64, 128, 256] };
                for &t in tess_list {
                    if ui.selectable_label(self.view3d.tess == t, format!("{t} x {t}")).clicked() {
                        self.view3d.tess = t;
                        ch = true;
                    }
                }
                ch |= ui.checkbox(&mut self.view3d.wireframe, "Show wireframe").changed();
            });
            ui.menu_button("Materials", |ui| {
                if self.maps.contains_key("height") {
                    ui.label("Height display");
                    for (i, n) in ["Normal map only", "Bump", "Parallax (outward)", "Displacement (real geometry)"].iter().enumerate() {
                        if ui.selectable_label(self.view3d.relief == i as u32, *n).clicked() {
                            self.view3d.relief = i as u32;
                            ch = true;
                        }
                    }
                    ui.separator();
                    if self.view3d.relief >= 2 {
                        ch |= ui.add(egui::Slider::new(&mut self.view3d.height_scale, 0.0..=0.5).text("Height scale")).changed();
                    }
                    if self.view3d.relief == 1 {
                        ch |= ui.add(egui::Slider::new(&mut self.view3d.bump, 0.0..=12.0).text("Bump strength")).changed();
                    }
                    if self.view3d.relief >= 2 {
                        ch |= ui.add(egui::Slider::new(&mut self.view3d.height_level, 0.0..=1.0).text("Neutral level (0 = all outward)")).changed();
                    }
                } else {
                    ui.colored_label(Color32::from_rgb(255, 170, 120), "No Height output - add an Output node with usage Height.");
                }
                ch |= ui.add(egui::Slider::new(&mut self.view3d.normal_amt, 0.0..=1.0).text("Normal map strength")).changed();
                ui.separator();
                ui.small("Maps used by the viewer:");
                for k in self.maps.keys() {
                    ui.label(format!("  {k}"));
                }
            });
            ui.menu_button("Lights", |ui| {
                ch |= ui.checkbox(&mut self.view3d.light_follows_camera, "Light follows the camera").changed();
                ch |= ui.add(egui::Slider::new(&mut self.view3d.light_intensity, 0.0..=10.0).text("Intensity")).changed();
                ch |= ui.add(egui::Slider::new(&mut self.view3d.light_yaw, -3.14..=3.14).text("Azimuth")).changed();
                ch |= ui.add(egui::Slider::new(&mut self.view3d.light_pitch, -1.4..=1.4).text("Elevation")).changed();
            });
            ui.menu_button("Camera", |ui| {
                ch |= ui.add(egui::Slider::new(&mut self.view3d.dist, 1.6..=9.0).text("Distance")).changed();
                if ui.button("Reset camera").clicked() {
                    let d = View::default();
                    self.view3d.yaw = d.yaw;
                    self.view3d.pitch = d.pitch;
                    self.view3d.dist = d.dist;
                    self.view3d.pan_x = d.pan_x;
                    self.view3d.pan_y = d.pan_y;
                    self.view3d.light_yaw = d.light_yaw;
                    self.view3d.light_pitch = d.light_pitch;
                    ch = true;
                    ui.close();
                }
            });
            ui.menu_button("Environment", |ui| {
                ch |= ui.add(egui::Slider::new(&mut self.view3d.exposure, 0.1..=4.0).text("Exposure")).changed();
                ch |= ui.add(egui::Slider::new(&mut self.view3d.ambient, 0.0..=3.0).text("Ambient light")).changed();
            });
            ui.menu_button("Display", |ui| {
                ui.label(if self.gpu_ready() { "Tone mapping: ACES filmic" } else { "Tone mapping: filmic (Reinhard)" });
                ui.label("Color space: sRGB");
            });
            ui.menu_button("Renderer", |ui| {
                if ui.checkbox(&mut self.gpu_on, "GPU rendering (OpenGL)").changed() {
                    self.gpu_maps_dirty = true;
                    ch = true;
                }
                let (err, name) = self.gpu.lock().map(|g| (g.error.clone(), g.renderer.clone())).unwrap_or((None, String::new()));
                if let Some(e) = err {
                    ui.colored_label(Color32::from_rgb(255, 170, 120), format!("GPU renderer unavailable: {e}"));
                    ui.label("Using the CPU software renderer.");
                } else if self.gpu_on {
                    if !name.is_empty() {
                        ui.small(format!("GPU: {name}"));
                    }
                    ui.small("Real displacement, parallax relief, GGX PBR, mipmaps + anisotropic filtering, MSAA");
                    if let Ok(mut g) = self.gpu.lock() {
                        if ui.checkbox(&mut g.opts.self_shadow, "Height self-shadowing").changed() {
                            ch = true;
                        }
                    }
                }
                if !self.gpu_on {
                    ui.separator();
                    ui.label("CPU software PBR (GGX, normal mapping)");
                    let b = self.r3d_size;
                    for s in [256usize, 420, 640] {
                        ui.selectable_value(&mut self.r3d_size, s, format!("{s} px"));
                    }
                    if b != self.r3d_size {
                        ch = true;
                    }
                }
            });
        });
        if ch {
            self.dirty3d = true;
        }
        let bottom_h = 24.0;
        let avail = vec2(ui.available_width(), (ui.available_height() - bottom_h).max(40.0));
        let (rect, resp) = ui.allocate_exact_size(avail, Sense::click_and_drag());
        ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(30, 30, 30));
        let side = rect.width().min(rect.height());
        let img_rect = Rect::from_center_size(rect.center(), vec2(side, side));
        if resp.dragged() {
            let d = resp.drag_delta();
            let (alt, shift) = ui.input(|i| (i.modifiers.alt, i.modifiers.shift));
            if alt || resp.dragged_by(egui::PointerButton::Middle) {
                let k = 0.004 * self.view3d.dist / 3.4;
                self.view3d.pan_x -= d.x * k;
                self.view3d.pan_y += d.y * k;
                self.view3d.pan_x = self.view3d.pan_x.clamp(-3.0, 3.0);
                self.view3d.pan_y = self.view3d.pan_y.clamp(-3.0, 3.0);
            } else if shift {
                self.view3d.light_yaw -= d.x * 0.01;
                self.view3d.light_pitch = (self.view3d.light_pitch + d.y * 0.01).clamp(-1.4, 1.4);
            } else {
                self.view3d.yaw -= d.x * 0.01;
                self.view3d.pitch = (self.view3d.pitch + d.y * 0.01).clamp(-1.4, 1.4);
            }
            self.dirty3d = true;
        }
        if resp.hovered() {
            let sc = ui.input(|i| crate::gui_input::raw_scroll_delta(i).y);
            if sc != 0.0 {
                self.view3d.dist = (self.view3d.dist * (1.0 - sc * 0.001)).clamp(1.6, 9.0);
                self.dirty3d = true;
            }
        }
        if self.gpu_ready() {
            let (g, v, sink) = (self.gpu.clone(), self.view3d.clone(), self.diag.sink());
            ui.painter().add(egui::PaintCallback {
                rect,
                callback: std::sync::Arc::new(eframe::egui_glow::CallbackFn::new(move |info, painter| gpu3d::paint(&g, &info, painter, &v, &*sink))),
            });
        } else if let Some(t) = &self.tex3d {
            ui.painter().image(t.id(), img_rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        ui.painter().text(
            rect.left_bottom() + vec2(6.0, -5.0),
            Align2::LEFT_BOTTOM,
            "drag: orbit   Alt+drag / middle: move   Shift+drag: light   wheel: zoom",
            FontId::proportional(10.0),
            Color32::from_gray(110),
        );
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.small("sRGB (default)");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(6.0);
                let before = self.view3d.shape;
                egui::ComboBox::from_id_salt("shape3d").width(120.0).selected_text(shape).show_ui(ui, |ui| {
                    for (n, id) in render3d::SHAPES.iter() {
                        ui.selectable_value(&mut self.view3d.shape, *id, *n);
                    }
                });
                if before != self.view3d.shape {
                    self.dirty3d = true;
                }
            });
        });
    }

    // ------------------------------------------------------------ graph properties
    fn graph_props(&mut self, ui: &mut egui::Ui) {
        let mut meta = self.g.meta.clone();
        let mut size = self.g.size;
        let mut height = self.g.height;
        let mut seed = self.g.seed_offset;
        egui::CollapsingHeader::new(egui::RichText::new("GRAPH SETTINGS").strong()).default_open(true).show(ui, |ui| {
            ui.strong("Output Size");
            let mut e = (size as f32).log2().round() as i32;
            ui.horizontal(|ui| {
                ui.label("Width");
                ui.add(egui::Slider::new(&mut e, 4..=12).custom_formatter(|v, _| format!("{} px", 1u32 << (v as u32))).show_value(true));
            });
            let mut linked = height == 0;
            let mut eh = if linked { e } else { (height as f32).log2().round() as i32 };
            ui.horizontal(|ui| {
                ui.label("Height");
                ui.add_enabled(!linked, egui::Slider::new(&mut eh, 4..=12).custom_formatter(|v, _| format!("{} px", 1u32 << (v as u32))));
                let (r, resp) = ui.allocate_exact_size(vec2(22.0, 20.0), Sense::click());
                draw_icon(ui.painter(), r.center(), Ic::Link, if linked { Color32::WHITE } else { Color32::from_gray(110) });
                if resp
                    .on_hover_text(if linked { "Linked: height follows width (click to unlink)" } else { "Unlinked: click to make the graph square again" })
                    .clicked()
                {
                    linked = !linked;
                    if !linked {
                        eh = e;
                    }
                }
            });
            size = 1usize << e.clamp(4, 12);
            height = if linked { 0 } else { 1usize << eh.clamp(4, 12) };
            ui.add_space(6.0);
            ui.strong("Output Format");
            let cur = meta.get("output_format").cloned().unwrap_or_else(|| "8".into());
            egui::ComboBox::from_id_salt("ofmt").width(220.0).selected_text(if cur == "16" { "16 Bits per Channel" } else { "8 Bits per Channel" }).show_ui(
                ui,
                |ui| {
                    if ui.selectable_label(cur != "16", "8 Bits per Channel").clicked() {
                        meta.insert("output_format".into(), "8".into());
                    }
                    if ui.selectable_label(cur == "16", "16 Bits per Channel").clicked() {
                        meta.insert("output_format".into(), "16".into());
                    }
                },
            );
            ui.add_space(6.0);
            ui.strong("Node Precision");
            let prec = meta.get("ofmt").and_then(|s| s.parse::<usize>().ok()).unwrap_or(0).min(3);
            let pnames = ["32 bits float", "8 bits integer", "16 bits integer", "16 bits float"];
            egui::ComboBox::from_id_salt("nodeprec").width(220.0).selected_text(pnames[prec]).show_ui(ui, |ui| {
                for (i, n) in pnames.iter().enumerate() {
                    if ui
                        .selectable_label(prec == i, *n)
                        .on_hover_text("Every node is rounded to this precision unless it sets its own Output Format")
                        .clicked()
                    {
                        if i == 0 {
                            meta.remove("ofmt");
                        } else {
                            meta.insert("ofmt".into(), i.to_string());
                        }
                    }
                }
            });
            ui.add_space(6.0);
            ui.strong("Tiling Mode");
            ui.add_enabled(false, egui::Button::new("H and V Tiling").min_size(vec2(220.0, 20.0)));
            ui.add_space(6.0);
            ui.strong("Random Seed");
            ui.add(egui::Slider::new(&mut seed, 0..=9999));
        });
        egui::CollapsingHeader::new(egui::RichText::new("DETAILS").strong()).default_open(true).show(ui, |ui| {
            meta_field(ui, &mut meta, "Identifier", "identifier", false);
            meta_field(ui, &mut meta, "Label", "label", false);
            ui.strong("Type");
            let ty = meta.get("type").cloned().unwrap_or_else(|| "Standard material".into());
            egui::ComboBox::from_id_salt("gtype").width(f32::INFINITY.min(300.0)).selected_text(&ty).show_ui(ui, |ui| {
                for t in ["Standard material", "Filter", "Generator", "Other"] {
                    if ui.selectable_label(ty == t, t).clicked() {
                        meta.insert("type".into(), t.to_string());
                    }
                }
            });
            ui.add_space(4.0);
            ui.strong("Physical size (cm)");
            for (axis, key) in [("x", "phys_x"), ("y", "phys_y"), ("z", "phys_z")] {
                let mut v: f32 = meta.get(key).and_then(|s| s.parse().ok()).unwrap_or(0.0);
                ui.horizontal(|ui| {
                    ui.label(axis);
                    if ui.add(egui::Slider::new(&mut v, 0.0..=1000.0)).changed() {
                        meta.insert(key.to_string(), format!("{v}"));
                    }
                });
            }
            ui.add_space(4.0);
            ui.strong("Package");
            ui.add_enabled(
                false,
                egui::TextEdit::singleline(&mut self.file.as_ref().map(|p| p.display().to_string()).unwrap_or_default()).desired_width(f32::INFINITY),
            );
            ui.add_space(4.0);
            for (label, key) in [("Exposed in package", "exposed"), ("Show in library", "show_in_library")] {
                ui.strong(label);
                let on = meta.get(key).map(|s| s != "no").unwrap_or(true);
                if ui.add(egui::Button::new(if on { "Yes" } else { "No" }).min_size(vec2(ui.available_width(), 20.0))).clicked() {
                    meta.insert(key.to_string(), if on { "no" } else { "yes" }.to_string());
                }
                ui.add_space(4.0);
            }
            meta_field(ui, &mut meta, "Description", "description", true);
            meta_field(ui, &mut meta, "Category", "category", false);
            meta_field(ui, &mut meta, "Author", "author", false);
            meta_field(ui, &mut meta, "Author URL", "author_url", false);
            meta_field(ui, &mut meta, "Tags", "tags", false);
        });
        if meta != self.g.meta || size != self.g.size || height != self.g.height || seed != self.g.seed_offset {
            if !self.edit_open {
                self.snap();
                self.edit_open = true;
            }
            if size != self.g.size || height != self.g.height || seed != self.g.seed_offset {
                self.dirty = true;
            }
            self.g.meta = meta;
            self.g.size = size;
            self.g.height = height;
            self.g.seed_offset = seed;
        }
    }

    // ------------------------------------------------------------ editor layout
    fn editor_ui(&mut self, root: &mut egui::Ui) {
        let ctx = &root.ctx().clone();
        let mut acts: Vec<Act> = Vec::new();
        let in_sub = !self.parents.is_empty();
        let (can_undo, can_redo, has_sel, has_clip) = (!self.undo.is_empty(), !self.redo.is_empty(), !self.sel.is_empty(), !self.clip.is_empty());
        let panel_fill = Color32::from_rgb(40, 40, 40);

        egui::Panel::top("menu").frame(egui::Frame::NONE.fill(Color32::from_rgb(34, 34, 34)).inner_margin(egui::Margin::symmetric(6, 2))).show(root, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    ui.menu_button("New", |ui| {
                        if ui.button("Material Graph...   Ctrl+N").clicked() {
                            acts.push(Act::NewGraph);
                            ui.close();
                        }
                        if ui.button("Empty Package...").clicked() {
                            acts.push(Act::NewPackage);
                            ui.close();
                        }
                    });
                    for (label, a) in [("Open package(s)...   Ctrl+O", Act::Open), ("Save   Ctrl+S", Act::Save), ("Save as...", Act::SaveAs)] {
                        if ui.button(label).clicked() {
                            acts.push(a);
                            ui.close();
                        }
                    }
                    ui.separator();
                    for (label, a) in [
                        ("Link package as node...", Act::LinkPackage),
                        ("Refresh linked packages", Act::RefreshLinks),
                        ("Dependencies...", Act::ToggleDeps),
                        ("Import bitmap...", Act::ImportBitmap),
                        ("Export textures...", Act::Export),
                        ("Publish package (.mca)...", Act::Publish),
                    ] {
                        if ui.button(label).clicked() {
                            acts.push(a);
                            ui.close();
                        }
                    }
                    ui.separator();
                    if ui.button("Home screen").clicked() {
                        acts.push(Act::Home);
                        ui.close();
                    }
                });
                ui.menu_button("Edit", |ui| {
                    if ui.add_enabled(can_undo, egui::Button::new("Undo   Ctrl+Z")).clicked() {
                        acts.push(Act::Undo);
                        ui.close();
                    }
                    if ui.add_enabled(can_redo, egui::Button::new("Redo   Ctrl+Y")).clicked() {
                        acts.push(Act::Redo);
                        ui.close();
                    }
                    ui.separator();
                    for (label, a, en) in [
                        ("Cut   Ctrl+X", Act::Cut, has_sel),
                        ("Copy   Ctrl+C", Act::Copy, has_sel),
                        ("Paste   Ctrl+V", Act::Paste(None), has_clip),
                        ("Duplicate   Ctrl+D", Act::Duplicate, has_sel),
                        ("Delete   Del", Act::Delete, true),
                        ("Select all   Ctrl+A", Act::SelectAll, true),
                    ] {
                        if ui.add_enabled(en, egui::Button::new(label)).clicked() {
                            acts.push(a);
                            ui.close();
                        }
                    }
                    ui.separator();
                    if ui.button("Add node...   Tab").clicked() {
                        acts.push(Act::Search(None));
                        ui.close();
                    }
                    if ui.button("Comment frame   C").clicked() {
                        acts.push(Act::Comment);
                        ui.close();
                    }
                    if ui.add_enabled(has_sel, egui::Button::new("Create subgraph from selection")).clicked() {
                        acts.push(Act::Subgraph);
                        ui.close();
                    }
                });
                ui.menu_button("Tools", |ui| {
                    if ui.button("Mesh map baker...").clicked() {
                        acts.push(Act::Bake);
                        ui.close();
                    }
                    if ui.button("Frame all   F").clicked() {
                        acts.push(Act::FrameAll);
                        ui.close();
                    }
                    if ui.button("Auto-layout").clicked() {
                        acts.push(Act::AutoLayout);
                        ui.close();
                    }
                    ui.separator();
                    match materialcraft::gpu::adapter_name() {
                        Some(name) => {
                            if ui
                                .checkbox(&mut self.gpu_eval, "GPU acceleration")
                                .on_hover_text(format!("Large noises, blurs and normal maps run on {name}. Results match the CPU path within rounding."))
                                .changed()
                            {
                                materialcraft::gpu::set_enabled(self.gpu_eval);
                                self.dirty = true;
                            }
                        }
                        None => {
                            ui.add_enabled(false, egui::Button::new("GPU acceleration (no suitable adapter)"));
                        }
                    }
                });
                ui.menu_button("Windows", |ui| {
                    for (i, n) in ["Explorer", "Library", "3D View", "2D View", "Properties"].iter().enumerate() {
                        ui.checkbox(&mut self.show[i], *n);
                    }
                    ui.separator();
                    if ui.button("Reset layout").clicked() {
                        self.show = [true; 5];
                        self.dock = default_dock();
                        self.dock_closed.clear();
                        ui.close();
                    }
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("Keyboard shortcuts").clicked() {
                        self.show_about = true;
                        ui.close();
                    }
                    ui.separator();
                    ui.small(self.logo_note.as_str());
                });
            });
        });
        // one tab per open package; clicking switches the editor to it, the cross closes it
        if self.pkgs.len() > 0 {
            egui::Panel::top("tabs").frame(egui::Frame::NONE.fill(Color32::from_rgb(30, 30, 30)).inner_margin(egui::Margin::symmetric(4, 2))).show(
                root,
                |ui| {
                    egui::ScrollArea::horizontal().id_salt("tabs_scroll").auto_shrink([false, true]).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            for i in 0..self.pkgs.len() {
                                let active = i == self.pkg_active;
                                let title = format!("{}{}", self.pkg_title(i), if self.pkg_modified(i) { "*" } else { "" });
                                let fill = if active { Color32::from_rgb(58, 58, 58) } else { Color32::from_rgb(40, 40, 40) };
                                let txt = egui::RichText::new(title).color(if active { Color32::WHITE } else { Color32::from_gray(170) });
                                let r = ui.add(egui::Button::new(txt).fill(fill).min_size(vec2(90.0, 20.0)));
                                if r.clicked() && !active {
                                    acts.push(Act::SwitchPkg(i));
                                }
                                if r.middle_clicked() {
                                    acts.push(Act::ClosePkg(i));
                                }
                                if ui
                                    .add(egui::Button::new(egui::RichText::new("x").color(Color32::from_gray(150))).fill(fill).min_size(vec2(18.0, 20.0)))
                                    .on_hover_text("Close package")
                                    .clicked()
                                {
                                    acts.push(Act::ClosePkg(i));
                                }
                                ui.add_space(4.0);
                            }
                            if ui.add(egui::Button::new("+").min_size(vec2(22.0, 20.0))).on_hover_text("New empty package").clicked() {
                                acts.push(Act::NewPackage);
                            }
                        });
                    });
                    // one tab per open graph of the active package: the main graph first, then every graph opened from it
                    let root_names: BTreeSet<String> = self.parents.first().map(|p| &p.0).unwrap_or(&self.g).subgraphs.keys().cloned().collect();
                    let open: Vec<String> = self.graph_tabs.iter().filter(|n| root_names.contains(*n)).cloned().collect();
                    if !open.is_empty() {
                        let current = self.parents.last().map(|(_, n)| n.clone()).unwrap_or_default();
                        egui::ScrollArea::horizontal().id_salt("gtabs_scroll").auto_shrink([false, true]).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 2.0;
                                for name in std::iter::once(String::new()).chain(open.iter().cloned()) {
                                    let active = current == name;
                                    let fill = if active { Color32::from_rgb(50, 62, 60) } else { Color32::from_rgb(36, 36, 36) };
                                    let label = if name.is_empty() { "Main graph".to_string() } else { name.clone() };
                                    let txt = egui::RichText::new(label).small().color(if active { ACCENT } else { Color32::from_gray(160) });
                                    if ui.add(egui::Button::new(txt).fill(fill).min_size(vec2(70.0, 18.0))).clicked() && !active {
                                        acts.push(Act::OpenGraphTab(name.clone()));
                                    }
                                    if !name.is_empty()
                                        && ui
                                            .add(
                                                egui::Button::new(egui::RichText::new("x").small().color(Color32::from_gray(140)))
                                                    .fill(fill)
                                                    .min_size(vec2(16.0, 18.0)),
                                            )
                                            .on_hover_text("Close graph tab")
                                            .clicked()
                                    {
                                        acts.push(Act::CloseGraphTab(name.clone()));
                                    }
                                    ui.add_space(3.0);
                                }
                            });
                        });
                    }
                },
            );
        }
        egui::Panel::top("toolbar").frame(egui::Frame::NONE.fill(Color32::from_rgb(38, 38, 38)).inner_margin(egui::Margin::symmetric(6, 1))).show(root, |ui| {
            ui.horizontal(|ui| {
                for (ic, tip, a, en) in [
                    (Ic::New, "New material graph (Ctrl+N)", Act::NewGraph, true),
                    (Ic::Open, "Open (Ctrl+O)", Act::Open, true),
                    (Ic::Save, "Save (Ctrl+S)", Act::Save, true),
                ] {
                    if icon_btn(ui, ic, tip, en).clicked() {
                        acts.push(a);
                    }
                }
                ui.separator();
                if icon_btn(ui, Ic::Undo, "Undo (Ctrl+Z)", can_undo).clicked() {
                    acts.push(Act::Undo);
                }
                if icon_btn(ui, Ic::Redo, "Redo (Ctrl+Y)", can_redo).clicked() {
                    acts.push(Act::Redo);
                }
            });
        });
        egui::Panel::bottom("status").frame(egui::Frame::NONE.fill(Color32::from_rgb(30, 30, 30)).inner_margin(egui::Margin::symmetric(8, 2))).show(
            root,
            |ui| {
                ui.horizontal(|ui| {
                    let (msg, ok) = &self.status;
                    ui.colored_label(if *ok { Color32::from_gray(170) } else { Color32::from_rgb(255, 120, 110) }, msg);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
                        let engine = match materialcraft::gpu::adapter_name().filter(|_| self.gpu_eval) {
                            Some(a) => format!("CPU ({threads} threads) + GPU compute: {a}"),
                            None => format!("CPU ({threads} threads)"),
                        };
                        ui.label(format!(
                            "MaterialCraft Engine: {engine}{}    Version: {}",
                            if self.job.is_some() { "   computing..." } else { "" },
                            env!("CARGO_PKG_VERSION")
                        ));
                    });
                });
            },
        );

        // keep the panel layout and the Windows menu in step
        for pn in Pn::ALL {
            let Some(i) = pn.show_index() else { continue };
            let present = self.dock.contains(&pn);
            if !self.show[i] && present {
                if let Ok(loc) = self.dock.location(&pn) {
                    self.dock_closed.insert(pn, loc);
                }
                let _ = self.dock.apply(craft_ui::docking::Action::Close { panel: pn });
            } else if self.show[i] && !present {
                let restored = match self.dock_closed.get(&pn).cloned() {
                    Some(loc) => self.dock.restore(pn, &loc).is_ok(),
                    None => false,
                };
                if !restored {
                    let _ = self.dock.apply(craft_ui::docking::Action::OpenAt {
                        panel: pn,
                        anchor: Pn::Graph,
                        placement: craft_ui::docking::Placement::Split(craft_ui::docking::Zone::Right),
                    });
                }
            }
        }
        let mut layout = std::mem::take(&mut self.dock);
        let before = layout.clone();
        let mut dock_actions: Vec<craft_ui::docking::Action<Pn>> = Vec::new();
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(panel_fill)).show(root, |ui| {
            let mut style = craft_ui::docking::DockStyle::from_ui(ui);
            style.tab_height = 22.0;
            style.gap = 3.0;
            style.background = Color32::from_rgb(24, 24, 24);
            style.tab_background = Color32::from_rgb(30, 30, 30);
            style.active_background = Color32::from_rgb(52, 52, 52);
            style.accent = ACCENT;
            style.text = Color32::from_gray(230);
            style.inactive_text = Color32::from_gray(150);
            let out = craft_ui::docking::DockArea::new(Id::new("editor_dock")).show_with_limits(
                ui,
                &layout,
                &style,
                |p| p.label().to_string(),
                |p| if *p == Pn::Graph { craft_ui::docking::Permissions::PROTECTED } else { craft_ui::docking::Permissions::default() },
                |p| match p {
                    Pn::Graph => craft_ui::docking::PanelLimits { min: vec2(240.0, 160.0), ..Default::default() },
                    _ => craft_ui::docking::PanelLimits { min: vec2(120.0, 80.0), ..Default::default() },
                },
                |ui, p| match p {
                    Pn::Explorer => self.explorer_ui(ui),
                    Pn::Library => {
                        let mut s = std::mem::take(&mut self.lib_search);
                        self.library_ui(ui, &mut s);
                        self.lib_search = s;
                    }
                    Pn::View3d => self.view3d_panel(ui),
                    Pn::View2d => self.view2d_panel(ui),
                    Pn::Props => {
                        let title = match self.active.filter(|a| self.sel.contains(a)).and_then(|a| self.g.nodes.get(&a).map(|n| n.kind.replace('_', " "))) {
                            Some(k) => format!("{} - PROPERTIES", k.to_uppercase()),
                            None => format!("{} - PROPERTIES", self.gname()),
                        };
                        panel_header(ui, &title, false);
                        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                            ui.add_space(4.0);
                            egui::Frame::NONE.inner_margin(egui::Margin::symmetric(8, 0)).show(ui, |ui| self.inspector(ui));
                        });
                    }
                    Pn::Graph => {
                        panel_header(ui, &format!("{} - GRAPH", self.gname()), false);
                        ui.horizontal(|ui| {
                            ui.add_space(4.0);
                            for (ic, tip, a, en) in [
                                (Ic::Frame, "Frame all (F)", Act::FrameAll, true),
                                (Ic::Layout, "Auto-layout", Act::AutoLayout, true),
                                (Ic::Comment, "Comment frame (C)", Act::Comment, true),
                                (Ic::Subgraph, "Create subgraph from selection", Act::Subgraph, has_sel),
                            ] {
                                if icon_btn(ui, ic, tip, en).clicked() {
                                    acts.push(a);
                                }
                            }
                            ui.separator();
                            ui.label("Parent Size:");
                            let before = self.g.size;
                            egui::ComboBox::from_id_salt("tb_size").width(70.0).selected_text(format!("{}", self.g.size)).show_ui(ui, |ui| {
                                for s in [128usize, 256, 512, 1024, 2048, 4096, 8192] {
                                    ui.selectable_value(&mut self.g.size, s, format!("{s}"));
                                }
                            });
                            if before != self.g.size {
                                self.dirty = true;
                                self.modified = true;
                            }
                            let linked = self.g.height == 0;
                            let (r, resp) = ui.allocate_exact_size(vec2(24.0, 22.0), Sense::click());
                            ui.painter().rect_filled(r, 3.0, if linked { Color32::from_rgb(70, 70, 70) } else { Color32::from_rgb(50, 50, 50) });
                            draw_icon(ui.painter(), r.center(), Ic::Link, if linked { Color32::WHITE } else { Color32::from_gray(110) });
                            if resp
                                .on_hover_text(if linked {
                                    "Width and height are linked (click to unlink)"
                                } else {
                                    "Width and height are unlinked (click to link)"
                                })
                                .clicked()
                            {
                                // unlinking keeps the current square size; the height is then edited on its own
                                self.g.height = if linked { self.g.size } else { 0 };
                                self.dirty = true;
                                self.modified = true;
                            }
                            if linked {
                                ui.add_enabled(false, egui::Button::new(format!("{}", self.g.size)).min_size(vec2(70.0, 20.0)));
                            } else {
                                let hbefore = self.g.height;
                                egui::ComboBox::from_id_salt("tb_height").width(70.0).selected_text(format!("{}", self.g.height)).show_ui(ui, |ui| {
                                    for s in [128usize, 256, 512, 1024, 2048, 4096, 8192] {
                                        ui.selectable_value(&mut self.g.height, s, format!("{s}"));
                                    }
                                });
                                if hbefore != self.g.height {
                                    self.dirty = true;
                                    self.modified = true;
                                }
                            }
                            ui.separator();
                            if in_sub && ui.button("< Back").clicked() {
                                acts.push(Act::ExitSub);
                            }
                            let mut path = String::new();
                            for (_, n) in &self.parents {
                                path.push_str(&format!(" > {n}"));
                            }
                            if !path.is_empty() {
                                ui.strong(path);
                            }
                            if has_sel {
                                ui.small(format!("{} selected", self.sel.len()));
                            }
                        });
                        self.canvas(ui);
                        if self.need_fit {
                            self.need_fit = false;
                            self.fit_view();
                        }
                    }
                },
            );
            dock_actions = out.actions;
        });
        for a in dock_actions {
            if let craft_ui::docking::Action::Close { panel } = &a {
                if let Some(i) = panel.show_index() {
                    self.show[i] = false;
                }
                if let Ok(loc) = layout.location(panel) {
                    self.dock_closed.insert(*panel, loc);
                }
            }
            let _ = layout.apply_with_permissions(a, |p| {
                if *p == Pn::Graph { craft_ui::docking::Permissions::PROTECTED } else { craft_ui::docking::Permissions::default() }
            });
        }
        self.dock = layout;
        if self.dock != before {
            let l = self.dock.clone();
            self.prefs.update(|v| v.layout.dock = Some(l));
        }

        self.windows(ctx);
        self.search_ui(ctx);
        self.acts.extend(acts);

        if let Some(p) = egui::DragAndDrop::payload::<GraphDrag>(ctx) {
            if let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) {
                let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, Id::new("dnd_ghost_graph")));
                let text = format!("{} (graph)", p.1);
                let r = painter.text(pos + vec2(14.0, 14.0), Align2::LEFT_TOP, text.clone(), FontId::proportional(12.0), Color32::WHITE);
                painter.rect_filled(r.expand(4.0), 3.0, Color32::from_rgba_unmultiplied(20, 20, 20, 200));
                painter.text(pos + vec2(14.0, 14.0), Align2::LEFT_TOP, text, FontId::proportional(12.0), Color32::WHITE);
            }
        }
        if let Some(p) = egui::DragAndDrop::payload::<String>(ctx) {
            if let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) {
                let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, Id::new("dnd_ghost")));
                let r = painter.text(pos + vec2(14.0, 14.0), Align2::LEFT_TOP, (*p).clone(), FontId::proportional(12.0), Color32::WHITE);
                painter.rect_filled(r.expand(4.0), 3.0, Color32::from_rgba_unmultiplied(20, 20, 20, 200));
                painter.text(pos + vec2(14.0, 14.0), Align2::LEFT_TOP, (*p).clone(), FontId::proportional(12.0), Color32::WHITE);
            }
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = &root.ctx().clone();
        self.diag.drain();
        if self.screen == Screen::Paint {
            if self.paint.ui(root, self.diag.history()) == gui_paint::PaintAction::Home {
                self.screen = Screen::Home;
                self.last_title.clear();
            }
            return;
        }
        // coming back to the window: pick up edits made to linked package files in the meantime
        let focused = ctx.input(|i| i.focused);
        if focused && !self.was_focused && self.screen == Screen::Editor && self.parents.is_empty() && !self.g.links.is_empty() {
            self.acts.push(Act::RefreshLinks);
        }
        self.was_focused = focused;
        // graph files dropped from the OS onto the window: import as a node (editor) or open (home screen)
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        for p in dropped {
            if !matches!(p.extension().and_then(|e| e.to_str()), Some("mcg" | "mca")) {
                continue;
            }
            if self.screen == Screen::Editor {
                let at = ctx.input(|i| i.pointer.latest_pos()).filter(|q| self.canvas_rect.contains(*q));
                self.acts.push(Act::ImportGraph(p, at, None));
            } else {
                self.open_path(p);
            }
        }
        // background asset results
        if let Some(rx) = &self.banner_rx {
            if let Ok(img) = rx.try_recv() {
                let mut o = egui::TextureOptions::LINEAR;
                o.wrap_mode = egui::TextureWrapMode::Repeat;
                self.banner = Some(ctx.load_texture("banner", img, o));
                self.banner_rx = None;
            }
        }
        if let Some(rx) = &self.lib_rx {
            if let Ok(m) = rx.try_recv() {
                for (k, img) in m {
                    self.lib_thumbs.insert(k.clone(), ctx.load_texture(format!("lib_{k}"), img, egui::TextureOptions::LINEAR));
                }
                self.lib_rx = None;
            }
        }
        if self.banner_rx.is_some() || self.lib_rx.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(60));
        }

        let title = format!("MaterialCraft - {}{}", self.package_label(), if self.modified { "*" } else { "" });
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }

        if self.screen == Screen::Home {
            self.home_ui(root);
            self.new_dialog(ctx);
            self.windows(ctx);
            if self.screen == Screen::Editor {
                ctx.request_repaint();
            }
            self.text_focus = ctx.egui_wants_keyboard_input();
            return;
        }

        if !ctx.input(|i| i.pointer.any_down()) {
            self.edit_open = false;
        }
        self.poll_eval(ctx);
        if let Some(rx) = &self.bake.job {
            match rx.try_recv() {
                Ok(Ok(files)) => {
                    self.bake.job = None;
                    self.bake.msg = format!("Done: {} maps written. Added as bitmap nodes.", files.len());
                    let base = self.view_center_world();
                    for (i, f) in files.iter().enumerate() {
                        let id = self.add_node_at("bitmap", None, pos2(base.x + (i as f32 % 4.0) * (CARD + 20.0), base.y + (i / 4) as f32 * (CARD_H + 30.0)));
                        if let Some(n) = self.g.nodes.get_mut(&id) {
                            n.strs.insert("file".into(), f.to_string_lossy().to_string());
                        }
                    }
                }
                Ok(Err(e)) => {
                    self.bake.job = None;
                    self.bake.msg = format!("Bake failed: {e}");
                }
                Err(TryRecvError::Empty) => ctx.request_repaint_after(std::time::Duration::from_millis(100)),
                Err(TryRecvError::Disconnected) => {
                    self.bake.job = None;
                    self.bake.msg = "Bake failed unexpectedly.".into();
                }
            }
        }
        if self.dirty3d {
            self.render_3d(ctx);
        }

        // keyboard shortcuts
        let pointer = ctx.input(|i| i.pointer.hover_pos()).filter(|p| self.canvas_rect.contains(*p));
        if self.search.is_some() {
            ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Tab));
        } else if !self.text_focus && !self.new_dlg.open {
            let tab = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Tab));
            if tab {
                ctx.memory_mut(|m| {
                    if let Some(id) = m.focused() {
                        m.surrender_focus(id);
                    }
                });
                self.open_search(pointer.unwrap_or_else(|| self.canvas_rect.center()), None);
            }
        }
        if self.search.is_none() && !self.text_focus && !self.new_dlg.open {
            let k = ctx.input_mut(|i| {
                let redo = i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z) || i.consume_key(Modifiers::COMMAND, Key::Y);
                let undo = i.consume_key(Modifiers::COMMAND, Key::Z);
                let copy = i.events.iter().any(|e| matches!(e, egui::Event::Copy)) || i.consume_key(Modifiers::COMMAND, Key::C);
                let cut = i.events.iter().any(|e| matches!(e, egui::Event::Cut)) || i.consume_key(Modifiers::COMMAND, Key::X);
                let paste = i.consume_key(Modifiers::COMMAND, Key::V);
                let pasted: Option<String> = i.events.iter().find_map(|e| if let egui::Event::Paste(t) = e { Some(t.clone()) } else { None });
                let dup = i.consume_key(Modifiers::COMMAND, Key::D);
                let all = i.consume_key(Modifiers::COMMAND, Key::A);
                let save = i.consume_key(Modifiers::COMMAND, Key::S);
                let open = i.consume_key(Modifiers::COMMAND, Key::O);
                let newg = i.consume_key(Modifiers::COMMAND, Key::N);
                let del = i.consume_key(Modifiers::NONE, Key::Delete) || i.consume_key(Modifiers::NONE, Key::Backspace);
                let frame = i.consume_key(Modifiers::NONE, Key::F);
                let com = i.consume_key(Modifiers::NONE, Key::C);
                (redo, undo, copy, cut, paste, dup, all, save, open, del, frame, com, newg, pasted)
            });
            let w = pointer.map(|p| self.screen_to_world(p));
            if let Some(text) = &k.13 {
                if let Some(body) = text.strip_prefix("MaterialCraft nodes\n") {
                    if let Ok(g) = Graph::parse_unchecked(body) {
                        self.clip = g.nodes.into_iter().collect();
                        self.acts.push(Act::Paste(w));
                    }
                }
            }
            for (on, a) in [
                (k.0, Act::Redo),
                (k.1, Act::Undo),
                (k.2, Act::Copy),
                (k.3, Act::Cut),
                (k.4, Act::Paste(w)),
                (k.5, Act::Duplicate),
                (k.6, Act::SelectAll),
                (k.7, Act::Save),
                (k.8, Act::Open),
                (k.9, Act::Delete),
                (k.10, Act::FrameAll),
                (k.11, Act::Comment),
                (k.12, Act::NewGraph),
            ] {
                if on {
                    self.acts.push(a);
                }
            }
        }

        self.editor_ui(root);
        self.new_dialog(ctx);
        self.confirm_close_ui(ctx);
        if let Some(t) = self.copy_out.take() {
            ctx.copy_text(t);
        }

        let todo = std::mem::take(&mut self.acts);
        for a in todo {
            self.do_act(a);
        }
        if self.dirty {
            self.dirty = false;
            self.refine = false;
            let first = self.first_tier();
            self.spawn_eval(first);
        } else if self.refine && self.job.is_none() {
            self.refine = false;
            if let Some(sz) = self.refine_size {
                self.spawn_eval(sz);
            }
        }
        if self.job.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        }
        self.text_focus = ctx.egui_wants_keyboard_input();
        // save changed settings shortly after the last change
        let tick = self.prefs.tick(std::time::Instant::now());
        if let Some(deadline) = tick.next_deadline {
            ctx.request_repaint_after(deadline.saturating_duration_since(std::time::Instant::now()));
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.paint.flush();
        let _ = self.prefs.flush();
    }
}

static LOGO: std::sync::OnceLock<(Tex, String)> = std::sync::OnceLock::new();

fn logo() -> &'static (Tex, String) {
    LOGO.get_or_init(load_logo)
}

/// Embedded logo, else assets/MaterialCraftLogo.png next to the exe / working dir, else a built-in purple squid.
fn load_logo() -> (Tex, String) {
    let mut bytes: Option<(Vec<u8>, String)> = logo_data::LOGO_PNG.map(|b| (b.to_vec(), "embedded".to_string()));
    if bytes.is_none() {
        let mut dirs: Vec<PathBuf> = Vec::new();
        if let Ok(exe) = std::env::current_exe() {
            if let Some(d) = exe.parent() {
                dirs.push(d.join("assets"));
                dirs.push(d.to_path_buf());
            }
        }
        if let Ok(c) = std::env::current_dir() {
            dirs.push(c.join("assets"));
            dirs.push(c);
        }
        'outer: for d in dirs {
            if let Ok(rd) = std::fs::read_dir(&d) {
                for e in rd.flatten() {
                    let n = e.file_name().to_string_lossy().to_lowercase();
                    if n.starts_with("materialcraftlogo") && n.ends_with(".png") {
                        if let Ok(b) = std::fs::read(e.path()) {
                            bytes = Some((b, e.path().display().to_string()));
                            break 'outer;
                        }
                    }
                }
            }
        }
    }
    match bytes {
        Some((b, src)) => match png::decode(&b) {
            Ok(t) => (t, format!("Logo loaded from {src}")),
            Err(e) => (fallback_logo(128), format!("Logo file found but could not be read ({e}); using the built-in logo")),
        },
        None => (fallback_logo(128), "No MaterialCraftLogo.png found; using the built-in logo".into()),
    }
}

/// Built-in purple squid logo (used when no logo file is present).
fn fallback_logo(n: usize) -> Tex {
    let ell = |x: f32, y: f32, cx: f32, cy: f32, rx: f32, ry: f32| ((x - cx) / rx).powi(2) + ((y - cy) / ry).powi(2) <= 1.0;
    Tex::from_fn(n, n, |x, y| {
        // rounded square background
        let (qx, qy) = ((x - 0.5).abs() - 0.5 + 0.2, (y - 0.5).abs() - 0.5 + 0.2);
        let d = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - 0.2;
        if d > 0.0 {
            return [0.0, 0.0, 0.0, 0.0];
        }
        let t = 0.5 + 0.5 * (y - 0.5);
        let mut c = [0.32 - 0.1 * t, 0.15 - 0.05 * t, 0.62 - 0.12 * t, 1.0];
        let lilac = [0.92, 0.88, 1.0, 1.0];
        let fin = [0.78, 0.7, 0.96, 1.0];
        if ell(x, y, 0.37, 0.24, 0.07, 0.05) || ell(x, y, 0.63, 0.24, 0.07, 0.05) {
            c = fin;
        }
        let tri = y > 0.12 && y < 0.5 && (x - 0.5).abs() < (y - 0.12) * 0.42;
        if ell(x, y, 0.5, 0.4, 0.16, 0.24) || tri || ell(x, y, 0.5, 0.58, 0.16, 0.09) {
            c = lilac;
        }
        for i in 0..5 {
            let x0 = 0.36 + 0.07 * i as f32;
            let wx = x0 + 0.012 * (y * 38.0 + i as f32).sin();
            if y > 0.62 && y < 0.84 && (x - wx).abs() < 0.017 || ell(x, y, wx, 0.84, 0.021, 0.021) {
                c = lilac;
            }
        }
        for ex in [0.435, 0.565] {
            if ell(x, y, ex, 0.55, 0.036, 0.036) {
                c = [0.18, 0.08, 0.36, 1.0];
            }
            if ell(x, y, ex - 0.01, 0.54, 0.011, 0.011) {
                c = [1.0, 1.0, 1.0, 1.0];
            }
        }
        c
    })
}

fn window_icon() -> egui::IconData {
    let n = 128usize;
    let (t, _) = logo();
    let scale = (n as f32 / t.w as f32).min(n as f32 / t.h as f32);
    let (sw, sh) = (t.w as f32 * scale, t.h as f32 * scale);
    let mut rgba = Vec::with_capacity(n * n * 4);
    for y in 0..n {
        for x in 0..n {
            let (px, py) = (x as f32 + 0.5 - (n as f32 - sw) * 0.5, y as f32 + 0.5 - (n as f32 - sh) * 0.5);
            if px < 0.0 || py < 0.0 || px >= sw || py >= sh {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                let p = t.sample(px / sw, py / sh);
                rgba.extend_from_slice(&[
                    (p[0].clamp(0.0, 1.0) * 255.0) as u8,
                    (p[1].clamp(0.0, 1.0) * 255.0) as u8,
                    (p[2].clamp(0.0, 1.0) * 255.0) as u8,
                    (p[3].clamp(0.0, 1.0) * 255.0) as u8,
                ]);
            }
        }
    }
    egui::IconData { rgba, width: n as u32, height: n as u32 }
}

/// Diagnostics and user settings, set up before the window opens. The log file lives next to
/// the settings (`<settings dir>/logs`); with settings in memory only, there is no log file.
fn boot() -> (logging::Diagnostics, gui_settings::UserSettings) {
    use materialcraft::diag::{Area, SinkExt};
    let diag = logging::Diagnostics::start();
    let (settings, problems) = gui_settings::UserSettings::open(craft_settings::Location::Auto);
    if let Some(dir) = settings.dir()
        && let Err(e) = diag.attach_dir(&dir.join("logs"))
    {
        log::warn!("no log file: {e}");
    }
    for p in problems {
        diag.sink().warn(Area::General, p);
    }
    (diag, settings)
}

fn main() -> eframe::Result<()> {
    // 4x MSAA + depth buffer for the GPU viewport; fall back to no MSAA on drivers that cannot provide it.
    let make = |msaa: u16| eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1500.0, 920.0]).with_title("MaterialCraft").with_icon(window_icon()),
        multisampling: msaa,
        depth_buffer: 24,
        ..Default::default()
    };
    // Logging starts before the window, so start-up messages (GL context, portal) are kept.
    let state = std::cell::RefCell::new(Some(boot()));
    // The fallback run gets the same state if the first one never created the app.
    let take = || state.borrow_mut().take().unwrap_or_else(boot);
    match eframe::run_native("MaterialCraft", make(4), Box::new(|cc| Ok(Box::new(App::new(cc, take()))))) {
        Err(eframe::Error::NoGlutinConfigs(..)) => eframe::run_native("MaterialCraft", make(0), Box::new(|cc| Ok(Box::new(App::new(cc, take()))))),
        other => other,
    }
}

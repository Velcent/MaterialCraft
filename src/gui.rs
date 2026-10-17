#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
//! MaterialCraft visual editor. Run: cargo run --release --features gui --bin materialcraft-gui
use eframe::egui::{
    self, epaint::CubicBezierShape, Align2, Color32, ColorImage, FontId, Id, Key, Modifiers, PointerButton, Pos2, Rect,
    Rounding, Sense, Stroke, TextStyle, Vec2, pos2, vec2,
};
use materialcraft::bake::{self, BakeSettings};
use materialcraft::graph::{category, label, spec, usage_label, Comment, Exposed, Graph, Node, NodeCache, EXAMPLE, SPECS, THUMB};
use materialcraft::render3d::{self, Maps, View};
use materialcraft::tex::{self, Tex};
use materialcraft::export::{self, Format};
use materialcraft::gpu3d::{self, MapInputs};
use materialcraft::{ops, png};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, TryRecvError};

mod logo_data {
    include!(concat!(env!("OUT_DIR"), "/logo.rs"));
}
#[path = "gui_paint/mod.rs"]
mod gui_paint;
mod gui_theme;
use gui_theme::panel_header;

// ---------------------------------------------------------------- layout constants
const CARD: f32 = 92.0;
const BAR: f32 = 16.0;
const CARD_H: f32 = BAR + CARD;

/// Height of a node card's body: the square thumbnail, or taller when many input sockets (or outputs) need room.
fn card_body(n_slots: usize, n_outs: usize) -> f32 {
    CARD.max(15.0 * (n_slots as f32 + 1.0)).max(if n_outs >= 2 { 20.0 * n_outs as f32 } else { 0.0 })
}
const OUTPUT_NAMES: [&str; 11] = ["basecolor", "normal", "roughness", "metallic", "height", "ambient_occlusion", "opacity", "emissive", "diffuse", "specular", "glossiness"];
const COMMENT_COLORS: [[u8; 3]; 6] = [[90, 140, 200], [200, 160, 60], [90, 180, 110], [190, 90, 90], [150, 100, 200], [150, 150, 150]];

// ---------------------------------------------------------------- parameter tables
fn slots(kind: &str) -> Vec<(&'static str, bool)> {
    spec(kind)
        .map(|s| s.req.iter().map(|n| (*n, true)).chain(s.opt.iter().map(|n| (*n, false))).collect())
        .unwrap_or_default()
}

fn kind_color(kind: &str) -> Color32 {
    match kind {
        "subgraph" => Color32::from_rgb(214, 140, 56),
        "input" | "dot" => Color32::from_rgb(110, 116, 128),
        "bitmap" => Color32::from_rgb(188, 118, 70),
        "output" => Color32::from_rgb(74, 98, 120),
        _ => match category(kind) {
            "Generators" => Color32::from_rgb(176, 76, 66),
            "SDF" => Color32::from_rgb(140, 88, 184),
            "Material" => Color32::from_rgb(190, 150, 50),
            "Utility" => Color32::from_rgb(100, 110, 130),
            _ => Color32::from_rgb(64, 112, 176),
        },
    }
}

fn qimg(t: &Tex) -> ColorImage {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    let bytes: Vec<u8> = t.px.iter().flat_map(|p| [q(p[0]), q(p[1]), q(p[2]), q(p[3])]).collect();
    ColorImage::from_rgba_unmultiplied([t.w, t.h], &bytes)
}

// ---------------------------------------------------------------- types
#[derive(Clone)]
struct Entry {
    label: String,
    kind: String,
    sub: Option<String>,
    cat: &'static str,
    doc: String,
    nums: Vec<(&'static str, f32)>,
}

struct SearchState {
    screen: Pos2,
    world: Pos2,
    text: String,
    idx: usize,
    link_from: Option<u32>,
    link_to: Option<(u32, String)>,
    fresh: bool,
}

enum Act {
    Open,
    Save,
    SaveAs,
    Publish,
    ImportBitmap,
    Export,
    Bake,
    Undo,
    Redo,
    Copy,
    Cut,
    Paste(Option<Pos2>),
    Duplicate,
    Delete,
    SelectAll,
    Comment,
    Subgraph,
    EnterSub(String),
    ExitSub,
    AutoLayout,
    FrameAll,
    Search(Option<Pos2>),
    NewOutput(String, u32),
    NewGraph,
    Home,
    Drop(String, Pos2),
    ImportGraph(PathBuf, Option<Pos2>, Option<String>),
    /// Import a graph of an open package (index, graph name or None for its main graph) as a node.
    ImportFromPkg(usize, Option<String>, Option<Pos2>),
    NewPackage,
    NewGraphIn(usize),
    SwitchPkg(usize),
    SavePkg(usize),
    ClosePkg(usize),
}

/// Drag payload: a graph of an open package (package index, graph label, graph name or None for the main graph).
#[derive(Clone)]
struct GraphDrag(usize, String, Option<String>);

/// A package that is open in the workspace but not the one being edited right now.
struct Pkg {
    path: Option<PathBuf>,
    name: String,
    g: Graph,
    modified: bool,
}

fn read_pkg(path: &std::path::Path) -> Result<Pkg, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut g = Graph::parse_unchecked(&text).map_err(|e| e.to_string())?;
    g.migrate_outputs();
    auto_layout_if_unplaced(&mut g);
    if !g.meta.get("identifier").map(|s| !s.is_empty()).unwrap_or(false) {
        g.meta.insert("identifier".into(), path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "graph".into()));
    }
    Ok(Pkg { path: Some(path.to_path_buf()), name: path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(), g, modified: false })
}

struct EvalResult {
    r#gen: u64,
    thumbs: HashMap<u32, Vec<u8>>,
    errors: HashMap<u32, String>,
    kept: HashMap<u32, std::sync::Arc<Tex>>,
    outputs: Vec<(String, u32)>,
    sizes: HashMap<u32, usize>,
    /// the 2D view image and the GPU textures are converted on the worker thread so the editor never stalls on them
    img2d: Option<ColorImage>,
    img_for: Option<u32>,
    gpu: Option<gpu3d::GpuMaps>,
    /// resolution this pass was computed at
    size: usize,
}

/// The 2D view image of the selected node, sent as soon as that node is done (before the rest of the graph).
struct Partial {
    r#gen: u64,
    img2d: ColorImage,
    img_for: u32,
}

struct ExportUi {
    open: bool,
    dir: Option<PathBuf>,
    prefix: String,
    sizes: [bool; 6],
    depth16: bool,
    fmt: Format,
    quality: u8,
    skip: BTreeSet<String>,
}

struct BakeUi {
    open: bool,
    low: Option<PathBuf>,
    high: Option<PathBuf>,
    out: Option<PathBuf>,
    s: BakeSettings,
    job: Option<Receiver<Result<Vec<PathBuf>, String>>>,
    msg: String,
}

struct App {
    g: Graph,
    parents: Vec<(Graph, String)>,
    undo: Vec<Graph>,
    redo: Vec<Graph>,
    edit_open: bool,
    sel: BTreeSet<u32>,
    active: Option<u32>,
    sel_comment: Option<usize>,
    clip: Vec<(u32, Node)>,
    pan: Vec2,
    zoom: f32,
    canvas_rect: Rect,
    connecting: Option<u32>,
    /// Output name of the graph-node socket a link is being dragged from.
    connecting_out: Option<String>,
    /// Output name to apply when a node created from a link drag gets connected.
    link_out: Option<String>,
    rubber: Option<Pos2>,
    comment_members: Vec<u32>,
    search: Option<SearchState>,
    acts: Vec<Act>,
    status: (String, bool),
    file: Option<PathBuf>,
    /// editor preview resolution in pixels; 0 = the graph's own size (capped at 4096)
    prev_size: usize,
    /// a quick low-resolution result was just shown; compute the full-size one next
    refine: bool,
    /// resolution of the pass being computed, and the next (bigger) one to run when it finishes
    job_size: usize,
    refine_size: Option<usize>,
    node_sizes: HashMap<u32, usize>,
    /// resolution of the pass whose results are on screen (node_sizes are measured at this size)
    pass_size: usize,
    tile_preview: bool,
    thumbs: HashMap<u32, egui::TextureHandle>,
    node_err: HashMap<u32, String>,
    tex2d: Option<egui::TextureHandle>,
    tex2d_size: (usize, usize),
    maps: BTreeMap<String, std::sync::Arc<Tex>>,
    view3d: View,
    tex3d: Option<egui::TextureHandle>,
    r3d_size: usize,
    gpu: gpu3d::Shared,
    gpu_on: bool,
    gpu_maps_dirty: bool,
    /// Open packages in display order; `None` marks the one being edited (its live state is in `g`, `file`, ...).
    pkgs: Vec<Option<Pkg>>,
    pkg_active: usize,
    /// Package that new graphs go to (selected in the Explorer).
    pkg_sel: usize,
    unsaved_name: String,
    confirm_close: Option<usize>,
    dirty: bool,
    dirty3d: bool,
    r#gen: u64,
    job: Option<Receiver<EvalResult>>,
    part: Option<Receiver<Partial>>,
    node_cache: std::sync::Arc<NodeCache>,
    lib_search: String,
    text_focus: bool,
    export: ExportUi,
    bake: BakeUi,
    show_about: bool,
    screen: Screen,
    new_dlg: NewDlg,
    recent: Vec<PathBuf>,
    home_filter: String,
    home_page: u8,
    skip_home: bool,
    hide_suggestions: bool,
    banner: Option<egui::TextureHandle>,
    banner_rx: Option<Receiver<ColorImage>>,
    lib_thumbs: HashMap<String, egui::TextureHandle>,
    lib_rx: Option<Receiver<HashMap<String, ColorImage>>>,
    lib_cat: String,
    favorites: BTreeSet<String>,
    show: [bool; 5],
    modified: bool,
    last_title: String,
    z2d: f32,
    pan2d: Vec2,
    left_split: f32,
    need_fit: bool,
    w_left: f32,
    w_props: f32,
    h_views: f32,
    view_split: f32,
    connecting_in: Option<(u32, String)>,
    copy_out: Option<String>,
    logo: Option<egui::TextureHandle>,
    logo_note: String,
    /// The Paint screen (owns all of its state).
    paint: gui_paint::PaintMode,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> App {
        apply_theme(&cc.egui_ctx);
        let (logo_tex, logo_note) = logo().clone();
        let logo_handle = cc.egui_ctx.load_texture("logo", qimg(&logo_tex), egui::TextureOptions::LINEAR);
        let mut g = Graph::default();
        g.meta.insert("identifier".into(), "Graph".into());
        let active: Option<u32> = None;
        let settings = read_lines("settings.txt");
        let skip_home = settings.iter().any(|l| l == "skip_home=1");
        let favorites: BTreeSet<String> = settings.iter().find_map(|l| l.strip_prefix("favorites=")).map(|v| v.split(',').filter(|x| !x.is_empty()).map(String::from).collect()).unwrap_or_default();
        let recent: Vec<PathBuf> = read_lines("recent.txt").into_iter().map(PathBuf::from).collect();
        let paint = gui_paint::PaintMode::from_args();
        App {
            parents: Vec::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            edit_open: false,
            sel: BTreeSet::new(),
            active,
            sel_comment: None,
            clip: Vec::new(),
            pan: vec2(30.0, 30.0),
            zoom: 0.9,
            canvas_rect: Rect::from_min_size(Pos2::ZERO, vec2(900.0, 500.0)),
            connecting: None,
            connecting_out: None,
            link_out: None,
            rubber: None,
            comment_members: Vec::new(),
            search: None,
            acts: Vec::new(),
            status: ("Press Tab in the graph to add a node at the cursor.".into(), true),
            file: None,
            prev_size: 0,
            refine: false,
            job_size: 0,
            refine_size: None,
            node_sizes: HashMap::new(),
            pass_size: 0,
            tile_preview: false,
            thumbs: HashMap::new(),
            node_err: HashMap::new(),
            tex2d: None,
            tex2d_size: (0, 0),
            maps: BTreeMap::new(),
            view3d: View { tess: 512, ..View::default() },
            tex3d: None,
            r3d_size: 420,
            gpu: gpu3d::new_shared(),
            gpu_on: true,
            gpu_maps_dirty: true,
            pkgs: Vec::new(),
            pkg_active: 0,
            pkg_sel: 0,
            unsaved_name: "Unsaved Package".into(),
            confirm_close: None,
            dirty: true,
            dirty3d: true,
            r#gen: 0,
            job: None,
            part: None,
            node_cache: std::sync::Arc::new(NodeCache::with_default_budget()),
            lib_search: String::new(),
            text_focus: false,
            export: ExportUi { open: false, dir: None, prefix: String::new(), sizes: [false, true, false, false, false, false], depth16: false, fmt: Format::Png, quality: 92, skip: BTreeSet::new() },
            bake: BakeUi { open: false, low: None, high: None, out: None, s: BakeSettings { size: 512, ..Default::default() }, job: None, msg: String::new() },
            show_about: false,
            screen: if paint.wants_screen() { Screen::Paint } else if skip_home { Screen::Editor } else { Screen::Home },
            new_dlg: NewDlg { open: false, sel: 1, name: "Graph".into(), size: 2048, depth16: false, dest: None },
            recent,
            home_filter: String::new(),
            home_page: 0,
            skip_home,
            hide_suggestions: false,
            banner: None,
            banner_rx: Some(spawn_banner()),
            lib_thumbs: HashMap::new(),
            lib_rx: Some(spawn_library_thumbs()),
            lib_cat: "Texture generators/Patterns".into(),
            favorites,
            show: [true; 5],
            modified: false,
            last_title: String::new(),
            z2d: 1.0,
            pan2d: Vec2::ZERO,
            left_split: 0.33,
            need_fit: false,
            w_left: 380.0,
            w_props: 330.0,
            h_views: 330.0,
            view_split: 0.5,
            connecting_in: None,
            copy_out: None,
            logo: Some(logo_handle),
            logo_note,
            paint,
            g,
        }
    }

    /// Import a saved graph file as a subgraph of the current graph and place it as a node.
    fn import_graph_file(&mut self, p: PathBuf, at: Option<Pos2>, graph: Option<String>) {
        let loaded = std::fs::read_to_string(&p).map_err(|e| e.to_string()).and_then(|t| Graph::parse_unchecked(&t).map_err(|e| e.to_string()));
        let other = match loaded {
            Ok(g) => g,
            Err(e) => return self.status(format!("Cannot import {}: {e}", p.display()), false),
        };
        let stem: String = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "graph".into());
        self.import_graph_value(other, stem, at, graph);
    }

    /// Import a graph of an open package as a node.
    fn import_from_pkg(&mut self, i: usize, graph: Option<String>, at: Option<Pos2>) {
        let Some(pk) = self.pkgs.get(i).and_then(|p| p.as_ref()) else {
            return self.status("Drag graphs from another package onto this graph", false);
        };
        let stem = pk.name.trim_end_matches(".mcg").trim_end_matches(".mca").to_string();
        let other = pk.g.clone();
        self.import_graph_value(other, stem, at, graph);
    }

    /// Embed `other` (or one named graph of it) as a subgraph of the current graph and place it as a node.
    fn import_graph_value(&mut self, mut other: Graph, mut stem: String, at: Option<Pos2>, graph: Option<String>) {
        other.migrate_outputs();
        auto_layout_if_unplaced(&mut other);
        // a specific graph of the package: it becomes the imported root, the other graphs stay available to it
        if let Some(gn) = graph.filter(|gn| other.subgraphs.contains_key(gn)) {
            let mut sel = other.subgraphs.remove(&gn).unwrap_or_default();
            sel.subgraphs = std::mem::take(&mut other.subgraphs);
            sel.resources = other.resources.clone();
            other = sel;
            stem = format!("{stem}_{gn}");
        } else if let Some(id) = other.meta.get("identifier").filter(|s| !s.is_empty()) {
            stem = id.clone();
        }
        let base: String = stem.chars().map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' }).collect();
        let mut name = base.clone();
        let mut i = 2;
        while self.g.subgraphs.contains_key(&name) || self.parents.iter().any(|(pg, n)| pg.subgraphs.contains_key(&name) || *n == name) {
            name = format!("{base}_{i}");
            i += 1;
        }
        self.snap();
        if let Err(e) = self.g.import_graph(other, &name) {
            self.undo.pop();
            return self.status(format!("Cannot import '{stem}': {e}"), false);
        }
        // the copy only exists to feed the node, so keep it out of the Explorer's graph list
        let prefix = format!("{name}__");
        for (k, sg) in self.g.subgraphs.iter_mut() {
            if *k == name || k.starts_with(&prefix) {
                sg.meta.insert("embedded".into(), "1".into());
            }
        }
        let pos = at.unwrap_or_else(|| self.view_center_world());
        self.add_node_at("subgraph", Some(&name), pos);
        self.undo.pop(); // one undo step for the whole import
        self.status(format!("Added '{name}' as a node - drag from one of its output sockets to connect it"), true);
    }

    // ------------------------------------------------------------ workspace of packages
    fn ws_init(&mut self) {
        if self.pkgs.is_empty() {
            self.pkgs = vec![None];
            self.pkg_active = 0;
            self.pkg_sel = 0;
        }
        if self.pkg_active >= self.pkgs.len() {
            self.pkg_active = 0;
        }
        if self.pkg_sel >= self.pkgs.len() {
            self.pkg_sel = self.pkg_active;
        }
    }

    /// Start a fresh workspace holding just the package that is about to be loaded.
    fn ws_fresh(&mut self) {
        self.pkgs = vec![None];
        self.pkg_active = 0;
        self.pkg_sel = 0;
        self.confirm_close = None;
    }

    fn pkg_title(&self, i: usize) -> String {
        match self.pkgs.get(i) {
            Some(Some(p)) => p.name.clone(),
            _ => self.package_label(),
        }
    }

    fn pkg_modified(&self, i: usize) -> bool {
        match self.pkgs.get(i) {
            Some(Some(p)) => p.modified || p.path.is_none(),
            _ => self.modified || self.file.is_none(),
        }
    }

    /// True when closing package `i` would lose changes.
    fn pkg_dirty(&self, i: usize) -> bool {
        match self.pkgs.get(i) {
            Some(Some(p)) => p.modified,
            _ => self.modified,
        }
    }

    fn unique_unsaved_name(&self) -> String {
        let taken = |n: &str| (0..self.pkgs.len()).any(|i| self.pkg_title(i) == n);
        let mut n = "Unsaved Package".to_string();
        let mut k = 2;
        while taken(&n) {
            n = format!("Unsaved Package {k}");
            k += 1;
        }
        n
    }

    fn snapshot_active(&mut self) -> Pkg {
        self.goto_root();
        Pkg { path: self.file.clone(), name: self.package_label(), g: self.g.clone(), modified: self.modified }
    }

    fn load_active(&mut self, p: Pkg) {
        self.parents.clear();
        self.g = p.g;
        self.clear_views();
        self.file = p.path;
        self.unsaved_name = p.name;
        self.modified = p.modified;
        self.clear_history();
        self.thumbs.clear();
        self.screen = Screen::Editor;
        self.fit_view();
        self.active = self.g.nodes.keys().next().copied();
    }

    /// Make open package `i` the one shown in the editor.
    fn switch_pkg(&mut self, i: usize) {
        self.ws_init();
        self.pkg_sel = i.min(self.pkgs.len() - 1);
        if i == self.pkg_active || i >= self.pkgs.len() {
            return;
        }
        let Some(target) = self.pkgs[i].take() else { return };
        let snap = self.snapshot_active();
        self.pkgs[self.pkg_active] = Some(snap);
        self.load_active(target);
        self.pkg_active = i;
    }

    /// Add a new package to the workspace and edit it.
    fn add_pkg_active(&mut self, p: Pkg) {
        if self.screen != Screen::Editor {
            self.ws_fresh();
            self.load_active(p);
            return;
        }
        self.ws_init();
        let snap = self.snapshot_active();
        self.pkgs[self.pkg_active] = Some(snap);
        self.pkgs.push(None);
        self.pkg_active = self.pkgs.len() - 1;
        self.pkg_sel = self.pkg_active;
        self.load_active(p);
    }

    /// Open package files into the workspace (File > Open while editing).
    fn open_packages(&mut self, files: Vec<PathBuf>) {
        self.ws_init();
        let mut last = None;
        for p in files {
            let open_at = (0..self.pkgs.len()).find(|&i| match &self.pkgs[i] {
                Some(k) => k.path.as_ref() == Some(&p),
                None => self.file.as_ref() == Some(&p),
            });
            if let Some(i) = open_at {
                self.pkg_sel = i;
                self.status(format!("{} is already open", p.display()), false);
                continue;
            }
            match read_pkg(&p) {
                Ok(pk) => {
                    self.pkgs.push(Some(pk));
                    self.pkg_sel = self.pkgs.len() - 1;
                    self.push_recent(&p);
                    last = Some(p);
                }
                Err(e) => self.status(format!("Open failed: {}: {e}", p.display()), false),
            }
        }
        if let Some(p) = last {
            self.status(format!("Opened package {} - drag its graphs onto the canvas", p.display()), true);
        }
    }

    /// Add a new graph from the New dialog to open package `dest` and open it in the editor.
    fn add_graph_to_pkg(&mut self, dest: usize, g: Graph, name: &str) {
        self.switch_pkg(dest);
        self.goto_root();
        let base: String = name.chars().map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' }).collect();
        let base = if base.is_empty() { "Material_Graph".to_string() } else { base };
        let root_id = self.gname();
        let mut unique = base.clone();
        let mut k = 2;
        while self.g.subgraphs.contains_key(&unique) || unique == root_id {
            unique = format!("{base}_{k}");
            k += 1;
        }
        self.snap();
        self.g.subgraphs.insert(unique.clone(), g);
        self.enter_sub(&unique);
        self.modified = true;
        self.status(format!("Created graph '{unique}' in {}", self.package_label()), true);
    }

    fn close_pkg(&mut self, i: usize) {
        self.ws_init();
        if i >= self.pkgs.len() {
            return;
        }
        if self.pkgs.len() == 1 {
            self.pkgs.clear();
            self.screen = Screen::Home;
            self.modified = false;
            return;
        }
        if i == self.pkg_active {
            let t = if i + 1 < self.pkgs.len() { i + 1 } else { i - 1 };
            let Some(target) = self.pkgs[t].take() else { return };
            self.load_active(target);
            self.pkgs.remove(i);
            self.pkg_active = if t > i { t - 1 } else { t };
        } else {
            self.pkgs.remove(i);
            if i < self.pkg_active {
                self.pkg_active -= 1;
            }
        }
        self.pkg_sel = self.pkg_active;
    }

    fn status(&mut self, msg: impl Into<String>, ok: bool) {
        self.status = (msg.into(), ok);
    }

    // ------------------------------------------------------------ undo / redo
    fn snap(&mut self) {
        self.modified = true;
        self.undo.push(self.g.clone());
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn undo(&mut self) {
        if let Some(prev) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.g, prev));
            self.after_restore();
        }
    }

    fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.g, next));
            self.after_restore();
        }
    }

    fn after_restore(&mut self) {
        let g = &self.g;
        self.sel.retain(|i| g.nodes.contains_key(i));
        if self.active.map(|a| !g.nodes.contains_key(&a)).unwrap_or(false) {
            self.active = None;
        }
        self.sel_comment = None;
        self.dirty = true;
    }

    fn clear_history(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.sel.clear();
        self.active = None;
        self.sel_comment = None;
        self.dirty = true;
    }

    // ------------------------------------------------------------ graph editing
    fn depends_on(&self, a: u32, b: u32) -> bool {
        let mut stack = vec![a];
        let mut seen = BTreeSet::new();
        while let Some(x) = stack.pop() {
            if x == b {
                return true;
            }
            if !seen.insert(x) {
                continue;
            }
            if let Some(n) = self.g.nodes.get(&x) {
                stack.extend(n.ins.values().copied());
            }
        }
        false
    }

    fn try_connect(&mut self, src: u32, dst: u32, slot: &str) {
        self.try_connect_out(src, dst, slot, None);
    }

    /// Connect `src` (reading its named output when it is a graph node) to input `slot` of `dst`.
    fn try_connect_out(&mut self, src: u32, dst: u32, slot: &str, out: Option<String>) {
        if src == dst || self.depends_on(src, dst) {
            self.status("Connection rejected: it would create a cycle.", false);
            return;
        }
        self.snap();
        if let Some(n) = self.g.nodes.get_mut(&dst) {
            n.ins.insert(slot.to_string(), src);
            match out {
                Some(o) => {
                    n.in_out.insert(slot.to_string(), o);
                }
                None => {
                    n.in_out.remove(slot);
                }
            }
            self.dirty = true;
        }
    }

    fn add_node_at(&mut self, kind: &str, sub: Option<&str>, pos: Pos2) -> u32 {
        self.snap();
        let id = self.g.nodes.keys().max().copied().unwrap_or(0) + 1;
        let mut n = Node { kind: kind.to_string(), pos: [pos.x, pos.y], ..Default::default() };
        match kind {
            "pixel_processor" => {
                n.strs.insert("expr".into(), "u".into());
            }
            "subgraph" => {
                n.strs.insert("graph".into(), sub.unwrap_or("").to_string());
            }
            "colorize" => {
                for (i, c) in [(0, 0.0f32), (1, 1.0)] {
                    n.nums.insert(format!("p{i}"), c);
                    for ch in ["r", "g", "b"] {
                        n.nums.insert(format!("{ch}{i}"), c);
                    }
                }
            }
            _ => {}
        }
        for (k, v) in new_node_defaults(kind) {
            n.nums.insert((*k).to_string(), *v);
        }
        self.g.nodes.insert(id, n);
        self.sel = [id].into_iter().collect();
        self.active = Some(id);
        self.sel_comment = None;
        self.dirty = true;
        id
    }

    fn view_center_world(&self) -> Pos2 {
        let c = self.canvas_rect;
        let w = ((c.center() - c.min) - self.pan) / self.zoom;
        pos2(w.x - CARD * 0.5, w.y - CARD_H * 0.5)
    }

    fn spawn(&mut self, e: &Entry, world: Pos2, link_from: Option<u32>) -> Option<u32> {
        match e.kind.as_str() {
            "comment" => {
                self.add_comment_at(Some(world));
                return None;
            }
            "frame" => {
                self.add_comment_at(None);
                return None;
            }
            _ => {}
        }
        let id = self.add_node_at(&e.kind, e.sub.as_deref(), world);
        let in_sub = !self.parents.is_empty();
        if let Some(n) = self.g.nodes.get_mut(&id) {
            for (k, v) in &e.nums {
                n.nums.insert(k.to_string(), *v);
            }
            if e.kind == "output" {
                n.strs.insert("usage".into(), if in_sub { "out" } else { "basecolor" }.into());
            }
        }
        if let Some(src) = link_from {
            let first = slots(&e.kind).first().map(|s| s.0.to_string());
            if let Some(slot) = first {
                if src != id && e.kind != "input" {
                    let o = self.link_out.take();
                    if let Some(n) = self.g.nodes.get_mut(&id) {
                        n.ins.insert(slot.clone(), src);
                        if let Some(o) = o {
                            n.in_out.insert(slot, o);
                        }
                    }
                }
            }
        }
        Some(id)
    }

    fn new_output(&mut self, usage: &str, from: u32) {
        self.snap();
        let id = self.g.nodes.keys().max().copied().unwrap_or(0) + 1;
        let pos = self.g.nodes.get(&from).map(|n| [n.pos[0] + CARD + 70.0, n.pos[1]]).unwrap_or([0.0, 0.0]);
        let mut node = Node { kind: "output".into(), pos, ..Default::default() };
        node.strs.insert("usage".into(), usage.to_string());
        node.ins.insert("in".into(), from);
        self.g.nodes.insert(id, node);
        self.sel = [id].into_iter().collect();
        self.active = Some(id);
        self.dirty = true;
    }

    fn delete_selected(&mut self) {
        if self.sel.is_empty() {
            if let Some(ci) = self.sel_comment.take() {
                if ci < self.g.comments.len() {
                    self.snap();
                    self.g.comments.remove(ci);
                }
            }
            return;
        }
        self.snap();
        let sel = std::mem::take(&mut self.sel);
        for id in &sel {
            self.g.nodes.remove(id);
        }
        for n in self.g.nodes.values_mut() {
            n.ins.retain(|_, s| !sel.contains(s));
        }
        self.g.outputs.retain(|(_, s)| !sel.contains(s));
        self.g.exposed.retain(|e| !sel.contains(&e.node));
        self.active = None;
        self.dirty = true;
    }

    fn copy(&mut self) {
        self.clip = self.sel.iter().filter_map(|i| self.g.nodes.get(i).map(|n| (*i, n.clone()))).collect();
        if self.clip.is_empty() {
            return;
        }
        let ids: BTreeSet<u32> = self.clip.iter().map(|(i, _)| *i).collect();
        let mut tmp = Graph::default();
        for (id, n) in &self.clip {
            let mut nn = n.clone();
            nn.ins.retain(|_, s| ids.contains(s));
            tmp.nodes.insert(*id, nn);
        }
        self.copy_out = Some(format!("MaterialCraft nodes\n{}", tmp.to_text()));
        self.status(format!("Copied {} node(s)", self.clip.len()), true);
    }

    fn paste(&mut self, at: Option<Pos2>) {
        if self.clip.is_empty() {
            return;
        }
        self.snap();
        let origin = self.clip.iter().fold([f32::MAX, f32::MAX], |a, (_, n)| [a[0].min(n.pos[0]), a[1].min(n.pos[1])]);
        let target = at.unwrap_or_else(|| pos2(origin[0] + 30.0, origin[1] + 30.0));
        let mut next = self.g.nodes.keys().max().copied().unwrap_or(0) + 1;
        let mut map = HashMap::new();
        for (id, _) in &self.clip {
            map.insert(*id, next);
            next += 1;
        }
        let mut new_sel = BTreeSet::new();
        for (id, n) in self.clip.clone() {
            let mut nn = n.clone();
            nn.pos = [target.x + n.pos[0] - origin[0], target.y + n.pos[1] - origin[1]];
            nn.ins.retain(|_, s| map.contains_key(s));
            for v in nn.ins.values_mut() {
                *v = map[v];
            }
            self.g.nodes.insert(map[&id], nn);
            new_sel.insert(map[&id]);
        }
        self.active = new_sel.iter().next_back().copied();
        self.sel = new_sel;
        self.dirty = true;
    }

    fn add_comment(&mut self) {
        self.add_comment_at(None);
    }

    fn add_comment_at(&mut self, at: Option<Pos2>) {
        self.snap();
        let mut c = Comment { text: "Comment".into(), pos: [0.0, 0.0], size: [320.0, 220.0], color: 0 };
        if let Some(p) = at {
            c.pos = [p.x, p.y];
            c.size = [260.0, 130.0];
            c.text = "Comment".into();
        } else if self.sel.is_empty() {
            let p = self.view_center_world();
            c.pos = [p.x - 120.0, p.y - 90.0];
        } else {
            let (mut mn, mut mx) = ([f32::MAX; 2], [f32::MIN; 2]);
            for id in &self.sel {
                if let Some(n) = self.g.nodes.get(id) {
                    mn = [mn[0].min(n.pos[0]), mn[1].min(n.pos[1])];
                    mx = [mx[0].max(n.pos[0] + CARD), mx[1].max(n.pos[1] + BAR + card_body(slots(&n.kind).len(), 0))];
                }
            }
            c.pos = [mn[0] - 24.0, mn[1] - 44.0];
            c.size = [mx[0] - mn[0] + 48.0, mx[1] - mn[1] + 68.0];
        }
        self.g.comments.push(c);
        self.sel_comment = Some(self.g.comments.len() - 1);
        self.sel.clear();
    }

    fn make_subgraph(&mut self) {
        let mut n = 1;
        let name = loop {
            let c = format!("subgraph_{n}");
            if !self.g.subgraphs.contains_key(&c) && !self.parents.iter().any(|(p, _)| p.subgraphs.contains_key(&c)) {
                break c;
            }
            n += 1;
        };
        self.snap();
        let sel = self.sel.clone();
        match self.g.extract_subgraph(&sel, &name) {
            Ok(id) => {
                self.sel = [id].into_iter().collect();
                self.active = Some(id);
                self.dirty = true;
                self.status(format!("Created '{name}' - double-click it to edit its contents"), true);
            }
            Err(e) => {
                self.undo.pop();
                self.status(e, false);
            }
        }
    }

    /// The shown graph changed (switch, open, enter or leave a subgraph): drop the previews of the old one right away
    /// instead of leaving them on screen until the new evaluation finishes.
    fn clear_views(&mut self) {
        self.maps.clear();
        self.tex2d = None;
        self.thumbs.clear();
        self.node_err.clear();
        self.dirty3d = true;
        self.gpu_maps_dirty = true;
        self.dirty = true;
    }

    fn enter_sub(&mut self, name: &str) {
        let Some(sub) = self.g.subgraphs.get(name).cloned() else {
            return self.status(format!("subgraph '{name}' not found"), false);
        };
        let mut w = sub;
        w.subgraphs = self.g.subgraphs.clone();
        w.subgraphs.remove(name);
        w.resources = self.g.resources.clone();
        w.seed_offset = self.g.seed_offset;
        w.size = self.g.size;
        auto_layout_if_unplaced(&mut w);
        let parent = std::mem::replace(&mut self.g, w);
        self.parents.push((parent, name.to_string()));
        self.clear_views();
        self.clear_history();
        self.fit_view();
    }

    fn exit_sub(&mut self) {
        if let Some((mut parent, name)) = self.parents.pop() {
            let mut sub = std::mem::take(&mut self.g);
            let libs = std::mem::take(&mut sub.subgraphs);
            sub.resources.clear();
            for (k, v) in libs {
                parent.subgraphs.entry(k).or_insert(v);
            }
            parent.subgraphs.insert(name, sub);
            self.g = parent;
            self.clear_views();
            self.clear_history();
            self.fit_view();
        }
    }

    fn goto_root(&mut self) {
        while !self.parents.is_empty() {
            self.exit_sub();
        }
    }

    fn fit_view(&mut self) {
        if self.g.nodes.is_empty() {
            self.pan = vec2(30.0, 30.0);
            self.zoom = 0.9;
            return;
        }
        let (mut mn, mut mx) = ([f32::MAX; 2], [f32::MIN; 2]);
        for n in self.g.nodes.values() {
            mn = [mn[0].min(n.pos[0]), mn[1].min(n.pos[1])];
            mx = [mx[0].max(n.pos[0] + CARD), mx[1].max(n.pos[1] + BAR + card_body(slots(&n.kind).len(), 0))];
        }
        let c = self.canvas_rect;
        let z = ((c.width() - 80.0) / (mx[0] - mn[0]).max(1.0)).min((c.height() - 80.0) / (mx[1] - mn[1]).max(1.0)).clamp(0.3, 1.2);
        self.zoom = z;
        self.pan = vec2(40.0 - mn[0] * z, 40.0 - mn[1] * z);
    }

    fn entries(&self) -> Vec<Entry> {
        let mk = |label: &str, kind: &str, cat: &'static str, doc: &str, nums: Vec<(&'static str, f32)>| Entry { label: label.to_string(), kind: kind.to_string(), sub: None, cat, doc: doc.to_string(), nums };
        let in_sub = !self.parents.is_empty();
        let mut v: Vec<Entry> = SPECS
            .iter()
            .filter(|s| !matches!(s.kind, "input" | "subgraph" | "output" | "dot"))
            .map(|s| mk(&label(s.kind), s.kind, category(s.kind), s.doc, vec![]))
            .collect();
        v.push(mk("Output", "output", "Utility", "graph output: drives the 3D view and exports a texture", vec![]));
        if in_sub {
            v.push(mk("Input Color", "input", "Utility", "color input of the subgraph", vec![("gray", 0.0)]));
            v.push(mk("Input Grayscale", "input", "Utility", "grayscale input of the subgraph", vec![("gray", 1.0)]));
        }
        v.push(mk("Comment", "comment", "Utility", "small note placed at the cursor", vec![]));
        v.push(mk("Frame", "frame", "Utility", "frame around the selected nodes", vec![]));
        v.push(mk("Dot Node", "dot", "Utility", "re-route a link", vec![]));
        let mut subs: BTreeSet<String> = self.g.subgraphs.keys().cloned().collect();
        for (p, _) in &self.parents {
            subs.extend(p.subgraphs.keys().cloned());
        }
        for name in subs {
            v.push(Entry { label: format!("subgraph: {name}"), kind: "subgraph".into(), sub: Some(name.clone()), cat: "Subgraphs", doc: format!("instance of subgraph '{name}'"), nums: vec![] });
        }
        v
    }

    // ------------------------------------------------------------ file operations
    fn open_file(&mut self) {
        let Some(files) = rfd::FileDialog::new().add_filter("MaterialCraft graph / package", &["mcg", "mca"]).pick_files() else { return };
        if self.screen == Screen::Editor {
            // a project is already open: the opened files join the workspace as packages in the Explorer
            self.open_packages(files);
        } else if let Some(p) = files.into_iter().next() {
            self.open_path(p);
        }
    }

    fn save_to(&mut self, p: PathBuf) {
        self.goto_root();
        match std::fs::write(&p, self.g.to_text()) {
            Ok(()) => {
                self.status(format!("Saved {}", p.display()), true);
                self.push_recent(&p);
                self.file = Some(p);
                self.modified = false;
            }
            Err(e) => self.status(format!("Save failed: {e}"), false),
        }
    }

    fn save(&mut self) {
        match self.file.clone() {
            Some(p) => self.save_to(p),
            None => self.save_as(),
        }
    }

    fn save_as(&mut self) {
        if let Some(p) = rfd::FileDialog::new().add_filter("MaterialCraft graph", &["mcg"]).set_file_name("material.mcg").save_file() {
            self.save_to(p);
        }
    }

    fn publish(&mut self) {
        self.goto_root();
        match self.g.publish() {
            Ok(text) => {
                if let Some(p) = rfd::FileDialog::new().add_filter("MaterialCraft published asset", &["mca"]).set_file_name("material.mca").save_file() {
                    match std::fs::write(&p, &text) {
                        Ok(()) => self.status(format!("Published {} ({} KB, {} exposed parameters)", p.display(), text.len() / 1024, self.g.exposed.len()), true),
                        Err(e) => self.status(format!("Publish failed: {e}"), false),
                    }
                }
            }
            Err(e) => self.status(format!("Cannot publish: {e}"), false),
        }
    }

    fn import_bitmap(&mut self) {
        let Some(p) = rfd::FileDialog::new().add_filter("PNG image", &["png"]).pick_file() else { return };
        let pos = self.view_center_world();
        let id = self.add_node_at("bitmap", None, pos);
        if let Some(n) = self.g.nodes.get_mut(&id) {
            n.strs.insert("file".into(), p.to_string_lossy().to_string());
        }
    }

    fn run_export(&mut self) {
        self.goto_root();
        let Some(dir) = self.export.dir.clone() else {
            return self.status("Choose an export folder first.", false);
        };
        let sizes: Vec<usize> = [256usize, 512, 1024, 2048, 4096, 8192].iter().zip(self.export.sizes).filter(|(_, on)| *on).map(|(s, _)| *s).collect();
        if sizes.is_empty() {
            return self.status("Select at least one resolution.", false);
        }
        let idents: Vec<String> = self.g.output_nodes().into_iter().map(|(_, i, _)| i).collect();
        if idents.iter().all(|i| self.export.skip.contains(i)) {
            return self.status("Select at least one output to export.", false);
        }
        let fmt = self.export.fmt;
        let depth = if fmt != Format::Jpeg && (self.export.depth16 || self.g.meta.get("output_format").map(|f| f == "16" && false).unwrap_or(false)) { 16 } else { 8 };
        let mut count = 0;
        for &s in &sizes {
            match self.g.evaluate(Some(s)) {
                Ok(out) => {
                    let d = if sizes.len() > 1 { dir.join(s.to_string()) } else { dir.clone() };
                    if let Err(e) = std::fs::create_dir_all(&d) {
                        return self.status(format!("Export failed: {e}"), false);
                    }
                    for (name, t) in &out {
                        if self.export.skip.contains(name) {
                            continue;
                        }
                        let p = d.join(format!("{}{}.{}", self.export.prefix, name, fmt.ext()));
                        if let Err(e) = std::fs::write(&p, export::encode(fmt, t, depth, self.export.quality)) {
                            return self.status(format!("Export failed: {e}"), false);
                        }
                        count += 1;
                    }
                }
                Err(e) => return self.status(format!("Export blocked: {e}"), false),
            }
        }
        self.status(format!("Exported {count} {} file(s) to {}", fmt.name(), dir.display()), true);
    }

    fn start_bake(&mut self) {
        let (Some(lo), Some(hi), Some(out)) = (self.bake.low.clone(), self.bake.high.clone(), self.bake.out.clone()) else {
            self.bake.msg = "Choose the low-poly mesh, high-poly mesh and output folder.".into();
            return;
        };
        let s = self.bake.s.clone();
        let (tx, rx) = channel();
        self.bake.job = Some(rx);
        self.bake.msg = "Baking...".into();
        std::thread::spawn(move || {
            let r = (|| -> Result<Vec<PathBuf>, String> {
                let low = bake::parse_obj(&std::fs::read_to_string(&lo).map_err(|e| format!("{}: {e}", lo.display()))?).map_err(|e| format!("low-poly: {e}"))?;
                let high = bake::parse_obj(&std::fs::read_to_string(&hi).map_err(|e| format!("{}: {e}", hi.display()))?).map_err(|e| format!("high-poly: {e}"))?;
                let maps = bake::bake(&low, &high, &s)?;
                std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
                let mut files = Vec::new();
                for (name, t) in &maps {
                    let p = out.join(format!("{name}.png"));
                    std::fs::write(&p, png::encode(t.w, t.h, &t.px, 8)).map_err(|e| e.to_string())?;
                    files.push(p);
                }
                Ok(files)
            })();
            let _ = tx.send(r);
        });
    }

    // ------------------------------------------------------------ background evaluation
    /// First, cheap resolution of a progressive preview: a tiny pass shows the change at once, then every pass doubles
    /// the size until the chosen preview size is reached.
    /// The size a node will have in the final result (what Designer shows under a node): its size in the pass on screen,
    /// scaled up to the graph's own size.
    fn nominal_node_size(&self, id: u32) -> usize {
        match (self.node_sizes.get(&id), self.pass_size) {
            (Some(&n), p) if p > 0 => ((n as u64 * self.g.size as u64) / p as u64).clamp(1, 8192) as usize,
            _ => self.g.size,
        }
    }

    fn first_tier(&self) -> usize {
        let full = self.preview_full();
        if full > 256 {
            256
        } else {
            full
        }
    }

    fn spawn_eval(&mut self, size: usize) {
        let g = self.g.clone();
        let full = self.preview_full();
        let size = size.clamp(16, full.max(16));
        self.job_size = size;
        self.refine_size = None;
        let mut keep: Vec<u32> = g.output_nodes().iter().map(|o| o.2).collect();
        let active = self.active;
        if let Some(a) = active {
            keep.push(a);
        }
        // a newer job makes every older one stale: their loops stop within a few rows
        self.r#gen += 1;
        let r#gen = self.r#gen;
        tex::CURRENT_JOB.store(r#gen, std::sync::atomic::Ordering::Relaxed);
        let (tx, rx) = channel();
        self.job = Some(rx);
        let (ptx, prx) = channel();
        self.part = Some(prx);
        let cache = self.node_cache.clone();
        std::thread::spawn(move || {
            let mut on_first = |t: &Tex| {
                if let Some(a) = active {
                    if !tex::stale(r#gen) {
                        let _ = ptx.send(Partial { r#gen, img2d: qimg(t), img_for: a });
                    }
                }
            };
            let r = g.evaluate_all_cached(size, &keep, r#gen, &cache, active, &mut on_first);
            if tex::stale(r#gen) {
                return; // superseded: nobody waits for this result
            }
            tex::set_job(r#gen);
            let outputs: Vec<(String, u32)> = g.output_nodes().into_iter().map(|(u, _, id)| (u, id)).collect();
            let named = |n: &str| outputs.iter().find(|o| o.0 == n).and_then(|o| r.kept.get(&o.1)).map(|a| a.as_ref());
            let inv_gloss = named("glossiness").map(ops::invert);
            let gpu = gpu3d::prepare(&MapInputs {
                base: named("basecolor").or_else(|| named("out")).or_else(|| named("diffuse")),
                normal: named("normal"),
                rough: named("roughness").or(inv_gloss.as_ref()),
                metal: named("metallic"),
                ao: named("ambient_occlusion"),
                emissive: named("emissive"),
                height: named("height"),
            });
            let img2d = active.and_then(|a| r.kept.get(&a)).map(|a| qimg(a));
            tex::set_job(0);
            if tex::stale(r#gen) {
                return;
            }
            let _ = tx.send(EvalResult {
                r#gen,
                thumbs: r.thumbs,
                errors: r.errors,
                kept: r.kept,
                outputs,
                sizes: r.sizes,
                img2d,
                img_for: active,
                gpu: Some(gpu),
                size,
            });
        });
    }

    /// Resolution of the editor preview: the chosen size, or the graph's own size (at most 4096).
    fn preview_full(&self) -> usize {
        if self.prev_size == 0 {
            self.g.size.clamp(16, 4096)
        } else {
            self.prev_size
        }
    }

    fn poll_eval(&mut self, ctx: &egui::Context) {
        // the selected node usually finishes long before the whole graph: show it right away
        if let Some(prx) = &self.part {
            let mut latest = None;
            while let Ok(p) = prx.try_recv() {
                latest = Some(p);
            }
            if let Some(p) = latest {
                if p.r#gen == self.r#gen && self.active == Some(p.img_for) {
                    let opts = egui::TextureOptions {
                        magnification: egui::TextureFilter::Nearest,
                        minification: egui::TextureFilter::Linear,
                        wrap_mode: egui::TextureWrapMode::Repeat,
                        mipmap_mode: Some(egui::TextureFilter::Linear),
                    };
                    self.tex2d_size = (p.img2d.size[0], p.img2d.size[1]);
                    self.tex2d = Some(ctx.load_texture("view2d", p.img2d, opts));
                    ctx.request_repaint();
                }
            }
        }
        let Some(rx) = &self.job else { return };
        match rx.try_recv() {
            Ok(r) => {
                self.job = None;
                if r.r#gen != self.r#gen {
                    return;
                }
                // progressive preview: this pass is on screen, the next (double size) one starts right away
                let full = self.preview_full();
                if r.size < full {
                    self.refine_size = Some((r.size * 2).min(full));
                    self.refine = true;
                }
                // like Designer's 2D view: zoomed in, every texel is a crisp square; zoomed out, a mip chain keeps it smooth
                let opts = egui::TextureOptions {
                    magnification: egui::TextureFilter::Nearest,
                    minification: egui::TextureFilter::Linear,
                    wrap_mode: egui::TextureWrapMode::Repeat,
                    mipmap_mode: Some(egui::TextureFilter::Linear),
                };
                self.thumbs.retain(|id, _| r.thumbs.contains_key(id));
                for (id, bytes) in &r.thumbs {
                    let img = ColorImage::from_rgba_unmultiplied([THUMB, THUMB], bytes);
                    match self.thumbs.get_mut(id) {
                        Some(h) => h.set(img, egui::TextureOptions::LINEAR),
                        None => {
                            self.thumbs.insert(*id, ctx.load_texture(format!("thumb{id}"), img, egui::TextureOptions::LINEAR));
                        }
                    }
                }
                self.node_err = r.errors;
                self.node_sizes = r.sizes;
                self.pass_size = r.size;
                if let Some(a) = self.active {
                    if let (Some(img), true) = (r.img2d, r.img_for == Some(a)) {
                        self.tex2d_size = (img.size[0], img.size[1]);
                        self.tex2d = Some(ctx.load_texture("view2d", img, opts));
                    } else if let Some(t) = r.kept.get(&a) {
                        self.tex2d_size = (t.w, t.h);
                        self.tex2d = Some(ctx.load_texture("view2d", qimg(t), opts));
                    } else {
                        self.tex2d = None;
                    }
                } else {
                    self.tex2d = None;
                }
                self.maps.clear();
                for (name, id) in &r.outputs {
                    if let Some(t) = r.kept.get(id) {
                        self.maps.insert(name.clone(), t.clone());
                    }
                }
                if !self.maps.contains_key("basecolor") {
                    if let Some(t) = self.maps.get("out").cloned() {
                        self.maps.insert("basecolor".into(), t);
                    }
                }
                self.dirty3d = true;
                if let Some(m) = r.gpu {
                    if let Ok(mut g) = self.gpu.lock() {
                        g.set_maps(m);
                    }
                    self.gpu_maps_dirty = false;
                } else {
                    self.gpu_maps_dirty = true;
                }
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                self.job = None;
                self.status("Evaluation failed unexpectedly.", false);
            }
        }
    }

    /// Hand the current graph outputs to the GPU viewer (uploaded on its next paint).
    fn upload_gpu_maps(&mut self) {
        self.gpu_maps_dirty = false;
        let get = |k: &str| self.maps.get(k).map(|a| a.as_ref());
        let inv_gloss = get("glossiness").map(ops::invert);
        let m = MapInputs {
            base: get("basecolor").or_else(|| get("diffuse")),
            normal: get("normal"),
            rough: get("roughness").or(inv_gloss.as_ref()),
            metal: get("metallic"),
            ao: get("ambient_occlusion"),
            emissive: get("emissive"),
            height: get("height"),
        };
        let prepared = gpu3d::prepare(&m);
        if let Ok(mut g) = self.gpu.lock() {
            g.set_maps(prepared);
        }
    }

    fn gpu_ready(&self) -> bool {
        self.gpu_on && self.gpu.lock().map(|g| g.ready()).unwrap_or(false)
    }

    fn render_3d(&mut self, ctx: &egui::Context) {
        self.dirty3d = false;
        if self.gpu_maps_dirty {
            self.upload_gpu_maps();
        }
        if self.gpu_ready() {
            ctx.request_repaint();
            return;
        }
        let (flat, half, zero, one) = (Tex::solid(4, 4, [0.5, 0.5, 1.0, 1.0]), Tex::solid(4, 4, [0.5, 0.5, 0.5, 1.0]), Tex::solid(4, 4, [0.0, 0.0, 0.0, 1.0]), Tex::solid(4, 4, [1.0; 4]));
        let get = |k: &str| self.maps.get(k).map(|a| a.as_ref());
        let inv_gloss = get("glossiness").map(ops::invert);
        let m = Maps {
            base: get("basecolor").or_else(|| get("diffuse")).unwrap_or(&half),
            normal: get("normal").unwrap_or(&flat),
            rough: get("roughness").or(inv_gloss.as_ref()).unwrap_or(&half),
            metal: get("metallic").unwrap_or(&zero),
            ao: get("ambient_occlusion").unwrap_or(&one),
            emissive: get("emissive"),
            height: get("height"),
        };
        let mut cpu_view = self.view3d.clone();
        cpu_view.tess = cpu_view.tess.min(256);
        let img = render3d::render(&m, &cpu_view, self.r3d_size, self.r3d_size);
        self.tex3d = Some(ctx.load_texture("view3d", qimg(&img), egui::TextureOptions::LINEAR));
    }
}

// ---------------------------------------------------------------- free helpers
fn auto_layout(g: &mut Graph) {
    let ids: Vec<u32> = g.nodes.keys().copied().collect();
    let mut level: HashMap<u32, usize> = ids.iter().map(|i| (*i, 0)).collect();
    for _ in 0..=ids.len() {
        let mut changed = false;
        for id in &ids {
            let l = g.nodes[id].ins.values().map(|s| level.get(s).copied().unwrap_or(0) + 1).max().unwrap_or(0);
            if l > level[id] && l <= ids.len() {
                level.insert(*id, l);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let mut rows: HashMap<usize, f32> = HashMap::new();
    for id in &ids {
        let l = level[id];
        let y = rows.entry(l).or_insert(0.0);
        if let Some(n) = g.nodes.get_mut(id) {
            n.pos = [l as f32 * (CARD + 70.0), *y];
        }
        *y += CARD_H + 40.0;
    }
}

fn auto_layout_if_unplaced(g: &mut Graph) {
    if g.nodes.len() > 1 && g.nodes.values().all(|n| n.pos == [0.0, 0.0]) {
        auto_layout(g);
    }
}

fn apply_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    let mut v = egui::Visuals::dark();
    v.panel_fill = Color32::from_rgb(46, 46, 46);
    v.window_fill = Color32::from_rgb(52, 52, 52);
    v.extreme_bg_color = Color32::from_rgb(30, 30, 30);
    v.faint_bg_color = Color32::from_rgb(52, 52, 52);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(28, 28, 28));
    v.widgets.inactive.bg_fill = Color32::from_rgb(66, 66, 66);
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(62, 62, 62);
    v.widgets.hovered.bg_fill = Color32::from_rgb(88, 88, 88);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(84, 84, 84);
    v.widgets.active.bg_fill = Color32::from_rgb(235, 130, 20);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(235, 130, 20);
    v.selection.bg_fill = Color32::from_rgb(220, 120, 20);
    v.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);
    v.window_rounding = Rounding::same(3.0);
    v.menu_rounding = Rounding::same(3.0);
    style.visuals = v;
    style.text_styles.insert(TextStyle::Body, FontId::proportional(12.5));
    style.text_styles.insert(TextStyle::Button, FontId::proportional(12.5));
    style.text_styles.insert(TextStyle::Small, FontId::proportional(10.5));
    style.text_styles.insert(TextStyle::Heading, FontId::proportional(14.0));
    style.spacing.item_spacing = vec2(6.0, 4.0);
    style.spacing.button_padding = vec2(6.0, 2.0);
    style.spacing.slider_width = 150.0;
    ctx.set_style(style);
}

include!("gui_params.rs");
include!("gui_ui.rs");
include!("gui_home.rs");

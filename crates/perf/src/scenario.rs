//! [`Scenario`]: one benchmark described as data: where the document comes from ([`Source`]), what is done to it
//! ([`Action`]) and how often. Read from and written to JSON with the core's json module:
//!
//! ```json
//! {"id": "erase-2k", "about": "Eraser over a painted stroke",
//!  "source": {"mesh": "/path/LP.obj", "resolution": 2048},
//!  "action": {"stroke": {"tool": "erase", "primed": true, "path": "wave", "samples": 200, "per_frame": 2}},
//!  "repeat": 1}
//! ```
//!
//! Sources: `{"mesh": <obj or glTF>, "resolution": <px>}` (a new project, File > New project) or
//! `{"document": <.mcpaint or .spp>}`. Actions: `"open"` (open and composite every set, `repeat` more times),
//! `{"stroke": {..}}` ([`StrokeSpec`], every field optional) or `{"edits": ["opacity", ..]}` ([`EditKind`]).

use materialcraft::json::Value;
use std::path::{Path, PathBuf};

/// One benchmark.
#[derive(Debug, Clone, PartialEq)]
pub struct Scenario {
    /// Short, stable name (`stroke-2k`): the command line and the history use it.
    pub id: String,
    /// One line on what it measures.
    pub about: String,
    pub source: Source,
    pub action: Action,
    /// How often the action is played on the opened document (at least 1).
    pub repeat: u32,
}

/// Where the document comes from.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// A new project made from a mesh with the first built-in template, every set at `resolution`.
    NewProject { mesh: PathBuf, resolution: u32 },
    /// A saved `.mcpaint`, or a Painter `.spp` imported on the fly.
    Document(PathBuf),
}

impl Source {
    /// The file the source reads.
    pub fn path(&self) -> &Path {
        match self {
            Source::NewProject { mesh, .. } => mesh,
            Source::Document(p) => p,
        }
    }
}

/// What is done to the opened document.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Composite every set again (the cost of opening).
    Open,
    Stroke(StrokeSpec),
    /// Layer edits in this order, each one history step, then the recomposite of what it reached.
    Edits(Vec<EditKind>),
}

/// A brush stroke across the view.
#[derive(Debug, Clone, PartialEq)]
pub struct StrokeSpec {
    pub tool: StrokeTool,
    /// A brush preset file (`.mcbrush`) for the tool; `None`: the tool's default brush.
    pub preset: Option<PathBuf>,
    /// Brush diameter (the preset's size units: a fraction of the mesh radius by default); `None`: the preset's.
    pub size: Option<f32>,
    pub target: StrokeTarget,
    pub layer: LayerChoice,
    /// The texture set painted, by name; `None`: the first set of a new project, else the set under the stroke.
    pub set: Option<String>,
    pub symmetry: Symmetry,
    pub path: PathShape,
    /// Pointer samples along the path.
    pub samples: usize,
    /// Samples delivered per frame.
    pub per_frame: usize,
    /// Paint the same path once before measuring, so the eraser and smudge have something to work on.
    pub primed: bool,
    /// After the strokes, undo the last one and redo it (measured).
    pub undo_redo: bool,
}

impl Default for StrokeSpec {
    fn default() -> StrokeSpec {
        StrokeSpec {
            tool: StrokeTool::Paint,
            preset: None,
            size: None,
            target: StrokeTarget::Content,
            layer: LayerChoice::TopPaint,
            set: None,
            symmetry: Symmetry::Off,
            path: PathShape::Wave,
            samples: 200,
            per_frame: 2,
            primed: false,
            undo_redo: false,
        }
    }
}

/// Declares a fieldless enum with its JSON names.
macro_rules! named {
    ($(#[$m:meta])* $name:ident { $($(#[$vm:meta])* $v:ident = $s:literal),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $name { $($(#[$vm])* $v),+ }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$v),+];

            /// The JSON name.
            pub fn name(self) -> &'static str {
                match self { $($name::$v => $s),+ }
            }

            /// The value named `s`.
            pub fn from_name(s: &str) -> Result<$name, String> {
                match s {
                    $($s => Ok($name::$v),)+
                    _ => Err(format!("unknown {} {s:?} (one of: {})", stringify!($name), [$($s),+].join(", "))),
                }
            }
        }
    };
}

named!(
    /// The brush tool.
    StrokeTool { Paint = "paint", Erase = "erase", Smudge = "smudge", Blur = "blur" }
);

named!(
    /// What the stroke paints into.
    StrokeTarget {
        /// The layer's own channel images.
        Content = "content",
        /// The layer's mask (a white one is added first when it has none).
        Mask = "mask",
    }
);

named!(
    /// Which layer is painted.
    LayerChoice {
        /// The topmost paint layer of the set (a new one when it has none).
        TopPaint = "top-paint",
        /// A new paint layer on top of the stack.
        New = "new",
    }
);

named!(
    /// The pointer's path across the view.
    PathShape {
        /// Straight across the middle.
        Line = "line",
        /// Across the middle, waving up and down.
        Wave = "wave",
        /// Three turns outwards from the centre.
        Spiral = "spiral",
    }
);

named!(
    /// One layer edit, through the layer panel's path (or the history's, for undo and redo). Edits act on the
    /// middle top-level layer of the set with the most layers; mask edits on its first masked layer, effect edits
    /// on its first mask generator or filter.
    EditKind {
        Opacity = "opacity",
        Hide = "hide",
        Show = "show",
        BlendMode = "blend-mode",
        /// Invert a mask.
        Mask = "mask",
        /// Switch a mask off and back on (two steps).
        MaskToggle = "mask-toggle",
        /// Change a numeric parameter of an effect.
        EffectParam = "effect-param",
        /// Switch an effect off and back on (two steps).
        EffectToggle = "effect-toggle",
        Undo = "undo",
        Redo = "redo",
    }
);

/// Symmetry of the stroke (object axes 0 = X, 1 = Y, 2 = Z).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Symmetry {
    Off,
    Mirror { axis: usize },
    Radial { axis: usize, count: u32 },
}

const AXES: [&str; 3] = ["x", "y", "z"];

fn axis_name(axis: usize) -> &'static str {
    AXES.get(axis).copied().unwrap_or("x")
}

fn axis_of(v: &Value) -> Result<usize, String> {
    let s = v.as_str().ok_or("a symmetry axis is \"x\", \"y\" or \"z\"")?;
    AXES.iter().position(|a| *a == s).ok_or_else(|| format!("unknown axis {s:?}"))
}

impl Symmetry {
    pub fn to_json(self) -> Value {
        match self {
            Symmetry::Off => "off".into(),
            Symmetry::Mirror { axis } => Value::object().with("mirror", axis_name(axis)),
            Symmetry::Radial { axis, count } => Value::object().with("radial", axis_name(axis)).with("count", count),
        }
    }

    pub fn from_json(v: &Value) -> Result<Symmetry, String> {
        if v.as_str() == Some("off") || v.is_null() {
            return Ok(Symmetry::Off);
        }
        if let Some(a) = v.get("mirror") {
            return Ok(Symmetry::Mirror { axis: axis_of(a)? });
        }
        if let Some(a) = v.get("radial") {
            let count = v.get("count").map_or(Some(4), |c| c.as_u64()).and_then(|c| u32::try_from(c).ok()).ok_or("radial count is a whole number")?;
            return Ok(Symmetry::Radial { axis: axis_of(a)?, count });
        }
        Err(format!("symmetry is \"off\", {{\"mirror\": axis}} or {{\"radial\": axis, \"count\": n}}, not {v}"))
    }
}

impl Scenario {
    pub fn to_json(&self) -> Value {
        let source = match &self.source {
            Source::NewProject { mesh, resolution } => Value::object().with("mesh", mesh.display().to_string()).with("resolution", *resolution),
            Source::Document(p) => Value::object().with("document", p.display().to_string()),
        };
        let action = match &self.action {
            Action::Open => Value::from("open"),
            Action::Stroke(s) => Value::object().with("stroke", s.to_json()),
            Action::Edits(list) => Value::object().with("edits", list.iter().map(|e| e.name()).collect::<Vec<_>>()),
        };
        Value::object()
            .with("id", self.id.as_str())
            .with("about", self.about.as_str())
            .with("source", source)
            .with("action", action)
            .with("repeat", self.repeat)
    }

    pub fn from_json(v: &Value) -> Result<Scenario, String> {
        known_keys(v, &["id", "about", "source", "action", "repeat"], "scenario")?;
        let id = v["id"].as_str().filter(|s| !s.is_empty()).ok_or("a scenario needs an \"id\"")?.to_string();
        let within = |e: String| format!("scenario {id}: {e}");
        let about = v["about"].as_str().unwrap_or_default().to_string();
        let source = source_from(&v["source"]).map_err(within)?;
        let action = action_from(&v["action"]).map_err(within)?;
        let repeat = match v.get("repeat") {
            None => 1,
            Some(r) => r.as_u64().and_then(|r| u32::try_from(r).ok()).filter(|r| *r > 0).ok_or_else(|| within("repeat is a positive whole number".into()))?,
        };
        Ok(Scenario { id, about, source, action, repeat })
    }
}

fn source_from(v: &Value) -> Result<Source, String> {
    if let Some(mesh) = v.get("mesh") {
        known_keys(v, &["mesh", "resolution"], "source")?;
        let mesh = mesh.as_str().ok_or("\"mesh\" is a path")?;
        let resolution = match v.get("resolution") {
            None => 2048,
            Some(r) => r.as_u64().and_then(|r| u32::try_from(r).ok()).filter(|r| r.is_power_of_two() && *r >= 16).ok_or("resolution is a power of two")?,
        };
        return Ok(Source::NewProject { mesh: PathBuf::from(mesh), resolution });
    }
    if let Some(doc) = v.get("document") {
        known_keys(v, &["document"], "source")?;
        return Ok(Source::Document(PathBuf::from(doc.as_str().ok_or("\"document\" is a path")?)));
    }
    Err("the source is {\"mesh\": path, \"resolution\": px} or {\"document\": path}".into())
}

fn action_from(v: &Value) -> Result<Action, String> {
    if v.as_str() == Some("open") {
        return Ok(Action::Open);
    }
    if let Some(s) = v.get("stroke") {
        return Ok(Action::Stroke(StrokeSpec::from_json(s)?));
    }
    if let Some(list) = v.get("edits") {
        let items = list.as_array().ok_or("\"edits\" is a list of names")?;
        let edits =
            items.iter().map(|e| e.as_str().ok_or_else(|| format!("{e} is not an edit name")).and_then(EditKind::from_name)).collect::<Result<Vec<_>, _>>()?;
        if edits.is_empty() {
            return Err("\"edits\" is empty".into());
        }
        return Ok(Action::Edits(edits));
    }
    Err("the action is \"open\", {\"stroke\": {..}} or {\"edits\": [..]}".into())
}

impl StrokeSpec {
    pub fn to_json(&self) -> Value {
        let mut v = Value::object().with("tool", self.tool.name());
        if let Some(p) = &self.preset {
            v.insert("preset", p.display().to_string());
        }
        if let Some(s) = self.size {
            v.insert("size", s);
        }
        if let Some(s) = &self.set {
            v.insert("set", s.as_str());
        }
        v.with("target", self.target.name())
            .with("layer", self.layer.name())
            .with("symmetry", self.symmetry.to_json())
            .with("path", self.path.name())
            .with("samples", self.samples)
            .with("per_frame", self.per_frame)
            .with("primed", self.primed)
            .with("undo_redo", self.undo_redo)
    }

    /// Missing fields keep their defaults; unknown ones are an error (a typo would otherwise be ignored).
    pub fn from_json(v: &Value) -> Result<StrokeSpec, String> {
        known_keys(v, &["tool", "preset", "size", "target", "layer", "set", "symmetry", "path", "samples", "per_frame", "primed", "undo_redo"], "stroke")?;
        let mut s = StrokeSpec::default();
        let text = |key: &str| v.get(key).map(|x| x.as_str().ok_or_else(|| format!("\"{key}\" is a string")));
        if let Some(t) = text("tool") {
            s.tool = StrokeTool::from_name(t?)?;
        }
        if let Some(t) = text("preset") {
            s.preset = Some(PathBuf::from(t?));
        }
        if let Some(t) = text("target") {
            s.target = StrokeTarget::from_name(t?)?;
        }
        if let Some(t) = text("layer") {
            s.layer = LayerChoice::from_name(t?)?;
        }
        if let Some(t) = text("set") {
            s.set = Some(t?.to_string());
        }
        if let Some(t) = text("path") {
            s.path = PathShape::from_name(t?)?;
        }
        if let Some(x) = v.get("size") {
            s.size = Some(x.as_f64().filter(|x| *x > 0.0).ok_or("\"size\" is a positive number")? as f32);
        }
        if let Some(x) = v.get("symmetry") {
            s.symmetry = Symmetry::from_json(x)?;
        }
        let count = |key: &str, default: usize| -> Result<usize, String> {
            v.get(key).map_or(Ok(default), |x| x.as_usize().filter(|n| *n > 0).ok_or_else(|| format!("\"{key}\" is a positive whole number")))
        };
        s.samples = count("samples", s.samples)?.max(2);
        s.per_frame = count("per_frame", s.per_frame)?;
        let flag = |key: &str| v.get(key).map_or(Ok(false), |x| x.as_bool().ok_or_else(|| format!("\"{key}\" is true or false")));
        s.primed = flag("primed")?;
        s.undo_redo = flag("undo_redo")?;
        Ok(s)
    }
}

/// An error naming the first key of object `v` not in `known`.
fn known_keys(v: &Value, known: &[&str], what: &str) -> Result<(), String> {
    let entries = v.as_object().ok_or_else(|| format!("the {what} is an object"))?;
    match entries.iter().find(|(k, _)| !known.contains(&k.as_str())) {
        Some((k, _)) => Err(format!("unknown {what} field {k:?} (known: {})", known.join(", "))),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Scenario {
        Scenario {
            id: "t".into(),
            about: "a test".into(),
            source: Source::NewProject { mesh: PathBuf::from("/m.obj"), resolution: 4096 },
            action: Action::Stroke(StrokeSpec {
                tool: StrokeTool::Erase,
                size: Some(0.25),
                target: StrokeTarget::Mask,
                symmetry: Symmetry::Radial { axis: 1, count: 6 },
                path: PathShape::Spiral,
                primed: true,
                ..StrokeSpec::default()
            }),
            repeat: 3,
        }
    }

    #[test]
    fn round_trips_through_json() {
        let s = sample();
        let text = s.to_json().to_string();
        let back = Scenario::from_json(&materialcraft::json::parse(&text).unwrap()).unwrap();
        assert_eq!(back, s);
        let edits = Scenario { action: Action::Edits(EditKind::ALL.to_vec()), source: Source::Document("/d.mcpaint".into()), ..s };
        assert_eq!(Scenario::from_json(&edits.to_json()).unwrap(), edits);
    }

    #[test]
    fn missing_fields_take_defaults() {
        let v = materialcraft::json::parse(r#"{"id": "x", "source": {"mesh": "/m.obj"}, "action": {"stroke": {}}}"#).unwrap();
        let s = Scenario::from_json(&v).unwrap();
        assert_eq!(s.repeat, 1);
        assert_eq!(s.source, Source::NewProject { mesh: "/m.obj".into(), resolution: 2048 });
        assert_eq!(s.action, Action::Stroke(StrokeSpec::default()));
    }

    #[test]
    fn rejects_typos_and_bad_values() {
        for bad in [
            r#"{"id": "x", "source": {"mesh": "/m.obj"}, "action": {"stroke": {"tol": "paint"}}}"#,
            r#"{"id": "x", "source": {"mesh": "/m.obj"}, "action": {"stroke": {"tool": "pencil"}}}"#,
            r#"{"id": "x", "source": {"mesh": "/m.obj", "resolution": 1000}, "action": "open"}"#,
            r#"{"id": "x", "source": {"document": "/d"}, "action": {"edits": ["opacty"]}}"#,
            r#"{"id": "x", "source": {"document": "/d"}, "action": "open", "repeat": 0}"#,
            r#"{"source": {"document": "/d"}, "action": "open"}"#,
        ] {
            assert!(Scenario::from_json(&materialcraft::json::parse(bad).unwrap()).is_err(), "{bad}");
        }
    }
}

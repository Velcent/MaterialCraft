//! `fragment.json`: the fragment's header (kind, name, resolution, needed maps, embedded assets) and its content
//! in the document manifest's own form: `channels` and `layers` for a smart material, one `mask` object for a smart
//! mask. Content is written and read through the texture-set mapping of the document manifest on the fragment's
//! carrier set ([`Fragment::carrier`]), so both formats stay one mapping.

use super::super::manifest::fields::Obj;
use super::super::manifest::{GENERATOR, ReadCtx, WriteCtx, texture_set};
use super::super::version::Version;
use super::super::{FORMAT_VERSION, FormatError, embedded};
use crate::json::Value;
use crate::paint::fragment::{CARRIER_LAYER, CARRIER_SET, Fragment, FragmentKind};
use crate::paint::{BakedKind, TextureSet};

/// Value of the `format` field of a fragment.
pub const FRAGMENT_FORMAT: &str = "mcfrag";

/// What a fragment's header says, read without its content (for the asset catalogue).
#[derive(Debug, Clone, PartialEq)]
pub struct FragmentInfo {
    pub kind: FragmentKind,
    pub name: String,
    pub resolution: (u32, u32),
    pub needs: Vec<BakedKind>,
    /// The thumbnail entry, if the fragment has one.
    pub thumbnail: Option<String>,
}

/// The manifest of `f`, whose carrier set `set` was planned in `ctx`.
pub fn write(f: &Fragment, set: &TextureSet, ctx: &WriteCtx, thumbnail: Option<&str>) -> Value {
    let carried = texture_set::write(0, set, ctx);
    let field = |key: &str| carried.get(key).cloned().unwrap_or_else(Value::array);
    let needs: Value = f.needs.iter().map(|k| k.id()).collect();
    let mut v = Value::object()
        .with("format", FRAGMENT_FORMAT)
        .with("format_version", FORMAT_VERSION)
        .with("generator", GENERATOR)
        .with("kind", f.kind().id())
        .with("name", f.name.as_str())
        .with("resolution", vec![f.resolution.0, f.resolution.1])
        .with("needs", needs);
    match f.kind() {
        FragmentKind::SmartMaterial => {
            v.insert("channels", field("channels"));
            v.insert("layers", field("layers"));
        }
        FragmentKind::SmartMask => {
            let mask = carried.get("layers").and_then(|l| l.at(0)).and_then(|l| l.get("mask")).cloned().unwrap_or_else(Value::object);
            v.insert("mask", mask);
        }
    }
    if !f.assets.is_empty() {
        v.insert("assets", embedded::manifest(&f.assets));
    }
    if let Some(t) = thumbnail {
        v.insert("thumbnail", t);
    }
    v
}

/// Reads the header: `format`, `format_version`, `kind`, `name`, `resolution`, `needs`, `thumbnail`.
pub fn read_info(o: &mut Obj) -> Result<FragmentInfo, FormatError> {
    if o.opt("format").and_then(Value::as_str) != Some(FRAGMENT_FORMAT) {
        return Err(FormatError::NotFragment);
    }
    Version::check_readable(o.str("format_version")?)?;
    o.opt("generator");
    let kind = o.str("kind")?;
    let kind = FragmentKind::from_id(kind).ok_or_else(|| o.error("kind", format!("unknown fragment kind {kind:?}")))?;
    let needs = o.array("needs")?.iter().map(|v| v.as_str().map(BakedKind::from_id)).collect::<Option<Vec<_>>>();
    let needs = needs.ok_or_else(|| o.error("needs", "expected an array of baked map kinds"))?;
    Ok(FragmentInfo {
        kind,
        name: o.str("name")?.to_string(),
        resolution: o.size("resolution")?,
        needs,
        thumbnail: o.opt_str("thumbnail")?.map(str::to_string),
    })
}

/// A read manifest: the header, the carrier set with correctly sized, still empty images (their entries and
/// stroke records are in `ctx`), and the references of the embedded assets.
pub struct ReadFragment {
    pub info: FragmentInfo,
    pub set: TextureSet,
    pub assets: Vec<String>,
}

/// Reads the whole manifest.
pub fn read(root: &Value, ctx: &mut ReadCtx) -> Result<ReadFragment, FormatError> {
    let mut o = Obj::new(root, "")?;
    let info = read_info(&mut o)?;
    let layers = match info.kind {
        FragmentKind::SmartMaterial => o.req("layers")?.clone(),
        FragmentKind::SmartMask => {
            let carrier = Value::object().with("id", CARRIER_LAYER).with("name", "").with("kind", "paint").with("mask", o.req("mask")?.clone());
            Value::Array(vec![carrier])
        }
    };
    let channels = o.opt("channels").cloned().unwrap_or_else(Value::array);
    let carried = Value::object()
        .with("id", CARRIER_SET)
        .with("name", info.name.as_str())
        .with("mesh_material", 0usize)
        .with("uv_set", 0usize)
        .with("resolution", vec![info.resolution.0, info.resolution.1])
        .with("channels", channels)
        .with("layers", layers);
    let set = texture_set::read(0, &carried, ctx)?;
    let assets = embedded::references(o.array("assets")?, &o.path("assets"))?;
    Ok(ReadFragment { info, set, assets })
}

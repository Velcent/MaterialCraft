//! Effects (since 0.3): `{"kind", "enabled", "label", "blend", …}` plus the fields of the kind. The same
//! object layout serves content and mask stacks; what differs (blend, paint pixels, fill values) is written by
//! the stack's [`DomainCodec`] ([`super::effect_content`], [`super::effect_mask`]). An unknown kind is kept
//! as the whole object and written back unchanged.

use super::extras::loc;
use super::fields::Obj;
use super::layer::SetScope;
use super::{ReadCtx, WriteCtx, effect_kinds as kinds};
use crate::json::Value;
use crate::paint::effect::{Effect, EffectDomain, EffectKind, PolygonFill, UnknownEffect};
use crate::paint::format::FormatError;

/// Where an effect sits: its texture set, layer, index in its stack, extras location and JSON path.
pub struct EffectAt<'a> {
    pub set: &'a SetScope<'a>,
    pub layer: &'a str,
    pub index: usize,
    /// Extras location of the effect object, e.g. `texture_sets/body/layers/l3/effects/0`.
    pub here: String,
}

/// The parts of an effect that depend on the stack it is in.
pub trait DomainCodec: EffectDomain {
    /// The blend of an effect that does not state one.
    fn default_blend() -> Self::Blend;
    fn write_blend(blend: &Self::Blend, at: &EffectAt, ctx: &WriteCtx) -> Value;
    fn read_blend(o: &mut Obj, at: &EffectAt, ctx: &mut ReadCtx) -> Result<Self::Blend, FormatError>;
    /// Adds the paint effect's pixel fields to `v`.
    fn write_pixels(p: &Self::Pixels, at: &EffectAt, ctx: &WriteCtx, v: &mut Value);
    fn read_pixels(o: &mut Obj, at: &EffectAt, ctx: &mut ReadCtx) -> Result<Self::Pixels, FormatError>;
    /// Adds the fill values' fields to `v`.
    fn write_values(values: &Self::Values, at: &EffectAt, ctx: &WriteCtx, v: &mut Value);
    fn read_values(o: &mut Obj, at: &EffectAt, ctx: &mut ReadCtx) -> Result<Self::Values, FormatError>;
}

/// An effect stack; `here` is the extras location of the object holding the `effects` array.
pub fn write_all<D: DomainCodec>(effects: &[Effect<D>], set: &SetScope, layer: &str, here: &str, ctx: &WriteCtx) -> Value {
    effects.iter().enumerate().map(|(index, e)| write(e, &EffectAt { set, layer, index, here: loc(here, "effects", &index.to_string()) }, ctx)).collect()
}

fn write<D: DomainCodec>(e: &Effect<D>, at: &EffectAt, ctx: &WriteCtx) -> Value {
    if let EffectKind::Unknown(u) = &e.kind {
        return kinds::write_unknown(u);
    }
    let mut v =
        Value::object().with("kind", e.kind.id()).with("enabled", e.enabled).with("label", e.label.as_str()).with("blend", D::write_blend(&e.blend, at, ctx));
    match &e.kind {
        EffectKind::Paint(p) => D::write_pixels(p, at, ctx, &mut v),
        EffectKind::Fill(values) => D::write_values(values, at, ctx, &mut v),
        EffectKind::Generator(p) | EffectKind::Filter(p) => kinds::write_procedural(p, &mut v),
        EffectKind::Levels(l) => kinds::write_levels(l, &mut v),
        EffectKind::ColorSelection(c) => kinds::write_color_selection(c, &mut v),
        EffectKind::PolygonFill(p) => {
            kinds::write_selection(&p.selection, &at.here, ctx.extras, &mut v);
            D::write_values(&p.values, at, ctx, &mut v);
        }
        EffectKind::Anchor(a) => {
            v.insert("id", a.id.as_str());
        }
        EffectKind::Reference(r) => {
            v.insert("anchor", r.anchor.as_str());
        }
        EffectKind::Unknown(_) => {}
    }
    ctx.extras.inject(&at.here, &mut v);
    v
}

/// Reads an effect stack; `at` is its JSON path, `here` the extras location of the object holding it.
pub fn read_all<D: DomainCodec>(list: &[Value], at: &str, set: &SetScope, layer: &str, here: &str, ctx: &mut ReadCtx) -> Result<Vec<Effect<D>>, FormatError> {
    let mut out = Vec::with_capacity(list.len());
    for (index, v) in list.iter().enumerate() {
        let place = EffectAt { set, layer, index, here: loc(here, "effects", &index.to_string()) };
        out.push(read(v, format!("{at}[{index}]"), &place, ctx)?);
    }
    Ok(out)
}

fn read<D: DomainCodec>(v: &Value, path: String, at: &EffectAt, ctx: &mut ReadCtx) -> Result<Effect<D>, FormatError> {
    let mut o = Obj::new(v, path)?;
    let kind_id = o.str("kind")?;
    if !kinds::KNOWN.contains(&kind_id) {
        // A newer kind: kept whole, not computable.
        let kind = EffectKind::Unknown(UnknownEffect { kind: kind_id.to_string(), data: v.clone() });
        return Ok(Effect { enabled: true, label: String::new(), blend: D::default_blend(), kind });
    }
    let enabled = o.bool_or("enabled", true)?;
    let label = o.opt_str("label")?.unwrap_or_default().to_string();
    let blend = D::read_blend(&mut o, at, ctx)?;
    let kind = match kind_id {
        "paint" => EffectKind::Paint(D::read_pixels(&mut o, at, ctx)?),
        "fill" => EffectKind::Fill(D::read_values(&mut o, at, ctx)?),
        "generator" => EffectKind::Generator(kinds::read_procedural(&mut o)?),
        "filter" => EffectKind::Filter(kinds::read_procedural(&mut o)?),
        "levels" => EffectKind::Levels(kinds::read_levels(&mut o)?),
        "color_selection" => EffectKind::ColorSelection(kinds::read_color_selection(&mut o)?),
        "polygon_fill" => {
            let selection = kinds::read_selection(&mut o, &at.here, ctx)?;
            EffectKind::PolygonFill(PolygonFill { selection, values: D::read_values(&mut o, at, ctx)? })
        }
        "anchor" => EffectKind::Anchor(kinds::read_anchor(&mut o)?),
        _ => EffectKind::Reference(kinds::read_reference(&mut o)?),
    };
    ctx.extras.record(&at.here, &o);
    Ok(Effect { enabled, label, blend, kind })
}

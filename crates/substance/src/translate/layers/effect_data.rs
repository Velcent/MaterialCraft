//! The data of Painter's effects that maps the same way in content and mask stacks: procedurals (generators
//! and filters, by `substance:<resource name>` with their parameter values as strings), levels, colour
//! selection, anchors and references (by `anchor-<uid>` ids), and classes we do not know (kept by name as
//! unknown effects, which load as not computable).

use crate::model::{
    ColorSelectionEffect, EffectHeader, Levels as PainterLevels, PainterSource, Procedural as PainterProcedural,
    RawEntry, ReferenceSource, TweakValue,
};
use crate::translate::channel_table::channel_for_bit;
use materialcraft::json::Value;
use materialcraft::paint::BakedKind;
use materialcraft::paint::effect::{Anchor, ColorSelection, Levels, Procedural, Reference, UnknownEffect};
use std::collections::BTreeSet;

/// Prefix of the generator and filter ids of Substance procedurals (no implementation is registered for them;
/// a replacement table may map them to ours later).
pub const SUBSTANCE_PREFIX: &str = "substance:";

/// Prefix of the parameters that record what feeds an image input (`input:texture` = `ratio_dirt_1`).
pub const INPUT_PREFIX: &str = "input:";

/// Prefix of the parameters that record the fill's projection on an imported procedural (`placement:projection`
/// = `tri_planar`, the keys of `materialcraft::proc_textures::placement`), so a texture replacement can place
/// itself like the fill did. Prefixed so they never meet a Painter tweak of the same name.
pub const PLACEMENT_PREFIX: &str = "placement:";

/// `params` with the placement parameters of `projection` added (prefixed); none for texture space.
pub fn with_placement(
    mut params: Vec<(String, String)>,
    projection: &materialcraft::projection::Projection,
) -> Vec<(String, String)> {
    let placed = materialcraft::proc_textures::placement::params(projection);
    params.extend(placed.into_iter().map(|(k, v)| (format!("{PLACEMENT_PREFIX}{k}"), v)));
    params
}

/// A generator or filter id plus parameters for a Painter procedural; `?` when the effect names none. The
/// parameters are its tweaks, then one `input:<identifier>` per fed image input naming what feeds it (a
/// procedural's graph name, `anchor`, `bitmap`, `uniform`), so replacements can follow the wiring.
pub fn procedural(p: Option<&PainterProcedural>) -> Procedural {
    let Some(p) = p else {
        return Procedural::new(format!("{SUBSTANCE_PREFIX}?"));
    };
    let mut params: Vec<(String, String)> = p
        .tweaks
        .iter()
        .filter_map(|t| tweak_text(&t.value).map(|v| (t.identifier.clone(), v)))
        .collect();
    for input in &p.inputs {
        let Some(source) = input.source.as_deref() else {
            continue;
        };
        let feeds = match source {
            PainterSource::Procedural(inner) => {
                inner.procedural.as_ref().map_or("procedural", |i| i.name()).to_string()
            }
            PainterSource::Reference(_) => "anchor".into(),
            PainterSource::Bitmap(_) => "bitmap".into(),
            PainterSource::Uniform(_) => "uniform".into(),
            PainterSource::Other(raw) => raw.class.clone(),
        };
        for (identifier, _) in &input.identifiers {
            params.push((format!("{INPUT_PREFIX}{identifier}"), feeds.clone()));
        }
    }
    Procedural::new(format!("{SUBSTANCE_PREFIX}{}", p.name())).with_params(params)
}

/// A tweak value as text: numbers comma-separated, text as is; classes we do not know are left out.
fn tweak_text(v: &TweakValue) -> Option<String> {
    let join = |n: Vec<String>| n.join(",");
    Some(match v {
        TweakValue::Int(i) => i.to_string(),
        TweakValue::Int2(a) => join(a.iter().map(i32::to_string).collect()),
        TweakValue::Floats(f) => join(f.iter().map(f32::to_string).collect()),
        TweakValue::Text(t) => t.clone(),
        TweakValue::Other(_) => return None,
    })
}

pub fn levels(l: Option<&PainterLevels>) -> Levels {
    match l {
        Some(l) => Levels {
            input_min: l.input_minimum,
            input_max: l.input_maximum,
            gamma: l.gamma,
            output_min: l.output_minimum,
            output_max: l.output_maximum,
            clamp: l.clamp != 0,
        },
        None => Levels::default(),
    }
}

/// Painter picks colours from a bitmap, usually the baked ID map.
pub fn color_selection(c: &ColorSelectionEffect) -> ColorSelection {
    ColorSelection {
        map: BakedKind::Id,
        colors: c.colors.clone(),
        tolerance: c.tolerance,
        hardness: c.hardness,
    }
}

/// The anchor by id, the channel the first binding reads (none: the channel being evaluated) and the levels.
pub fn reference(r: &ReferenceSource) -> Reference {
    Reference {
        anchor: r.anchor_uid.map(anchor_id).unwrap_or_default(),
        channel: r.bindings.first().and_then(|(source, _)| channel_for_bit(*source)),
        levels: r.levels.as_ref().map(|l| levels(Some(l))).filter(|l| !l.is_identity()),
    }
}

fn anchor_id(uid: u64) -> String {
    format!("anchor-{uid}")
}

/// An effect or source class the model does not know, by name.
pub fn unknown(raw: &RawEntry) -> UnknownEffect {
    unknown_named(&format!("substance:{}", raw.class), Value::object())
}

/// An unknown effect of `kind` holding `fields`.
pub fn unknown_named(kind: &str, fields: Value) -> UnknownEffect {
    let mut data = Value::object().with("kind", kind);
    for (k, v) in fields.as_object().into_iter().flatten() {
        data.insert(k.clone(), v.clone());
    }
    UnknownEffect {
        kind: kind.to_string(),
        data,
    }
}

/// Anchor ids unique within a texture set: `anchor-<uid>`, `-2`, `-3` … on repeats, `anchor-n<k>` without uid.
#[derive(Default)]
pub struct AnchorIds(BTreeSet<String>);

impl AnchorIds {
    pub fn anchor(&mut self, header: &EffectHeader) -> Anchor {
        let base = header
            .uid
            .map_or_else(|| format!("anchor-n{}", self.0.len()), anchor_id);
        let id = (1..)
            .map(|n| if n == 1 { base.clone() } else { format!("{base}-{n}") })
            .find(|id| !self.0.contains(id))
            .unwrap_or(base);
        self.0.insert(id.clone());
        Anchor { id }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Tweak;

    #[test]
    fn procedurals_tweaks_and_anchor_ids() {
        let p = PainterProcedural {
            uid: None,
            url: "resource://shelf/mg_dirt/mg_dirt?version=1.sbsar".into(),
            tweaks: vec![
                Tweak {
                    identifier: "$outputsize".into(),
                    value: TweakValue::Int2([11, 11]),
                },
                Tweak {
                    identifier: "amount".into(),
                    value: TweakValue::Floats(vec![0.5]),
                },
                Tweak {
                    identifier: "odd".into(),
                    value: TweakValue::Other("DataTweakX".into()),
                },
            ],
            outputs: vec![],
            inputs: vec![],
        };
        let ours = procedural(Some(&p));
        assert_eq!(ours.id, "substance:mg_dirt");
        assert_eq!(
            ours.params,
            [
                ("$outputsize".to_string(), "11,11".to_string()),
                ("amount".into(), "0.5".into())
            ]
        );
        assert_eq!(procedural(None).id, "substance:?");
        let mut ids = AnchorIds::default();
        let h = EffectHeader {
            uid: Some(7),
            ..EffectHeader::default()
        };
        assert_eq!(ids.anchor(&h).id, "anchor-7");
        assert_eq!(ids.anchor(&h).id, "anchor-7-2");
        assert_eq!(ids.anchor(&EffectHeader::default()).id, "anchor-n2");
        let r = ReferenceSource {
            uid: None,
            channels: Default::default(),
            anchor_uid: Some(7),
            anchor_label: String::new(),
            levels: Some(PainterLevels {
                input_maximum: [1.0; 3],
                gamma: [1.0; 3],
                output_maximum: [1.0; 3],
                ..PainterLevels::default()
            }),
            bindings: vec![(2, 0)],
            mode: 2,
        };
        let ours = reference(&r);
        assert_eq!(ours.anchor, "anchor-7");
        assert_eq!(ours.channel, Some(materialcraft::paint::Channel::Height));
        assert_eq!(ours.levels, None);
        assert_eq!(
            unknown_named("substance:bitmap_fill", Value::object().with("url", "/a"))
                .data
                .to_string(),
            r#"{"kind":"substance:bitmap_fill","url":"/a"}"#
        );
    }
}

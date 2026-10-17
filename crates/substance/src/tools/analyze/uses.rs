//! [`ProceduralUse`]: every place a Painter project uses a Substance procedural in a layer or mask (fills,
//! filters and the procedurals plugged into their image inputs), with its parameter values as text. Brush
//! tips and stencils of strokes are not listed (strokes are not replayed yet).

use crate::model::{PainterDocument, PainterEffect, PainterLayer, PainterSource, Procedural};
use crate::translate::layers::effect_data;

/// Where a procedural is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    /// Layer content (a fill source).
    Fill,
    /// A fill in a mask (generators, grunges).
    MaskFill,
    Filter,
    MaskFilter,
    /// Plugged into an image input of another procedural.
    Input,
}

impl Role {
    pub fn name(self) -> &'static str {
        match self {
            Role::Fill => "fill",
            Role::MaskFill => "mask fill",
            Role::Filter => "filter",
            Role::MaskFilter => "mask filter",
            Role::Input => "input",
        }
    }
}

/// One use.
#[derive(Debug, Clone, PartialEq)]
pub struct ProceduralUse {
    /// The graph name (`mg_dirt`).
    pub name: String,
    pub role: Role,
    pub set: String,
    /// Folder path and layer name.
    pub layer: String,
    /// The layer and its folders are shown and the effect is switched on.
    pub visible: bool,
    /// Parameter values as the importer writes them.
    pub params: Vec<(String, String)>,
    /// Image inputs that are fed: input identifier and what feeds it (`anchor`, `bitmap`, a procedural's name …).
    pub inputs: Vec<(String, String)>,
}

/// Every use in the document, set by set, bottom → top.
pub fn collect(document: &PainterDocument) -> Vec<ProceduralUse> {
    let mut out = Vec::new();
    for set in &document.texture_sets {
        let mut walk = Walk {
            set: &set.name,
            out: &mut out,
        };
        walk.layers(&set.layers, "", true);
    }
    out
}

struct Walk<'a> {
    set: &'a str,
    out: &'a mut Vec<ProceduralUse>,
}

struct At<'p> {
    layer: &'p str,
    visible: bool,
}

impl Walk<'_> {
    fn layers(&mut self, layers: &[PainterLayer], path: &str, visible: bool) {
        for l in layers {
            let name = if path.is_empty() {
                l.name.clone()
            } else {
                format!("{path} / {}", l.name)
            };
            let shown = visible && l.visible;
            let at = At {
                layer: &name,
                visible: shown,
            };
            self.effects(&l.content, &at, false);
            if let Some(mask) = &l.mask {
                let at = At {
                    layer: &name,
                    visible: shown && mask.enabled,
                };
                self.effects(&mask.effects, &at, true);
            }
            self.layers(l.children(), &name, shown);
        }
    }

    fn effects(&mut self, effects: &[PainterEffect], at: &At, mask: bool) {
        for e in effects {
            let on = At {
                layer: at.layer,
                visible: at.visible && e.is_enabled(),
            };
            match e {
                PainterEffect::Fill(fill) => {
                    for source in &fill.sources {
                        if let PainterSource::Procedural(p) = source
                            && let Some(p) = &p.procedural
                        {
                            self.add(p, if mask { Role::MaskFill } else { Role::Fill }, &on);
                        }
                    }
                }
                PainterEffect::Filter(f) => {
                    if let Some(p) = &f.procedural {
                        self.add(p, if mask { Role::MaskFilter } else { Role::Filter }, &on);
                    }
                }
                PainterEffect::Group(g) => self.effects(&g.effects, &on, mask),
                _ => {}
            }
        }
    }

    fn add(&mut self, p: &Procedural, role: Role, at: &At) {
        let mut inputs = Vec::new();
        for input in &p.inputs {
            let feeds = match input.source.as_deref() {
                Some(PainterSource::Procedural(inner)) => match &inner.procedural {
                    Some(inner) => {
                        self.add(inner, Role::Input, at);
                        inner.name().to_string()
                    }
                    None => "procedural".to_string(),
                },
                Some(PainterSource::Reference(_)) => "anchor".to_string(),
                Some(PainterSource::Bitmap(_)) => "bitmap".to_string(),
                Some(PainterSource::Uniform(_)) => "uniform".to_string(),
                Some(PainterSource::Other(raw)) => raw.class.clone(),
                None => continue,
            };
            for (identifier, _) in &input.identifiers {
                inputs.push((identifier.clone(), feeds.clone()));
            }
        }
        self.out.push(ProceduralUse {
            name: p.name().to_string(),
            role,
            set: self.set.to_string(),
            layer: at.layer.to_string(),
            visible: at.visible,
            params: effect_data::procedural(Some(p))
                .params
                .into_iter()
                .filter(|(k, _)| !k.starts_with(effect_data::INPUT_PREFIX))
                .collect(),
            inputs,
        });
    }
}

//! Shader parameters: what a shader file declares ([`ParamDecl`], [`ParamKind`]) and the values an instance holds
//! ([`ParamValue`]).

/// The type, range and default of one declared parameter.
#[derive(Debug, Clone, PartialEq)]
pub enum ParamKind {
    /// A number in `min ..= max` (a slider); `step` is the arrow step, `None` for about a hundredth of the range.
    Float {
        min: f32,
        max: f32,
        default: f32,
        step: Option<f32>,
    },
    /// A whole number in `min ..= max`.
    Int {
        min: i32,
        max: i32,
        default: i32,
    },
    Bool {
        default: bool,
    },
    /// A linear RGB colour, every component in 0 … 1.
    Color {
        default: [f32; 3],
    },
    /// One of a list of named options; the shader gets the option's index.
    Choice {
        options: Vec<String>,
        default: usize,
    },
}

/// One parameter of a shader file: shown in the settings panel under `group`, sent to the GLSL `uniform`.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamDecl {
    /// Stable id the instance's values are keyed by.
    pub id: String,
    pub label: String,
    /// Panel section ("" = the first, unnamed section).
    pub group: String,
    /// The GLSL uniform that receives the value.
    pub uniform: String,
    pub kind: ParamKind,
}

/// A parameter's value in an instance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParamValue {
    Float(f32),
    /// Whole numbers and choice indices.
    Int(i32),
    Bool(bool),
    Color([f32; 3]),
}

impl ParamDecl {
    /// The declared default.
    pub fn default_value(&self) -> ParamValue {
        match &self.kind {
            ParamKind::Float { default, .. } => ParamValue::Float(*default),
            ParamKind::Int { default, .. } => ParamValue::Int(*default),
            ParamKind::Bool { default } => ParamValue::Bool(*default),
            ParamKind::Color { default } => ParamValue::Color(*default),
            ParamKind::Choice { default, .. } => ParamValue::Int(i32::try_from(*default).unwrap_or(0)),
        }
    }

    /// `value` made valid for this parameter: clamped into its range, or the default when it has the wrong type or
    /// is not finite.
    pub fn sanitize(&self, value: ParamValue) -> ParamValue {
        match (&self.kind, value) {
            (ParamKind::Float { min, max, default, .. }, ParamValue::Float(v)) => ParamValue::Float(if v.is_finite() { v.clamp(*min, *max) } else { *default }),
            (ParamKind::Int { min, max, .. }, ParamValue::Int(v)) => ParamValue::Int(v.clamp(*min, *max)),
            (ParamKind::Bool { .. }, ParamValue::Bool(b)) => ParamValue::Bool(b),
            (ParamKind::Color { default }, ParamValue::Color(c)) => {
                let mut out = *default;
                for (o, v) in out.iter_mut().zip(c) {
                    if v.is_finite() {
                        *o = v.clamp(0.0, 1.0);
                    }
                }
                ParamValue::Color(out)
            }
            (ParamKind::Choice { options, .. }, ParamValue::Int(i)) if usize::try_from(i).is_ok_and(|i| i < options.len()) => ParamValue::Int(i),
            _ => self.default_value(),
        }
    }
}

impl ParamValue {
    pub fn as_f32(self) -> Option<f32> {
        match self {
            ParamValue::Float(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_bool(self) -> Option<bool> {
        match self {
            ParamValue::Bool(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_int(self) -> Option<i32> {
        match self {
            ParamValue::Int(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_color(self) -> Option<[f32; 3]> {
        match self {
            ParamValue::Color(v) => Some(v),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decl(kind: ParamKind) -> ParamDecl {
        ParamDecl { id: "p".into(), label: "P".into(), group: String::new(), uniform: "uP".into(), kind }
    }

    #[test]
    fn values_are_clamped_or_replaced_by_the_default() {
        let f = decl(ParamKind::Float { min: 0.0, max: 2.0, default: 1.0, step: None });
        assert_eq!(f.sanitize(ParamValue::Float(3.0)), ParamValue::Float(2.0));
        assert_eq!(f.sanitize(ParamValue::Float(f32::NAN)), ParamValue::Float(1.0));
        assert_eq!(f.sanitize(ParamValue::Bool(true)), ParamValue::Float(1.0), "wrong type");
        let c = decl(ParamKind::Color { default: [0.5; 3] });
        assert_eq!(c.sanitize(ParamValue::Color([2.0, f32::INFINITY, -1.0])), ParamValue::Color([1.0, 0.5, 0.0]));
        let ch = decl(ParamKind::Choice { options: vec!["a".into(), "b".into()], default: 1 });
        assert_eq!(ch.default_value(), ParamValue::Int(1));
        assert_eq!(ch.sanitize(ParamValue::Int(5)), ParamValue::Int(1));
        assert_eq!(ch.sanitize(ParamValue::Int(0)), ParamValue::Int(0));
        let i = decl(ParamKind::Int { min: 1, max: 4, default: 2 });
        assert_eq!(i.sanitize(ParamValue::Int(9)), ParamValue::Int(4));
    }
}

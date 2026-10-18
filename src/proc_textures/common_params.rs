//! [`COMMON_PARAMS`]: the parameters every texture takes (see the module header), described for an editor: name,
//! label, kind with its range and default. [`ParamInfo::read`] and [`ParamInfo::text`] convert between a stored
//! value and the number an editor shows.

use crate::paint::effect::Params;

/// How a parameter is edited.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParamKind {
    /// A whole number in `min..=max`.
    Int { min: i32, max: i32 },
    /// A number in `min..=max`.
    Range { min: f32, max: f32 },
    /// On (1) or off (0).
    Toggle,
}

/// One parameter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParamInfo {
    pub name: &'static str,
    pub label: &'static str,
    pub kind: ParamKind,
    pub default: f32,
}

const fn p(name: &'static str, label: &'static str, kind: ParamKind, default: f32) -> ParamInfo {
    ParamInfo { name, label, kind, default }
}

/// The shared parameters, in the order an editor shows them.
pub static COMMON_PARAMS: &[ParamInfo] = &[
    p("scale", "Scale", ParamKind::Int { min: 1, max: 64 }, 1.0),
    p("seed", "Seed", ParamKind::Int { min: 0, max: 9999 }, 0.0),
    p("disorder", "Disorder", ParamKind::Range { min: 0.0, max: 1.0 }, 0.0),
    p("balance", "Balance", ParamKind::Range { min: 0.0, max: 1.0 }, 0.5),
    p("contrast", "Contrast", ParamKind::Range { min: 0.0, max: 1.0 }, 0.0),
    p("position", "Position", ParamKind::Range { min: 0.0, max: 1.0 }, 0.5),
    p("range", "Range", ParamKind::Range { min: 0.0, max: 1.0 }, 0.0),
    p("invert", "Invert", ParamKind::Toggle, 0.0),
];

impl ParamInfo {
    /// The value in `params` (the default when missing or unreadable), within the parameter's range.
    pub fn read(&self, params: &[(String, String)]) -> f32 {
        let p = Params::new(params);
        match self.kind {
            ParamKind::Int { min, max } => p.int(self.name, self.default as i32).clamp(min, max) as f32,
            ParamKind::Range { min, max } => p.range(self.name, self.default, min, max),
            ParamKind::Toggle => f32::from(u8::from(p.on(self.name, self.default != 0.0))),
        }
    }

    /// `value` as stored text (whole numbers without a fraction, others with at most 4 decimals).
    pub fn text(&self, value: f32) -> String {
        match self.kind {
            ParamKind::Int { min, max } => (value.round() as i32).clamp(min, max).to_string(),
            ParamKind::Toggle => if value != 0.0 { "1" } else { "0" }.to_string(),
            ParamKind::Range { min, max } => {
                let v = format!("{:.4}", value.clamp(min, max));
                v.trim_end_matches('0').trim_end_matches('.').to_string()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(name: &str) -> &'static ParamInfo {
        COMMON_PARAMS.iter().find(|p| p.name == name).unwrap()
    }

    #[test]
    fn values_read_and_write_within_their_range() {
        let params = vec![("scale".to_string(), "300".to_string()), ("balance".to_string(), "0.25".to_string())];
        assert_eq!(info("scale").read(&params), 64.0);
        assert_eq!(info("balance").read(&params), 0.25);
        assert_eq!(info("contrast").read(&params), 0.0, "default");
        assert_eq!(info("invert").read(&[("invert".into(), "2".into())]), 1.0);
        assert_eq!(info("scale").text(3.4), "3");
        assert_eq!(info("balance").text(0.5), "0.5");
        assert_eq!(info("balance").text(1.7), "1");
        assert_eq!(info("invert").text(1.0), "1");
    }
}

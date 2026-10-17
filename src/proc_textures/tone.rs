//! [`Tone`]: the value adjustments every texture shares (balance, contrast, a second scan, invert).

use crate::paint::effect::Params;

fn unit(v: f32) -> f32 {
    if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 }
}

/// Balance and contrast, then a scan around `position` with `range`, then invert.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tone {
    /// 0..1, 0.5 neutral; higher is brighter.
    pub balance: f32,
    /// 0..1, 0 neutral; 1 is a hard step.
    pub contrast: f32,
    /// 0..1, 0.5 neutral: the value that maps to mid-grey in the second scan.
    pub position: f32,
    /// 0..1, 0 neutral: the second scan's contrast.
    pub range: f32,
    pub invert: bool,
}

impl Default for Tone {
    fn default() -> Tone {
        Tone { balance: 0.5, contrast: 0.0, position: 0.5, range: 0.0, invert: false }
    }
}

impl Tone {
    pub fn read(p: &Params) -> Tone {
        Tone {
            balance: p.range("balance", 0.5, 0.0, 1.0),
            contrast: p.range("contrast", 0.0, 0.0, 1.0),
            position: p.range("position", 0.5, 0.0, 1.0),
            range: p.range("range", 0.0, 0.0, 1.0),
            invert: p.on("invert", false),
        }
    }

    /// The tone with balance and contrast left neutral (for looks that use them themselves).
    pub fn without_balance(self) -> Tone {
        Tone { balance: 0.5, contrast: 0.0, ..self }
    }

    /// True if [`Tone::apply`] returns its input unchanged.
    pub fn is_identity(&self) -> bool {
        *self == Tone::default()
    }

    pub fn apply(&self, v: f32) -> f32 {
        if self.is_identity() {
            return v;
        }
        let scan = |v: f32, centre: f32, contrast: f32| unit((v - centre) / (1.0 - contrast).max(0.005) + 0.5);
        let v = scan(v, 1.0 - self.balance, self.contrast);
        let v = scan(v, self.position, self.range);
        if self.invert { 1.0 - v } else { v }
    }
}

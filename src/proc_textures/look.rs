//! [`Look`]: the raw look of a texture, and [`ProcTexture`]: a look with its disorder warp and tone, evaluated
//! at any (u, v) or at projector taps.

use super::tone::Tone;
use crate::noise::Disorder;
use crate::paint::effect::Params;
use crate::projection::Tap;

/// A grey look, periodic with period 1 in u and v. `value` is called with u, v in 0..1.
pub trait Look: Send + Sync {
    /// 0..1.
    fn value(&self, u: f32, v: f32) -> f32;

    /// True if the look applies the `balance` / `contrast` parameters itself (the grunge maps threshold with
    /// them), so the shared tone leaves them out.
    fn own_balance(&self) -> bool {
        false
    }
}

/// A texture ready to evaluate.
pub struct ProcTexture {
    look: Box<dyn Look>,
    disorder: Option<Disorder>,
    tone: Tone,
}

impl ProcTexture {
    /// `look` with the shared parameters (`disorder`, the tone) read from `p`.
    pub fn new(look: Box<dyn Look>, p: &Params) -> ProcTexture {
        let tone = Tone::read(p);
        let tone = if look.own_balance() { tone.without_balance() } else { tone };
        let amount = p.range("disorder", 0.0, 0.0, 1.0);
        let disorder = (amount > 0.0).then(|| Disorder::new(p.seed("seed").wrapping_add(31), amount, 1.0, 0.0, 0.0));
        ProcTexture { look, disorder, tone }
    }

    /// `look` as it is: no disorder, neutral tone.
    pub fn plain(look: Box<dyn Look>) -> ProcTexture {
        ProcTexture { look, disorder: None, tone: Tone::default() }
    }

    /// The grey value 0..1 at (u, v); any u, v (the texture repeats).
    pub fn value(&self, u: f32, v: f32) -> f32 {
        let (u, v) = (u.rem_euclid(1.0), v.rem_euclid(1.0));
        let (u, v) = match &self.disorder {
            Some(d) => {
                let (a, b) = d.warp(u, v);
                (a.rem_euclid(1.0), b.rem_euclid(1.0))
            }
            None => (u, v),
        };
        let x = self.tone.apply(self.look.value(u, v));
        if x.is_finite() { x.clamp(0.0, 1.0) } else { 0.0 }
    }

    /// One grey RGBA value (full coverage) per tap, as the paint model's fill sources write them.
    pub fn sample_taps(&self, taps: &[Tap], out: &mut [[f32; 4]]) {
        for (tap, o) in taps.iter().zip(out.iter_mut()) {
            let x = self.value(tap.uv[0], tap.uv[1]);
            *o = [x, x, x, 1.0];
        }
    }
}

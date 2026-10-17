//! The numbers behind the slider row, without any UI so they can be tested: modifier ratios,
//! arrow steps, drags, the jump to the pointer and typed input (SL-02…SL-06, ADR 0008 VQ2).
//!
//! Modifiers mean the same everywhere: **Shift = finer** (a tenth), **Ctrl/Cmd = coarser** (ten
//! times). Ctrl on Windows and Linux and Cmd on macOS are one modifier (egui's `command`).

/// The modifier keys that change a slider's step, already mapped from the platform's keys.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StepMods {
    /// Shift: a tenth of the step and of the drag speed.
    pub fine: bool,
    /// Ctrl/Cmd: ten times the step; a drag moves in those coarse steps.
    pub coarse: bool,
}

impl StepMods {
    pub const NONE: StepMods = StepMods { fine: false, coarse: false };
    #[cfg(test)]
    pub const FINE: StepMods = StepMods { fine: true, coarse: false };
    #[cfg(test)]
    pub const COARSE: StepMods = StepMods { fine: false, coarse: true };

    /// Shift wins when both are held: asking for precision is the safer reading.
    pub fn ratio(self) -> f32 {
        if self.fine {
            0.1
        } else if self.coarse {
            10.0
        } else {
            1.0
        }
    }
}

impl From<eframe::egui::Modifiers> for StepMods {
    fn from(m: eframe::egui::Modifiers) -> StepMods {
        StepMods { fine: m.shift, coarse: m.command }
    }
}

/// A value range that is always usable: reversed bounds are swapped, non-finite ones replaced.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    pub lo: f32,
    pub hi: f32,
}

impl Span {
    pub fn new(a: f32, b: f32) -> Span {
        let a = if a.is_finite() { a } else { 0.0 };
        let b = if b.is_finite() { b } else { 1.0 };
        if a <= b { Span { lo: a, hi: b } } else { Span { lo: b, hi: a } }
    }

    pub fn width(self) -> f32 {
        self.hi - self.lo
    }

    /// `v` inside the range; NaN becomes the lower bound.
    pub fn clamp(self, v: f32) -> f32 {
        if v.is_nan() { self.lo } else { v.clamp(self.lo, self.hi) }
    }

    /// Where `v` lies between the bounds, 0…1.
    pub fn fraction(self, v: f32) -> f32 {
        if self.width() <= 0.0 { 0.0 } else { ((self.clamp(v) - self.lo) / self.width()).clamp(0.0, 1.0) }
    }

    /// A step of about a hundredth of the range, rounded to 1, 2 or 5 times a power of ten
    /// (0…1 → 0.01, 0…12 → 0.1, 0…360 → 5).
    pub fn default_step(self) -> f32 {
        let raw = self.width() / 100.0;
        if !(raw.is_finite() && raw > 0.0) {
            return 1.0;
        }
        let pow = 10f32.powf(raw.log10().floor());
        let m = raw / pow;
        let nice = if m < 1.5 {
            1.0
        } else if m < 3.5 {
            2.0
        } else if m < 7.5 {
            5.0
        } else {
            10.0
        };
        nice * pow
    }
}

/// Removes float noise (0.1 + 0.2) from a value that sits on a step grid.
fn tidy(v: f32, step: f32) -> f32 {
    if !(step.is_finite() && step > 0.0) {
        return v;
    }
    let q = (v / step).round() * step;
    // Keep the digits the step has, plus two.
    let digits = (-step.log10().floor()).max(0.0) + 2.0;
    let k = 10f32.powf(digits.min(7.0));
    (q * k).round() / k
}

/// The value after `presses` arrow presses (Up positive), each worth `step` times the modifier
/// ratio. The value first snaps to that step's grid, so 55.4 plus one step of 1 gives 56 (SL-05).
pub fn arrow(value: f32, step: f32, presses: i32, mods: StepMods, span: Span) -> f32 {
    let step = if step.is_finite() && step > 0.0 { step } else { span.default_step() };
    let k = step * mods.ratio();
    if presses == 0 {
        return span.clamp(value);
    }
    let on_grid = (value / k).round() * k;
    let base = if presses > 0 && on_grid > value + k * 1e-3 || presses < 0 && on_grid < value - k * 1e-3 {
        // Snapping already moved one way; count that as the first press.
        on_grid - k * presses.signum() as f32
    } else {
        on_grid
    };
    span.clamp(tidy(base + k * presses as f32, k))
}

/// The value under the pointer at `x` on a track from `left` that is `width` wide (SL-03).
pub fn jump(x: f32, left: f32, width: f32, span: Span) -> f32 {
    if !(width.is_finite() && width > 0.0 && x.is_finite()) {
        return span.lo;
    }
    let f = ((x - left) / width).clamp(0.0, 1.0);
    span.lo + f * span.width()
}

/// Where a drag segment started: the pointer position and the value then. A new segment starts
/// whenever the modifiers change, so switching to Shift mid-drag never makes the value jump.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DragAnchor {
    pub x: f32,
    pub value: f32,
    pub mods: StepMods,
}

/// The value while dragging with the pointer at `x` (SL-02, SL-04). Without modifiers the value
/// follows the pointer one to one along the track; Shift moves it ten times slower; Ctrl/Cmd moves
/// it at the normal speed but only in coarse steps (ten times `step`).
pub fn drag(anchor: DragAnchor, x: f32, width: f32, step: f32, span: Span) -> f32 {
    if !(width.is_finite() && width > 0.0 && x.is_finite()) {
        return span.clamp(anchor.value);
    }
    let speed = if anchor.mods.fine { 0.1 } else { 1.0 };
    let raw = anchor.value + (x - anchor.x) / width * span.width() * speed;
    let v = if anchor.mods.coarse && !anchor.mods.fine {
        let step = if step.is_finite() && step > 0.0 { step } else { span.default_step() };
        tidy(raw, step * 10.0)
    } else {
        raw
    };
    span.clamp(v)
}

/// Removes everything a number can't contain from typed text (SL-06: non-numeric input is
/// refused): digits, one leading sign, one decimal separator (`.` or `,`) and an exponent.
pub fn filter_typed(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut seen_sep = false;
    let mut seen_exp = false;
    for c in text.chars().take(32) {
        let last = out.chars().last();
        let ok = match c {
            '0'..='9' => true,
            '-' | '+' | '−' => out.is_empty() || matches!(last, Some('e' | 'E')),
            '.' | ',' => !seen_sep && !seen_exp,
            'e' | 'E' => !seen_exp && last.is_some_and(|l| l.is_ascii_digit()),
            _ => false,
        };
        if ok {
            seen_sep |= matches!(c, '.' | ',');
            seen_exp |= matches!(c, 'e' | 'E');
            out.push(if c == '−' { '-' } else { c });
        }
    }
    out
}

/// The number in typed text, clamped to the range; `None` while it is not a number yet
/// (empty, a lone sign, `1e`).
pub fn parse_typed(text: &str, span: Span) -> Option<f32> {
    let t = filter_typed(text).replace(',', ".");
    let v: f32 = t.parse().ok()?;
    v.is_finite().then(|| span.clamp(v))
}

/// The value as the field shows it: `decimals` places, trailing zeros dropped ("0.5", "12", "-3").
pub fn format(v: f32, decimals: usize) -> String {
    let s = format!("{:.*}", decimals.min(6), v);
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_owned() } else { s };
    if s == "-0" { "0".to_owned() } else { s }
}

/// Places to show for a step: enough that one step changes the text (0.01 → 2, 5 → 0).
pub fn decimals_for(step: f32) -> usize {
    if !(step.is_finite() && step > 0.0) || step >= 1.0 {
        return 0;
    }
    (-step.log10().floor()) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    const UNIT: Span = Span { lo: 0.0, hi: 1.0 };
    const PERCENT: Span = Span { lo: 0.0, hi: 100.0 };

    #[test]
    fn shift_is_finer_and_ctrl_or_cmd_coarser() {
        assert_eq!(StepMods::NONE.ratio(), 1.0);
        assert_eq!(StepMods::FINE.ratio(), 0.1);
        assert_eq!(StepMods::COARSE.ratio(), 10.0);
        assert_eq!(StepMods { fine: true, coarse: true }.ratio(), 0.1, "Shift wins");
        // The platform mapping: egui's `command` is Ctrl on Windows/Linux and Cmd on macOS.
        let m = eframe::egui::Modifiers { ctrl: true, command: true, ..Default::default() };
        assert_eq!(StepMods::from(m), StepMods::COARSE);
        let m = eframe::egui::Modifiers { mac_cmd: true, command: true, ..Default::default() };
        assert_eq!(StepMods::from(m), StepMods::COARSE);
        assert_eq!(StepMods::from(eframe::egui::Modifiers::SHIFT), StepMods::FINE);
    }

    #[test]
    fn arrows_step_by_the_step_a_tenth_with_shift_and_ten_times_with_ctrl() {
        assert_eq!(arrow(50.0, 1.0, 1, StepMods::NONE, PERCENT), 51.0);
        assert_eq!(arrow(50.0, 1.0, -1, StepMods::NONE, PERCENT), 49.0);
        assert_eq!(arrow(50.0, 1.0, 1, StepMods::FINE, PERCENT), 50.1);
        assert_eq!(arrow(50.0, 1.0, 1, StepMods::COARSE, PERCENT), 60.0);
        assert_eq!(arrow(50.0, 1.0, 3, StepMods::FINE, PERCENT), 50.3);
        assert_eq!(arrow(0.5, 0.01, 1, StepMods::NONE, UNIT), 0.51);
        assert_eq!(arrow(0.5, 0.01, 1, StepMods::FINE, UNIT), 0.501);
        assert_eq!(arrow(0.5, 0.01, 1, StepMods::COARSE, UNIT), 0.6);
    }

    #[test]
    fn arrows_snap_to_the_grid_and_stay_in_range() {
        assert_eq!(arrow(55.4, 1.0, 1, StepMods::NONE, PERCENT), 56.0);
        assert_eq!(arrow(55.4, 1.0, -1, StepMods::NONE, PERCENT), 55.0);
        assert_eq!(arrow(55.6, 1.0, 1, StepMods::NONE, PERCENT), 56.0);
        assert_eq!(arrow(99.5, 1.0, 5, StepMods::NONE, PERCENT), 100.0);
        assert_eq!(arrow(0.0, 1.0, -1, StepMods::COARSE, PERCENT), 0.0);
        // Ten fine steps of 0.1 land exactly on the next whole step, without float noise.
        let mut v = 10.0;
        for _ in 0..10 {
            v = arrow(v, 1.0, 1, StepMods::FINE, PERCENT);
        }
        assert_eq!(v, 11.0);
    }

    #[test]
    fn a_plain_drag_follows_the_pointer_and_shift_moves_ten_times_slower() {
        let a = DragAnchor { x: 100.0, value: 50.0, mods: StepMods::NONE };
        // A 200-point track over 0…100: 2 points per unit.
        assert_eq!(drag(a, 120.0, 200.0, 1.0, PERCENT), 60.0);
        let fine = DragAnchor { mods: StepMods::FINE, ..a };
        assert!((drag(fine, 120.0, 200.0, 1.0, PERCENT) - 51.0).abs() < 1e-4);
        assert_eq!(drag(a, 10_000.0, 200.0, 1.0, PERCENT), 100.0, "clamped");
    }

    #[test]
    fn a_ctrl_drag_moves_in_coarse_steps() {
        let a = DragAnchor { x: 0.0, value: 50.0, mods: StepMods::COARSE };
        assert_eq!(drag(a, 7.0, 200.0, 1.0, PERCENT), 50.0, "3.5 units round to the 10-grid");
        assert_eq!(drag(a, 12.0, 200.0, 1.0, PERCENT), 60.0);
        assert_eq!(drag(a, -32.0, 200.0, 1.0, PERCENT), 30.0);
    }

    #[test]
    fn a_click_jumps_to_the_value_under_the_pointer() {
        assert_eq!(jump(150.0, 100.0, 200.0, PERCENT), 25.0);
        assert_eq!(jump(-5.0, 100.0, 200.0, PERCENT), 0.0);
        assert_eq!(jump(f32::NAN, 0.0, 200.0, PERCENT), 0.0);
        assert_eq!(jump(10.0, 0.0, 0.0, PERCENT), 0.0);
    }

    #[test]
    fn typing_refuses_what_is_not_a_number_and_moves_the_value_as_typed() {
        assert_eq!(filter_typed("12a.5x"), "12.5");
        assert_eq!(filter_typed("--3"), "-3");
        assert_eq!(filter_typed("1.2.3"), "1.23");
        assert_eq!(filter_typed("0,5"), "0,5");
        assert_eq!(filter_typed("1e-3"), "1e-3");
        assert_eq!(filter_typed("−2"), "-2");
        assert_eq!(parse_typed("4", PERCENT), Some(4.0));
        assert_eq!(parse_typed("45", PERCENT), Some(45.0));
        assert_eq!(parse_typed("0,5", UNIT), Some(0.5));
        assert_eq!(parse_typed("250", PERCENT), Some(100.0), "clamped");
        assert_eq!(parse_typed("-", PERCENT), None);
        assert_eq!(parse_typed("", PERCENT), None);
        assert_eq!(parse_typed("abc", PERCENT), None);
    }

    #[test]
    fn steps_and_formats_are_sensible() {
        assert_eq!(UNIT.default_step(), 0.01);
        assert_eq!(PERCENT.default_step(), 1.0);
        assert_eq!(Span::new(0.0, 360.0).default_step(), 5.0);
        assert_eq!(Span::new(0.0, 0.0).default_step(), 1.0);
        assert_eq!(Span::new(5.0, 1.0), Span { lo: 1.0, hi: 5.0 });
        assert_eq!(Span::new(f32::NAN, f32::INFINITY), UNIT);
        assert_eq!(format(0.5, 2), "0.5");
        assert_eq!(format(12.0, 2), "12");
        assert_eq!(format(-0.0001, 2), "0");
        assert_eq!(decimals_for(0.01), 2);
        assert_eq!(decimals_for(5.0), 0);
    }
}

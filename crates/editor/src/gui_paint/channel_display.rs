//! [`ChannelDisplay`]: how single-channel and mesh-map views show their values (DS-07, DS-08): one component
//! isolated or all of them, high-range values divided by a scale, and signed values in two chosen colours with zero
//! black. Display only: it changes what the view uploads, never the document.

use materialcraft::tex::Tex;

/// Which components of the shown values are displayed (DS-08).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Component {
    /// Colour, or grey for scalar channels.
    #[default]
    All,
    Red,
    Green,
    Blue,
    Alpha,
}

impl Component {
    pub const ALL: [(Component, &'static str); 5] =
        [(Component::All, "All"), (Component::Red, "Red"), (Component::Green, "Green"), (Component::Blue, "Blue"), (Component::Alpha, "Alpha")];

    fn index(self) -> Option<usize> {
        match self {
            Component::All => None,
            Component::Red => Some(0),
            Component::Green => Some(1),
            Component::Blue => Some(2),
            Component::Alpha => Some(3),
        }
    }
}

/// How the shown values encode signed numbers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Encoding {
    /// The values are the numbers (height: zero is the surface).
    Plain,
    /// 0 … 1 stands for −1 … 1 (normal maps).
    Biased,
}

/// The display options of channel and mesh-map views.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ChannelDisplay {
    pub component: Component,
    /// Values are divided by this before they are shown (DS-07: height and displacement beyond 0 … 1).
    pub scale: f32,
    /// Show signed values in two colours, zero black (DS-07). Needs a single value: one isolated component, or a
    /// scalar channel.
    pub signed: bool,
    /// The colour of −1 and of +1 (display values).
    pub negative: [f32; 3],
    pub positive: [f32; 3],
}

impl Default for ChannelDisplay {
    fn default() -> ChannelDisplay {
        ChannelDisplay { component: Component::All, scale: 1.0, signed: false, negative: [0.15, 0.45, 1.0], positive: [1.0, 0.55, 0.1] }
    }
}

/// Scales the user can pick (DS-07).
pub const SCALES: [(f32, &str); 6] = [(1.0, "1"), (2.0, "2"), (4.0, "4"), (10.0, "10"), (100.0, "100"), (0.5, "0.5")];

impl ChannelDisplay {
    /// The scale as a usable divisor (1 for a broken value).
    fn divisor(&self) -> f32 {
        if self.scale.is_finite() && self.scale > 0.0 { self.scale } else { 1.0 }
    }

    /// True if signed display applies to values of a channel with `color` components.
    pub fn shows_signed(&self, color: bool) -> bool {
        self.signed && (!color || self.component != Component::All)
    }

    /// `source`'s values as opaque display values. `color` says whether the source is a colour channel (three
    /// components; scalar channels are grey); `encoding` how it stores signed numbers.
    pub fn apply(&self, source: &Tex, color: bool, encoding: Encoding) -> Tex {
        let (k, signed) = (1.0 / self.divisor(), self.shows_signed(color));
        let (neg, pos) = (self.negative, self.positive);
        let pick = self.component.index();
        source.map(|p| {
            let decode = |v: f32| match encoding {
                Encoding::Plain => v,
                Encoding::Biased => v * 2.0 - 1.0,
            };
            if signed {
                // one value: the isolated component, or the grey channel's first
                let v = decode(p[pick.unwrap_or(0).min(3)]) * k;
                let (c, m) = if v < 0.0 { (neg, -v) } else { (pos, v) };
                let m = m.min(1.0);
                return [c[0] * m, c[1] * m, c[2] * m, 1.0];
            }
            match pick {
                Some(i) => {
                    let v = p[i.min(3)] * k;
                    [v, v, v, 1.0]
                }
                None => [p[0] * k, p[1] * k, p[2] * k, 1.0],
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(p: [f32; 4]) -> Tex {
        Tex::solid(1, 1, p)
    }

    #[test]
    fn components_are_isolated_as_grey() {
        let src = one([0.2, 0.4, 0.6, 0.8]);
        let show = |component| ChannelDisplay { component, ..ChannelDisplay::default() }.apply(&src, true, Encoding::Plain).px[0];
        assert_eq!(show(Component::All), [0.2, 0.4, 0.6, 1.0]);
        assert_eq!(show(Component::Green), [0.4, 0.4, 0.4, 1.0]);
        assert_eq!(show(Component::Alpha), [0.8, 0.8, 0.8, 1.0]);
    }

    #[test]
    fn high_range_values_are_divided_by_the_scale() {
        let d = ChannelDisplay { scale: 4.0, ..ChannelDisplay::default() };
        assert_eq!(d.apply(&one([2.0, 2.0, 2.0, 1.0]), false, Encoding::Plain).px[0], [0.5, 0.5, 0.5, 1.0]);
        let broken = ChannelDisplay { scale: f32::NAN, ..ChannelDisplay::default() };
        assert_eq!(broken.apply(&one([0.25; 4]), false, Encoding::Plain).px[0], [0.25, 0.25, 0.25, 1.0]);
    }

    #[test]
    fn signed_values_take_two_colours_with_zero_black() {
        let d = ChannelDisplay { signed: true, negative: [0.0, 0.0, 1.0], positive: [1.0, 0.0, 0.0], ..ChannelDisplay::default() };
        // height: plain numbers
        let px = |v: f32, enc| d.apply(&one([v, v, v, 1.0]), false, enc).px[0];
        assert_eq!(px(-0.5, Encoding::Plain), [0.0, 0.0, 0.5, 1.0]);
        assert_eq!(px(0.0, Encoding::Plain), [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(px(3.0, Encoding::Plain), [1.0, 0.0, 0.0, 1.0], "clipped at full colour");
        // a normal map's green: 0.25 encodes −0.5
        let n = ChannelDisplay { component: Component::Green, ..d };
        assert_eq!(n.apply(&one([0.5, 0.25, 1.0, 1.0]), true, Encoding::Biased).px[0], [0.0, 0.0, 0.5, 1.0]);
        // all components of a colour channel: no single value, shown as they are
        assert!(!d.shows_signed(true) && d.shows_signed(false) && n.shows_signed(true));
        assert_eq!(d.apply(&one([0.5, 0.25, 1.0, 1.0]), true, Encoding::Biased).px[0], [0.5, 0.25, 1.0, 1.0]);
    }
}

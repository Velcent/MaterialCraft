//! [`Look`]: what a material looks like in numbers, per channel: the mean (colour or grey value) and a small
//! histogram (base colour: of its luminance; height: of the deviation from its mean, since heights have no
//! common zero), from weighted samples in the channel's stored encoding. [`Look::similarity`] compares two.
//!
//! The same statistics are taken from Painter's saved result where a material fill shows and from our
//! materials rendered small, so a tray material can be ranked by how close its look is, without evaluating
//! anything of the Substance material.

use materialcraft::paint::Channel;

/// Histogram bins per channel.
pub const BINS: usize = 16;

/// The channels compared, with their weight in the overall similarity.
pub const COMPARED: [(Channel, f32); 4] = [
    (Channel::BaseColor, 0.5),
    (Channel::Roughness, 0.25),
    (Channel::Metallic, 0.15),
    (Channel::Height, 0.1),
];

/// Range of height deviations the histogram covers (± this around the mean).
const HEIGHT_SPAN: f32 = 0.25;

/// One channel's statistics.
#[derive(Debug, Clone, PartialEq)]
pub struct ChannelLook {
    pub channel: Channel,
    /// Mean value (RGB; grey channels repeat it).
    pub mean: [f32; 3],
    /// Normalised histogram (sums to 1).
    pub histogram: [f32; BINS],
    /// Sum of the sample weights.
    pub weight: f32,
}

impl ChannelLook {
    /// Similarity 0..1: half from the means (a difference of 0.5 or more gives 0), half from the histograms'
    /// overlap. Heights compare only the shape of their histograms.
    pub fn similarity(&self, other: &ChannelLook) -> f32 {
        let overlap: f32 = self
            .histogram
            .iter()
            .zip(&other.histogram)
            .map(|(a, b)| a.min(*b))
            .sum();
        if self.channel == Channel::Height {
            return overlap.clamp(0.0, 1.0);
        }
        let d = self
            .mean
            .iter()
            .zip(&other.mean)
            .map(|(a, b)| (a - b) * (a - b))
            .sum::<f32>();
        let d = match self.channel.is_color() {
            true => (d / 3.0).sqrt(),
            false => d.sqrt() / 3f32.sqrt(),
        };
        (0.5 * (1.0 - (2.0 * d).min(1.0)) + 0.5 * overlap).clamp(0.0, 1.0)
    }
}

/// Every compared channel a source had samples for.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Look {
    pub channels: Vec<ChannelLook>,
}

/// How close two looks are: per channel both have, and overall (weighted by [`COMPARED`]).
#[derive(Debug, Clone, PartialEq)]
pub struct LookSimilarity {
    pub channels: Vec<(Channel, f32)>,
    pub overall: f32,
}

impl Look {
    pub fn channel(&self, channel: Channel) -> Option<&ChannelLook> {
        self.channels.iter().find(|c| c.channel == channel)
    }

    /// `None` when the two share no compared channel.
    pub fn similarity(&self, other: &Look) -> Option<LookSimilarity> {
        let mut channels = Vec::new();
        let (mut sum, mut weights) = (0.0, 0.0);
        for (channel, w) in COMPARED {
            if let (Some(a), Some(b)) = (self.channel(channel), other.channel(channel)) {
                let s = a.similarity(b);
                channels.push((channel, s));
                sum += s * w;
                weights += w;
            }
        }
        (weights > 0.0).then(|| LookSimilarity {
            channels,
            overall: sum / weights,
        })
    }
}

/// Collects weighted samples into a [`Look`].
#[derive(Debug, Clone, Default)]
pub struct LookBuilder {
    channels: Vec<Accumulator>,
}

#[derive(Debug, Clone)]
struct Accumulator {
    channel: Channel,
    sum: [f64; 3],
    weight: f64,
    /// Kept for the histogram (heights need the mean first).
    samples: Vec<(f32, f32)>,
}

impl LookBuilder {
    /// Adds one sample of `channel` (stored encoding; grey channels use the first component) with `weight`.
    /// Channels that are not compared and non-finite values are ignored.
    pub fn add(&mut self, channel: Channel, value: [f32; 4], weight: f32) {
        if !COMPARED.iter().any(|(c, _)| *c == channel)
            || weight <= 0.0
            || !weight.is_finite()
            || !value.iter().take(3).all(|v| v.is_finite())
        {
            return;
        }
        let rgb = match channel.is_color() {
            true => [value[0], value[1], value[2]],
            false => [value[0]; 3],
        };
        let i = match self.channels.iter().position(|a| a.channel == channel) {
            Some(i) => i,
            None => {
                self.channels.push(Accumulator {
                    channel,
                    sum: [0.0; 3],
                    weight: 0.0,
                    samples: Vec::new(),
                });
                self.channels.len() - 1
            }
        };
        let Some(a) = self.channels.get_mut(i) else { return };
        for (s, v) in a.sum.iter_mut().zip(rgb) {
            *s += f64::from(v * weight);
        }
        a.weight += f64::from(weight);
        let key = match channel.is_color() {
            true => 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2],
            false => rgb[0],
        };
        a.samples.push((key, weight));
    }

    pub fn is_empty(&self) -> bool {
        self.channels.is_empty()
    }

    pub fn build(self) -> Look {
        let channels = self
            .channels
            .into_iter()
            .filter(|a| a.weight > 0.0)
            .map(finish)
            .collect();
        Look { channels }
    }
}

fn finish(a: Accumulator) -> ChannelLook {
    let mean = a.sum.map(|s| (s / a.weight) as f32);
    let centre = if a.channel == Channel::Height { mean[0] } else { 0.0 };
    let mut histogram = [0.0f32; BINS];
    for (v, w) in &a.samples {
        let x = match a.channel == Channel::Height {
            true => (v - centre) / (2.0 * HEIGHT_SPAN) + 0.5,
            false => *v,
        };
        let bin = ((x.clamp(0.0, 1.0) * BINS as f32) as usize).min(BINS - 1);
        if let Some(h) = histogram.get_mut(bin) {
            *h += *w;
        }
    }
    let total = a.weight as f32;
    histogram.iter_mut().for_each(|h| *h /= total);
    ChannelLook {
        channel: a.channel,
        mean,
        histogram,
        weight: total,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn look(base: [f32; 3], rough: &[f32]) -> Look {
        let mut b = LookBuilder::default();
        for r in rough {
            b.add(Channel::BaseColor, [base[0], base[1], base[2], 1.0], 1.0);
            b.add(Channel::Roughness, [*r, *r, *r, 1.0], 1.0);
        }
        b.build()
    }

    #[test]
    fn statistics_are_weighted_means_and_normalised_histograms() {
        let mut b = LookBuilder::default();
        b.add(Channel::Roughness, [0.0; 4], 1.0);
        b.add(Channel::Roughness, [1.0; 4], 3.0);
        b.add(Channel::Normal, [0.5; 4], 1.0);
        b.add(Channel::Roughness, [f32::NAN; 4], 1.0);
        let l = b.build();
        assert_eq!(l.channels.len(), 1, "normals are not compared");
        let r = l.channel(Channel::Roughness).unwrap();
        assert!((r.mean[0] - 0.75).abs() < 1e-6);
        assert!((r.histogram[0] - 0.25).abs() < 1e-6 && (r.histogram[BINS - 1] - 0.75).abs() < 1e-6);
        assert!((r.histogram.iter().sum::<f32>() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn similar_looks_score_higher_than_different_ones() {
        let rust = look([0.45, 0.2, 0.1], &[0.7, 0.8, 0.9]);
        let rusty = look([0.5, 0.25, 0.12], &[0.75, 0.8, 0.85]);
        let chrome = look([0.9, 0.9, 0.9], &[0.05, 0.1, 0.05]);
        let same = rust.similarity(&rust).unwrap();
        assert!((same.overall - 1.0).abs() < 1e-5);
        let near = rust.similarity(&rusty).unwrap().overall;
        let far = rust.similarity(&chrome).unwrap().overall;
        assert!(near > 0.5 && far < 0.3, "{near} {far}");
        assert_eq!(rust.similarity(&rusty).unwrap().channels.len(), 2);
        assert!(rust.similarity(&Look::default()).is_none());
    }

    #[test]
    fn heights_compare_their_shape_not_their_level() {
        let heights = |level: f32| {
            let mut b = LookBuilder::default();
            for d in [-0.1, 0.0, 0.1] {
                b.add(Channel::Height, [level + d; 4], 1.0);
            }
            b.build()
        };
        let s = heights(0.2).similarity(&heights(0.7)).unwrap();
        assert!((s.overall - 1.0).abs() < 1e-5, "{s:?}");
    }
}

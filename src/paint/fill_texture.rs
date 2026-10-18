//! [`TextureFill`]: a fill whose channels come from procedural textures, laid on by the fill's projection, with
//! constant values for the channels without a texture (the way [`super::ImageFill`] has images and constants).
//!
//! A texture is a [`Procedural`] (id and parameters) naming a generator of the [`super::EffectRegistry`] that
//! evaluates at points ([`super::effect::GeneratorSource::sample_points`]): the paint model names no concrete
//! texture. Its grey value is the channel's value in the channel's stored encoding.

use super::channel::Channel;
use super::effect::Procedural;

/// Textures per channel, and constants (normalised RGBA in the stored encoding, as a uniform fill's) for the
/// channels without one. A channel is in at most one of the two lists.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextureFill {
    textures: Vec<(Channel, Procedural)>,
    values: Vec<(Channel, [f32; 4])>,
}

impl TextureFill {
    /// `texture` on every channel of `channels`.
    pub fn on(channels: &[Channel], texture: &Procedural) -> TextureFill {
        let mut fill = TextureFill::default();
        for c in channels {
            fill.set_texture(*c, texture.clone());
        }
        fill
    }

    pub fn textures(&self) -> &[(Channel, Procedural)] {
        &self.textures
    }

    pub fn values(&self) -> &[(Channel, [f32; 4])] {
        &self.values
    }

    pub fn texture(&self, channel: Channel) -> Option<&Procedural> {
        self.textures.iter().find(|(c, _)| *c == channel).map(|(_, t)| t)
    }

    pub fn texture_mut(&mut self, channel: Channel) -> Option<&mut Procedural> {
        self.textures.iter_mut().find(|(c, _)| *c == channel).map(|(_, t)| t)
    }

    pub fn value(&self, channel: Channel) -> Option<[f32; 4]> {
        self.values.iter().find(|(c, _)| *c == channel).map(|(_, v)| *v)
    }

    /// Sets `channel` to `texture` (replacing a texture or a constant).
    pub fn set_texture(&mut self, channel: Channel, texture: Procedural) {
        self.values.retain(|(c, _)| *c != channel);
        match self.texture_mut(channel) {
            Some(slot) => *slot = texture,
            None => self.textures.push((channel, texture)),
        }
    }

    /// Sets `channel` to a constant (replacing a texture or a constant).
    pub fn set_value(&mut self, channel: Channel, value: [f32; 4]) {
        self.textures.retain(|(c, _)| *c != channel);
        match self.values.iter_mut().find(|(c, _)| *c == channel) {
            Some(slot) => slot.1 = value,
            None => self.values.push((channel, value)),
        }
    }

    /// Leaves `channel` out of the fill.
    pub fn remove(&mut self, channel: Channel) {
        self.textures.retain(|(c, _)| *c != channel);
        self.values.retain(|(c, _)| *c != channel);
    }

    /// Keeps only the channels `keep` accepts.
    pub fn retain(&mut self, keep: impl Fn(Channel) -> bool) {
        self.textures.retain(|(c, _)| keep(*c));
        self.values.retain(|(c, _)| keep(*c));
    }

    /// Every channel the fill sets: textures first, then constants.
    pub fn channels(&self) -> Vec<Channel> {
        self.textures.iter().map(|(c, _)| *c).chain(self.values.iter().map(|(c, _)| *c)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_channel_has_a_texture_or_a_value() {
        let perlin = Procedural::new("perlin").with_params(vec![("scale".into(), "2".into())]);
        let mut f = TextureFill::on(&[Channel::Roughness, Channel::Height], &perlin);
        assert_eq!(f.channels(), vec![Channel::Roughness, Channel::Height]);
        f.set_value(Channel::Height, [0.5, 0.5, 0.5, 1.0]);
        assert_eq!(f.texture(Channel::Height), None);
        assert_eq!(f.value(Channel::Height), Some([0.5, 0.5, 0.5, 1.0]));
        f.set_texture(Channel::Height, Procedural::new("checker"));
        assert_eq!(f.value(Channel::Height), None);
        assert_eq!(f.texture(Channel::Height).map(|t| t.id.as_str()), Some("checker"));
        f.set_value(Channel::BaseColor, [1.0; 4]);
        f.retain(|c| c != Channel::Roughness);
        assert_eq!(f.channels(), vec![Channel::Height, Channel::BaseColor]);
        f.remove(Channel::Height);
        assert_eq!(f.channels(), vec![Channel::BaseColor]);
    }
}

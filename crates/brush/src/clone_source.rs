//! [`CloneSource`]: where the clone tool copies from (PT-27, PT-28). The user marks a source point on the model; each
//! clone stroke then copies from an offset kept in texture space (texels), which the stamp stage applies texel for
//! texel ([`materialcraft::stamp::StampMode::Clone`]).
//!
//! - **Aligned** (the default): the offset is fixed by the first stroke after the source was marked (source minus
//!   that stroke's start), so the source moves on with every later stroke.
//! - **Fixed**: every stroke starts again from the marked source, for repeating one patch.

use materialcraft::projection::ViewProjector;

/// How the source moves from stroke to stroke.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SourceMode {
    #[default]
    Aligned,
    Fixed,
}

/// The clone tool's source.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CloneSource {
    pub mode: SourceMode,
    /// The marked source, in texels of the texture set it was marked in.
    source: Option<(String, [f32; 2])>,
    /// Aligned mode: the offset fixed by the first stroke after marking.
    offset: Option<[i32; 2]>,
}

impl CloneSource {
    /// Marks texel position `texel` of texture set `set` as the source; the next stroke sets the offset anew.
    pub fn mark(&mut self, set: &str, texel: [f32; 2]) {
        if texel.iter().all(|v| v.is_finite()) {
            self.source = Some((set.to_string(), texel));
            self.offset = None;
        }
    }

    /// The marked source, if it lies in texture set `set`.
    pub fn source(&self, set: &str) -> Option<[f32; 2]> {
        self.source.as_ref().filter(|(s, _)| s == set).map(|(_, t)| *t)
    }

    /// The offset (texels) for a stroke into texture set `set` that starts at texel position `start`; `None`
    /// without a source in that set.
    pub fn offset_for_stroke(&mut self, set: &str, start: [f32; 2]) -> Option<[i32; 2]> {
        let src = self.source(set)?;
        let fresh = [(src[0] - start[0]).round() as i32, (src[1] - start[1]).round() as i32];
        Some(match self.mode {
            SourceMode::Aligned => *self.offset.get_or_insert(fresh),
            SourceMode::Fixed => fresh,
        })
    }

    /// Forgets the source (another document).
    pub fn clear(&mut self) {
        self.source = None;
        self.offset = None;
    }
}

/// The texel position under screen position `screen` of `projector`'s view, in a texture of `size` texels; `None`
/// off the model or where its UVs leave the texture (other UV tiles).
pub fn texel_under(projector: &dyn ViewProjector, screen: [f32; 2], size: (u32, u32)) -> Option<[f32; 2]> {
    let hit = projector.pick(screen)?;
    let t = [hit.uv[0] * size.0 as f32, hit.uv[1] * size.1 as f32];
    let inside = t[0] >= 0.0 && t[1] >= 0.0 && t[0] < size.0 as f32 && t[1] < size.1 as f32;
    inside.then_some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aligned_keeps_the_first_offset_and_fixed_starts_over() {
        let mut c = CloneSource::default();
        assert_eq!(c.offset_for_stroke("s", [0.0; 2]), None, "no source yet");
        c.mark("s", [100.0, 50.0]);
        assert_eq!(c.offset_for_stroke("other", [0.0; 2]), None, "the source is in another set");
        assert_eq!(c.offset_for_stroke("s", [60.0, 40.0]), Some([40, 10]));
        assert_eq!(c.offset_for_stroke("s", [10.0, 10.0]), Some([40, 10]), "aligned: the offset stays");
        c.mode = SourceMode::Fixed;
        assert_eq!(c.offset_for_stroke("s", [10.0, 10.0]), Some([90, 40]), "fixed: from the source again");
        c.mode = SourceMode::Aligned;
        c.mark("s", [0.0, 0.0]);
        assert_eq!(c.offset_for_stroke("s", [10.0, 10.0]), Some([-10, -10]), "marking again sets a new offset");
    }
}

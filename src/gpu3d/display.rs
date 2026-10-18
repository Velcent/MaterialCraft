//! [`Display`]: how the viewport turns the scene into the picture, as plain data (DS-14: display only, never part
//! of the document): the camera's settings, exposure in stops, the tone curve, frame accumulation, lit or unlit
//! shading and the texture sharpness. Changing exposure or the tone curve only re-runs the post stage; everything
//! else restarts the accumulation.

use crate::camera::CameraSettings;

/// The tone curve that maps scene radiance to the display range (PP-01, ADR 0008 mc2-vq9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToneCurve {
    /// The viewport's own curve: a filmic shoulder on luminance that keeps saturated colours' hue, with only the
    /// very brightest highlights rolling off per channel. The default.
    #[default]
    Filmic,
    /// No curve: values above 1 clip.
    Linear,
    /// A standard fit of the ACES filmic curve, per channel (bright saturated colours shift towards white).
    Aces,
}

impl ToneCurve {
    /// Every curve with its label, in menu order.
    pub const ALL: [(ToneCurve, &'static str); 3] = [(ToneCurve::Filmic, "Filmic"), (ToneCurve::Linear, "Linear (clip)"), (ToneCurve::Aces, "ACES fit")];

    /// The id stored in settings.
    pub fn id(self) -> &'static str {
        match self {
            ToneCurve::Filmic => "filmic",
            ToneCurve::Linear => "linear",
            ToneCurve::Aces => "aces",
        }
    }

    pub fn from_id(id: &str) -> Option<ToneCurve> {
        ToneCurve::ALL.iter().map(|(c, _)| *c).find(|c| c.id() == id)
    }

    /// The curve's number in the post shader.
    pub(super) fn shader_index(self) -> i32 {
        match self {
            ToneCurve::Filmic => 0,
            ToneCurve::Linear => 1,
            ToneCurve::Aces => 2,
        }
    }
}

/// Anti-aliasing by accumulating jittered frames while nothing changes (PP-02); the window's multisampling
/// applies either way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Antialiasing {
    /// Every frame is drawn once.
    Off,
    /// 8 frames.
    #[default]
    Standard,
    /// 32 frames.
    High,
}

impl Antialiasing {
    pub const ALL: [(Antialiasing, &'static str); 3] =
        [(Antialiasing::Off, "Off"), (Antialiasing::Standard, "Standard (8 frames)"), (Antialiasing::High, "High (32 frames)")];

    pub fn id(self) -> &'static str {
        match self {
            Antialiasing::Off => "off",
            Antialiasing::Standard => "standard",
            Antialiasing::High => "high",
        }
    }

    pub fn from_id(id: &str) -> Option<Antialiasing> {
        Antialiasing::ALL.iter().map(|(a, _)| *a).find(|a| a.id() == id)
    }

    /// Frames accumulated for a still view.
    pub fn frames(self) -> u32 {
        match self {
            Antialiasing::Off => 1,
            Antialiasing::Standard => 8,
            Antialiasing::High => 32,
        }
    }
}

/// How the shown maps are shaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Shading {
    /// Lit by the scene's lights and environment, then exposed and tone mapped.
    #[default]
    Lit,
    /// The base colour (plus emissive) shown as it is: no light, exposure or tone curve (DS-06, single-channel
    /// views by default, ADR 0008 mc2-vq8).
    Unlit,
}

/// The named texture sharpness steps (DS-10, ADR 0008 mc2-vq3): mip-map bias 0 … −3.
pub const MIP_BIAS_STEPS: [(f32, &str); 4] = [(0.0, "Soft"), (-1.0, "Sharp"), (-2.0, "Sharper"), (-3.0, "Very sharp")];

/// Frames accumulated for depth of field when anti-aliasing alone would accumulate fewer.
const DOF_FRAMES: u32 = 32;

/// Display settings of one view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Display {
    pub camera: CameraSettings,
    /// Exposure in stops (0 = as lit, +1 = twice as bright).
    pub exposure_stops: f32,
    pub tone_curve: ToneCurve,
    pub antialiasing: Antialiasing,
    pub shading: Shading,
    /// Mip-map bias, 0 (soft) … −3 (very sharp, shimmers).
    pub mip_bias: f32,
}

impl Default for Display {
    /// The Paint viewport's defaults: filmic curve, no exposure change, standard accumulation.
    fn default() -> Display {
        Display {
            camera: CameraSettings::default(),
            exposure_stops: 0.0,
            tone_curve: ToneCurve::Filmic,
            antialiasing: Antialiasing::Standard,
            shading: Shading::Lit,
            mip_bias: 0.0,
        }
    }
}

impl Display {
    /// Exposure range of the settings, in stops.
    pub const EXPOSURE_RANGE: (f32, f32) = (-6.0, 6.0);

    /// One frame per change and nothing else (views that don't keep repainting, like the node editor's preview).
    pub fn plain() -> Display {
        Display { antialiasing: Antialiasing::Off, ..Display::default() }
    }

    /// Frames to accumulate for a still view (1 = draw once).
    pub fn frames(&self) -> u32 {
        let aa = self.antialiasing.frames();
        if self.camera.blurs() { aa.max(DOF_FRAMES) } else { aa }
    }

    /// The linear scale of the exposure (finite; 1 for a broken value).
    pub fn exposure_scale(&self) -> f32 {
        let s = self.exposure_stops.clamp(Self::EXPOSURE_RANGE.0, Self::EXPOSURE_RANGE.1);
        if s.is_finite() { s.exp2() } else { 1.0 }
    }

    /// The mip bias within its range (0 for a broken value).
    pub fn mip_bias(&self) -> f32 {
        if self.mip_bias.is_finite() { self.mip_bias.clamp(-3.0, 0.0) } else { 0.0 }
    }

    /// The settings that change the drawn scene (exposure and the tone curve only change the post stage).
    pub(super) fn scene_part(&self) -> (CameraSettings, Antialiasing, Shading, u32) {
        (self.camera, self.antialiasing, self.shading, self.mip_bias().to_bits())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::{DepthOfField, Projection};

    #[test]
    fn ids_round_trip_and_defaults_match_the_old_look() {
        for (c, _) in ToneCurve::ALL {
            assert_eq!(ToneCurve::from_id(c.id()), Some(c));
        }
        for (a, _) in Antialiasing::ALL {
            assert_eq!(Antialiasing::from_id(a.id()), Some(a));
        }
        let d = Display::default();
        assert_eq!((d.tone_curve, d.exposure_scale(), d.mip_bias()), (ToneCurve::Filmic, 1.0, 0.0));
        assert_eq!(Display::plain().frames(), 1);
    }

    #[test]
    fn depth_of_field_needs_enough_frames_and_a_lens() {
        let mut d = Display::plain();
        d.camera.depth_of_field = DepthOfField { enabled: true, ..DepthOfField::default() };
        assert_eq!(d.frames(), DOF_FRAMES);
        d.camera.projection = Projection::Orthographic;
        assert_eq!(d.frames(), 1, "a parallel projection has no lens");
        assert_eq!(Display { exposure_stops: f32::NAN, ..d }.exposure_scale(), 1.0);
        assert_eq!(Display { exposure_stops: 1.0, ..d }.exposure_scale(), 2.0);
    }
}

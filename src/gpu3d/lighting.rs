//! The environment's and the shadows' display settings, as plain data inside [`super::Display`] (DS-14: display
//! only). Which environment lights the scene is not here: it is uploaded content ([`super::Gpu::set_environment`]);
//! its turn and alignment are the light direction of [`crate::render3d::View`].

/// How the environment is shown behind the model (EN-05): only the backdrop pixels change, never the lighting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Backdrop {
    /// 0: the plain grey gradient … 1: the environment itself.
    pub opacity: f32,
    /// 0: sharp … 1: as blurred as the roughest reflection.
    pub blur: f32,
}

impl Default for Backdrop {
    fn default() -> Backdrop {
        Backdrop { opacity: 0.0, blur: 0.3 }
    }
}

/// The environment's light.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct EnvironmentLook {
    /// Exposure of the environment in stops (0 = as authored, EN-02).
    pub exposure_stops: f32,
    pub backdrop: Backdrop,
}

impl EnvironmentLook {
    pub const EXPOSURE_RANGE: (f32, f32) = (-6.0, 6.0);

    /// The linear scale of the environment's exposure (1 for a broken value).
    pub fn scale(&self) -> f32 {
        let s = self.exposure_stops.clamp(Self::EXPOSURE_RANGE.0, Self::EXPOSURE_RANGE.1);
        if s.is_finite() { s.exp2() } else { 1.0 }
    }
}

/// How shadows refine (EN-08): how many still frames they take to become soft and whether they are drawn while
/// the view moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShadowQuality {
    /// Moving the view costs nothing extra: a shadow that has to be redrawn (the light turned with the camera,
    /// the model changed) waits until the view rests, then softens over 32 frames.
    #[default]
    Smooth,
    /// Hard shadows also while moving (one extra depth pass per moving frame), soft over 32 frames.
    Balanced,
    /// Like balanced, soft over 128 frames (large soft lights without grain).
    Fine,
}

impl ShadowQuality {
    pub const ALL: [(ShadowQuality, &'static str); 3] =
        [(ShadowQuality::Smooth, "Smooth navigation"), (ShadowQuality::Balanced, "Shadows while moving"), (ShadowQuality::Fine, "Finest (slow refine)")];

    pub fn id(self) -> &'static str {
        match self {
            ShadowQuality::Smooth => "smooth",
            ShadowQuality::Balanced => "balanced",
            ShadowQuality::Fine => "fine",
        }
    }

    pub fn from_id(id: &str) -> Option<ShadowQuality> {
        ShadowQuality::ALL.iter().map(|(q, _)| *q).find(|q| q.id() == id)
    }

    /// Still frames the shadows take to converge.
    pub fn frames(self) -> u32 {
        match self {
            ShadowQuality::Smooth | ShadowQuality::Balanced => 32,
            ShadowQuality::Fine => 128,
        }
    }

    /// True if shadows that must be redrawn are drawn while the view moves.
    pub fn while_moving(self) -> bool {
        self != ShadowQuality::Smooth
    }
}

/// Scene shadows from the environment's dominant lights (EN-07).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shadows {
    pub enabled: bool,
    /// 0: no darkening … 1: full shadow.
    pub opacity: f32,
    pub quality: ShadowQuality,
}

impl Default for Shadows {
    fn default() -> Shadows {
        Shadows { enabled: true, opacity: 1.0, quality: ShadowQuality::Smooth }
    }
}

impl Shadows {
    /// The opacity within 0..1 (0 for a broken value).
    pub fn opacity(&self) -> f32 {
        if self.opacity.is_finite() { self.opacity.clamp(0.0, 1.0) } else { 0.0 }
    }

    /// True if shadows are drawn at all.
    pub fn active(&self) -> bool {
        self.enabled && self.opacity() > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip_and_values_are_guarded() {
        for (q, _) in ShadowQuality::ALL {
            assert_eq!(ShadowQuality::from_id(q.id()), Some(q));
        }
        assert!(!ShadowQuality::Smooth.while_moving() && ShadowQuality::Fine.frames() > ShadowQuality::Balanced.frames());
        assert_eq!(EnvironmentLook { exposure_stops: f32::NAN, ..Default::default() }.scale(), 1.0);
        assert_eq!(EnvironmentLook { exposure_stops: -1.0, ..Default::default() }.scale(), 0.5);
        assert!(!Shadows { opacity: f32::NAN, ..Shadows::default() }.active());
        assert!(Shadows::default().active());
    }
}

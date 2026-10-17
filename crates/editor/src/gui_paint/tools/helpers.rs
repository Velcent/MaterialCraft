//! [`StrokeHelpers`]: the session's lazy mouse, symmetry and visual-aid settings (PT-34 … PT-39), edited in the
//! quick-access bar and its symmetry menu, shared by the brush tools.

use materialcraft::mesh::Vec3;
use materialcraft::symmetry::{Flip, MAX_RADIAL_COPIES, SymmetrySpec};
use materialcraft_brush::LazyMouse;

/// Lazy mouse (PT-34).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LazySettings {
    pub on: bool,
    /// String length in screen points.
    pub radius: f32,
}

impl LazySettings {
    /// The lazy mouse of a new stroke, if switched on.
    pub fn start(&self) -> Option<LazyMouse> {
        if self.on { LazyMouse::new(self.radius) } else { None }
    }
}

/// Mirror or radial.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SymmetryMode {
    #[default]
    Mirror,
    Radial,
}

/// Which visual aids the view draws (PT-39).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SymmetryAids {
    /// The mirror plane, or the radial axis line.
    pub plane: bool,
    /// Where the plane or axis meets the mesh.
    pub meeting: bool,
    /// An extra cursor at each symmetric place.
    pub cursors: bool,
    /// Hide the extra cursors while painting.
    pub hide_cursors_while_painting: bool,
}

impl Default for SymmetryAids {
    fn default() -> SymmetryAids {
        SymmetryAids { plane: true, meeting: true, cursors: true, hide_cursors_while_painting: true }
    }
}

/// Symmetry (PT-36 … PT-38). Settings of both modes are kept while the other is chosen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SymmetrySettings {
    pub on: bool,
    pub mode: SymmetryMode,
    /// Object axis: 0 = X, 1 = Y, 2 = Z.
    pub axis: usize,
    /// Mirror plane offset along the axis (object units).
    pub offset: f32,
    /// Radial: the point the axis passes through.
    pub point: Vec3,
    /// Radial: copies, the original included.
    pub count: u32,
    pub flip: Flip,
    pub aids: SymmetryAids,
}

impl Default for SymmetrySettings {
    fn default() -> SymmetrySettings {
        SymmetrySettings {
            on: false,
            mode: SymmetryMode::Mirror,
            axis: 0,
            offset: 0.0,
            point: [0.0; 3],
            count: 4,
            flip: Flip::None,
            aids: SymmetryAids::default(),
        }
    }
}

impl SymmetrySettings {
    /// The spec strokes are painted with: none while switched off.
    pub fn spec(&self) -> SymmetrySpec {
        if !self.on {
            return SymmetrySpec::None;
        }
        let axis = self.axis.min(2);
        match self.mode {
            SymmetryMode::Mirror => SymmetrySpec::Mirror { axis, offset: self.offset },
            SymmetryMode::Radial => SymmetrySpec::Radial { axis, point: self.point, count: self.count.clamp(2, MAX_RADIAL_COPIES), flip: self.flip },
        }
    }
}

/// The helpers' settings of the session.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrokeHelpers {
    pub lazy: LazySettings,
    pub symmetry: SymmetrySettings,
}

impl Default for StrokeHelpers {
    fn default() -> StrokeHelpers {
        StrokeHelpers { lazy: LazySettings { on: false, radius: 25.0 }, symmetry: SymmetrySettings::default() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_give_specs_only_while_on() {
        let mut s = SymmetrySettings::default();
        assert_eq!(s.spec(), SymmetrySpec::None);
        s.on = true;
        s.offset = 0.5;
        assert_eq!(s.spec(), SymmetrySpec::Mirror { axis: 0, offset: 0.5 });
        s.mode = SymmetryMode::Radial;
        s.count = 1000;
        s.axis = 9;
        assert_eq!(s.spec(), SymmetrySpec::Radial { axis: 2, point: [0.0; 3], count: MAX_RADIAL_COPIES, flip: Flip::None });
        let mut h = StrokeHelpers::default();
        assert!(h.lazy.start().is_none());
        h.lazy.on = true;
        assert_eq!(h.lazy.start().map(|l| l.radius()), Some(25.0));
    }
}

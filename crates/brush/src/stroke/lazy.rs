//! [`LazyMouse`]: the brush trails the pointer at a distance (PT-34).

use crate::engine::{InputPoint, MAX_STRING, PulledString};

/// One stroke's lazy mouse. A larger radius gives a smoother line and slower turns. The first point paints where
/// it is; later points paint only once the pointer is `radius` away from the brush, which then moves towards it.
pub struct LazyMouse {
    inner: PulledString,
    radius: f32,
}

impl LazyMouse {
    /// The longest string (frame units).
    pub const MAX_RADIUS: f32 = MAX_STRING;

    /// A lazy mouse with a string of `radius` frame units (clamped to [`LazyMouse::MAX_RADIUS`]); `None` for a
    /// radius of zero or less, which is no lazy mouse.
    pub fn new(radius: f32) -> Option<LazyMouse> {
        (radius.is_finite() && radius > 0.0).then(|| {
            let radius = radius.min(MAX_STRING);
            LazyMouse { inner: PulledString::new(radius), radius }
        })
    }

    pub fn radius(&self) -> f32 {
        self.radius
    }

    /// The brush positions pointer points pull the brush to.
    pub fn push(&mut self, points: &[InputPoint]) -> Vec<InputPoint> {
        let mut out = Vec::with_capacity(points.len());
        for p in points {
            self.inner.push(*p, &mut out);
        }
        out
    }

    /// `(brush, pointer)` while the brush lags behind the pointer (the overlay's trailing line, PT-35).
    pub fn string(&self) -> Option<([f32; 2], [f32; 2])> {
        self.inner.string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_brush_trails_the_pointer_at_the_radius() {
        assert!(LazyMouse::new(0.0).is_none() && LazyMouse::new(f32::NAN).is_none());
        let mut lazy = LazyMouse::new(20.0).unwrap();
        let first = lazy.push(&[InputPoint::at([0.0, 0.0], 0.0)]);
        assert_eq!(first.len(), 1, "the first point paints");
        // within the radius the brush stays put
        assert!(lazy.push(&[InputPoint::at([15.0, 0.0], 0.1)]).is_empty());
        assert_eq!(lazy.string(), Some(([0.0, 0.0], [15.0, 0.0])));
        // past it the brush follows, the radius behind
        let moved = lazy.push(&[InputPoint { pressure: 0.4, ..InputPoint::at([50.0, 0.0], 0.2) }]);
        assert_eq!(moved.len(), 1);
        assert!((moved[0].pos[0] - 30.0).abs() < 1e-4 && moved[0].pos[1] == 0.0, "{:?}", moved[0].pos);
        assert_eq!((moved[0].pressure, moved[0].time), (0.4, 0.2));
        assert_eq!(LazyMouse::new(1e6).unwrap().radius(), LazyMouse::MAX_RADIUS);
    }
}

//! Painter's export conventions, applied before comparing: which values a file holds for a channel.
//!
//! - Height is exported as `0.5 + 0.5 × height` (Verified: least-squares fit gain 0.50, offset 0.50 on
//!   Modulin; 0 stays mid-grey).
//! - Integer files hold 0..1, so values outside are clipped.
//! - Packed maps hold one channel per component; DirectX normals have green flipped.

use super::exports::ExportMap;
use materialcraft::paint::Channel;
use materialcraft::tex::Tex;

/// Our map as Painter would export it to an integer file.
pub fn ours_as_exported(channel: Channel, ours: &Tex) -> Tex {
    let clip = |v: f32| v.clamp(0.0, 1.0);
    match channel {
        Channel::Height => ours.map_px(|p| {
            let v = clip(0.5 + 0.5 * p[0]);
            [v, v, v, 1.0]
        }),
        _ => ours.map_px(|p| [clip(p[0]), clip(p[1]), clip(p[2]), clip(p[3])]),
    }
}

/// The exported file in our conventions: one packed component spread to gray, DirectX normals turned to OpenGL.
pub fn theirs_in_our_terms(theirs: &Tex, export: &ExportMap) -> Tex {
    theirs.map_px(|p| match (export.component, export.directx) {
        (Some(c), _) => [p[c], p[c], p[c], 1.0],
        (None, true) => [p[0], 1.0 - p[1], p[2], p[3]],
        (None, false) => p,
    })
}

/// Per-pixel map without threads or job bookkeeping.
trait MapPx {
    fn map_px(&self, f: impl Fn([f32; 4]) -> [f32; 4]) -> Tex;
}

impl MapPx for Tex {
    fn map_px(&self, f: impl Fn([f32; 4]) -> [f32; 4]) -> Tex {
        Tex {
            w: self.w,
            h: self.h,
            px: self.px.iter().map(|p| f(*p)).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn height_is_offset_and_packed_components_spread() {
        let h = ours_as_exported(Channel::Height, &Tex::solid(1, 1, [0.2, 0.2, 0.2, 1.0]));
        assert!((h.px[0][0] - 0.6).abs() < 1e-6);
        assert_eq!(
            ours_as_exported(Channel::Height, &Tex::solid(1, 1, [3.0; 4])).px[0][0],
            1.0
        );
        let export = ExportMap {
            channel: Channel::Roughness,
            path: PathBuf::new(),
            component: Some(1),
            directx: false,
            suffix: "orm".into(),
        };
        let t = theirs_in_our_terms(&Tex::solid(1, 1, [0.1, 0.7, 0.3, 1.0]), &export);
        assert_eq!(t.px[0], [0.7, 0.7, 0.7, 1.0]);
    }
}

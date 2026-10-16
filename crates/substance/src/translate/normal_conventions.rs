//! [`NormalConventions`]: how Painter built a project's normal maps, in our terms. Painter's height → normal
//! filter changed with the document format ([`height_filter_for`]); baked normal maps are stored in the
//! project's normal map format (`normalMapSyscoord`), while a document holds every normal map as OpenGL, so
//! DirectX bakes get their green flipped on import ([`NormalConventions::to_opengl`]).

use crate::decode::document::{DocumentSettings, NormalMapSyscoord};
use materialcraft::paint::height_normal::HeightFilter;
use materialcraft::paint::{BakedKind, BakedMap, TiledImage};

/// The height → normal filter Painter used for a document of this format version (Verified on exports:
/// Sobel up to 61, backward differences from 64; 62 and 63 are unseen and treated as the newer filter).
pub fn height_filter_for(format_version: u32) -> HeightFilter {
    if format_version <= 61 {
        HeightFilter::Sobel
    } else {
        HeightFilter::Backward
    }
}

/// The convention baked normal maps are stored in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BakedNormals {
    OpenGl,
    DirectX,
    /// No or an unknown `normalMapSyscoord`: left as stored (OpenGL assumed).
    Unknown(Option<u64>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NormalConventions {
    /// `None` if `document.bin` could not be read; the filter is then our default.
    pub format_version: Option<u32>,
    pub height_filter: HeightFilter,
    pub baked: BakedNormals,
}

impl NormalConventions {
    pub fn from_settings(settings: Option<&DocumentSettings>) -> NormalConventions {
        let Some(s) = settings else {
            return NormalConventions {
                format_version: None,
                height_filter: HeightFilter::default(),
                baked: BakedNormals::Unknown(None),
            };
        };
        NormalConventions {
            format_version: Some(s.format_version),
            height_filter: height_filter_for(s.format_version),
            baked: match s.normal_map_syscoord {
                Some(NormalMapSyscoord::OpenGl) => BakedNormals::OpenGl,
                Some(NormalMapSyscoord::DirectX) => BakedNormals::DirectX,
                Some(NormalMapSyscoord::Other(n)) => BakedNormals::Unknown(Some(n)),
                None => BakedNormals::Unknown(None),
            },
        }
    }

    /// Turns a baked tangent-space normal map into OpenGL; other maps are left alone.
    pub fn to_opengl(&self, map: &mut BakedMap) {
        if map.kind == BakedKind::Normal && self.baked == BakedNormals::DirectX {
            flip_green(&mut map.image);
        }
    }
}

/// `g → 1 − g` on every stored texel (exact for integer formats). Unallocated tiles stay unallocated.
fn flip_green(image: &mut TiledImage) {
    let tiles: Vec<(u32, u32)> = image
        .all_tiles()
        .into_iter()
        .filter(|&(x, y)| image.tile_allocated(x, y))
        .collect();
    image.update_tiles(&tiles, |_, px| px.iter_mut().for_each(|p| p[1] = 1.0 - p[1]));
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft::paint::PixelFormat;

    fn settings(format_version: u32, syscoord: Option<NormalMapSyscoord>) -> DocumentSettings {
        DocumentSettings {
            format_version,
            normal_map_syscoord: syscoord,
        }
    }

    #[test]
    fn filter_follows_document_format() {
        assert_eq!(height_filter_for(59), HeightFilter::Sobel);
        assert_eq!(height_filter_for(61), HeightFilter::Sobel);
        assert_eq!(height_filter_for(64), HeightFilter::Backward);
        assert_eq!(height_filter_for(78), HeightFilter::Backward);
    }

    #[test]
    fn reads_settings_and_falls_back() {
        let n = NormalConventions::from_settings(Some(&settings(59, Some(NormalMapSyscoord::DirectX))));
        assert_eq!((n.height_filter, n.baked), (HeightFilter::Sobel, BakedNormals::DirectX));
        let n = NormalConventions::from_settings(Some(&settings(68, Some(NormalMapSyscoord::Other(3)))));
        assert_eq!(
            (n.height_filter, n.baked),
            (HeightFilter::Backward, BakedNormals::Unknown(Some(3)))
        );
        let n = NormalConventions::from_settings(None);
        assert_eq!((n.format_version, n.baked), (None, BakedNormals::Unknown(None)));
    }

    #[test]
    fn directx_normal_maps_get_green_flipped_exactly() {
        let mut image = TiledImage::new(3, 2, PixelFormat::from_id("rgb16").unwrap()).unwrap();
        image.set(1, 1, [0.25, 1000.0 / 65535.0, 1.0, 1.0]);
        let mut normal = BakedMap {
            kind: BakedKind::Normal,
            image: image.clone(),
        };
        let mut ao = BakedMap {
            kind: BakedKind::AmbientOcclusion,
            image,
        };
        let dx = NormalConventions::from_settings(Some(&settings(78, Some(NormalMapSyscoord::DirectX))));
        dx.to_opengl(&mut normal);
        dx.to_opengl(&mut ao);
        let raw = normal.image.to_raw();
        let g = u16::from_le_bytes([raw[(3 + 1) * 6 + 2], raw[(3 + 1) * 6 + 3]]);
        assert_eq!(g, 65535 - 1000);
        assert_eq!(normal.image.get(1, 1)[0], ao.image.get(1, 1)[0]);
        assert_eq!(ao.image.get(1, 1)[1], 1000.0 / 65535.0);
        let mut again = normal.clone();
        let gl = NormalConventions::from_settings(Some(&settings(78, Some(NormalMapSyscoord::OpenGl))));
        gl.to_opengl(&mut again);
        assert_eq!(again, normal);
    }
}

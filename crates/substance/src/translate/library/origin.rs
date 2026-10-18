//! [`Origin`]: where a project had a library candidate from, read from its resource URL, and whether a copy in the
//! user's library may be shared.
//!
//! The rule (from the URLs of the corpus projects, `research/procedural-census.md` counts the shelves):
//! - `/<name>?version=<id>`: a resource stored in the project itself (imported into it by the user, or made by Painter
//!   for it). The user's own: shareable.
//! - `resource://<shelf>/<path>?version=<id>`: from a shelf; the shelf is the first path segment up to a `.` (Painter
//!   appends a library id: `allegorithmic.1c86…`, `shelf.1328…`).
//!   - Painter's shipped shelves are **vendor** content, never shareable: `allegorithmic` (the shipped library),
//!     `starter_assets`, `legacy`, `tool` (the tools' own resources, e.g. the brush `shape`), `viewer` (the
//!     environment maps), `ps_brushes_context`, and anything named `adobe…` or `substance…`.
//!   - `shelf` is the user's own shelf (the "assets" folder in their documents): shareable.
//!   - Any other shelf name (a library the user added, a marketplace or third-party pack such as `custom_materials`)
//!     cannot be told apart from bought content, so it is treated like the vendor's: kept with the user, not
//!     shareable.
//! - Anything else (`resourcealias:…`, empty) is no stored resource and is never offered.

/// Shelf names (before any `.<library id>`) of the content Painter ships.
const VENDOR_SHELVES: &[&str] = &[
    "allegorithmic",
    "starter_assets",
    "legacy",
    "tool",
    "viewer",
    "ps_brushes_context",
];
/// Prefixes of vendor shelf names.
const VENDOR_PREFIXES: &[&str] = &["adobe", "substance"];
/// The user's own shelf.
const USER_SHELF: &str = "shelf";

/// Where a candidate came from.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Origin {
    /// Stored in the project (`/<name>?version=…`), or made from the project's own settings (brush presets).
    Project,
    /// The user's own Painter shelf.
    UserShelf,
    /// A shelf Painter ships (`allegorithmic`, `starter_assets` …), by name.
    VendorShelf(String),
    /// Another shelf (an added library, a third-party pack), by name.
    OtherShelf(String),
}

impl Origin {
    /// The origin of a resource URL; `None` for URLs that name no stored resource.
    pub fn of_url(url: &str) -> Option<Origin> {
        if let Some(rest) = url.strip_prefix("resource://") {
            let shelf = rest.split('/').next().unwrap_or_default();
            let name = shelf.split('.').next().unwrap_or_default().to_ascii_lowercase();
            if name.is_empty() {
                return None;
            }
            return Some(if name == USER_SHELF {
                Origin::UserShelf
            } else if VENDOR_SHELVES.contains(&name.as_str()) || VENDOR_PREFIXES.iter().any(|p| name.starts_with(p)) {
                Origin::VendorShelf(name)
            } else {
                Origin::OtherShelf(name)
            });
        }
        url.starts_with('/').then_some(Origin::Project)
    }

    /// True when a copy may leave the user's own library (it is the user's own work).
    pub fn shareable(&self) -> bool {
        matches!(self, Origin::Project | Origin::UserShelf)
    }

    /// The origin as the review and the sidecar say it.
    pub fn describe(&self) -> String {
        match self {
            Origin::Project => "stored in the project".to_string(),
            Origin::UserShelf => "your Painter shelf".to_string(),
            Origin::VendorShelf(s) => format!("Painter's shipped shelf \"{s}\""),
            Origin::OtherShelf(s) => format!("the Painter shelf \"{s}\""),
        }
    }

    /// The less shareable of two origins (a brush whose alpha came from the vendor's shelf is the vendor's).
    pub fn stricter(self, other: Origin) -> Origin {
        if self.shareable() { other } else { self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shelf_decides_the_origin() {
        let of = |u: &str| Origin::of_url(u);
        assert_eq!(of("/Scratches?version=ab12"), Some(Origin::Project));
        assert_eq!(
            of("resource://shelf/Cutlery1?version=c51e.image"),
            Some(Origin::UserShelf)
        );
        assert_eq!(
            of("resource://shelf.1328-81/noise/noise?version=1.sbsar"),
            Some(Origin::UserShelf)
        );
        assert_eq!(
            of("resource://allegorithmic.1c86/shape/shape?version=f2.sbsar"),
            Some(Origin::VendorShelf("allegorithmic".into()))
        );
        assert_eq!(
            of("resource://starter_assets/Dirt?version=1"),
            Some(Origin::VendorShelf("starter_assets".into()))
        );
        assert_eq!(
            of("resource://viewer/panorama?version=ce.image"),
            Some(Origin::VendorShelf("viewer".into()))
        );
        assert_eq!(
            of("resource://Adobe_Pack/x?version=1"),
            Some(Origin::VendorShelf("adobe_pack".into()))
        );
        assert_eq!(
            of("resource://custom_materials/Gold?version=1"),
            Some(Origin::OtherShelf("custom_materials".into()))
        );
        assert_eq!(of("resourcealias:Curvature"), None);
        assert_eq!(of(""), None);
        assert_eq!(of("resource:///x"), None);
    }

    #[test]
    fn only_the_users_own_work_is_shareable() {
        assert!(Origin::Project.shareable() && Origin::UserShelf.shareable());
        assert!(!Origin::VendorShelf("tool".into()).shareable() && !Origin::OtherShelf("x".into()).shareable());
        assert_eq!(
            Origin::Project.stricter(Origin::VendorShelf("tool".into())),
            Origin::VendorShelf("tool".into())
        );
        assert_eq!(
            Origin::OtherShelf("x".into()).stricter(Origin::Project),
            Origin::OtherShelf("x".into())
        );
    }
}

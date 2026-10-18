//! [`GeometryMask`]: a second mask of a layer or folder that limits it to parts of the geometry, by name or UV tile,
//! so it survives a mesh re-import (GM-01, GM-06). The compositor multiplies it in per texel from the surface map's
//! part ids; a layer without one (`None`) includes everything (GM-02).
//!
//! The helpers here are what the editor's edit mode needs without knowing the representation: how much of a set a
//! mask includes ([`Inclusion`]) and the mask after one part is switched on or off ([`GeometryMask::with_part`]).
//! UV tiles are data only until the format has UDIMs (decision 0008, q-udims); they are a variant of their own so
//! adding them changes no stored mask.

use crate::mesh::Mesh;

#[derive(Debug, Clone, PartialEq)]
pub enum GeometryMask {
    /// The layer applies only on these mesh parts (by [`crate::mesh::MeshPart::name`]). Names that no part of the
    /// current mesh carries are kept, so a part that is missing from one import comes back masked as before.
    Parts(Vec<String>),
    /// The layer applies only on these UV tiles (UDIM numbers, 1001 = the first tile).
    UvTiles(Vec<u32>),
}

/// How much of a texture set's geometry a mask lets through, for the layer panel's icon (GM-02).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Inclusion {
    /// Every part (also: no mask at all, or a mask the set cannot judge, such as UV tiles).
    All,
    /// Some parts: `included` of `total`.
    Some { included: usize, total: usize },
    /// No part.
    None,
}

impl GeometryMask {
    /// True if the mask lets the part named `name` through. A UV-tile mask does not restrict parts.
    pub fn includes_part(&self, name: &str) -> bool {
        match self {
            GeometryMask::Parts(parts) => parts.iter().any(|p| p == name),
            GeometryMask::UvTiles(_) => true,
        }
    }

    /// How much of the set whose parts are `set_parts` the mask `mask` includes (`None`: no mask).
    pub fn inclusion<S: AsRef<str>>(mask: Option<&GeometryMask>, set_parts: &[S]) -> Inclusion {
        let Some(mask) = mask else { return Inclusion::All };
        let total = set_parts.len();
        let included = set_parts.iter().filter(|p| mask.includes_part(p.as_ref())).count();
        match included {
            n if n == total => Inclusion::All,
            0 => Inclusion::None,
            included => Inclusion::Some { included, total },
        }
    }

    /// The mask after switching the part `part` on (`include`) or off, for a set whose parts are `set_parts`.
    /// A mask that then includes every part of the set becomes `None` (GM-02: everything, also parts added by a
    /// later re-import). A UV-tile mask is replaced by a part list.
    pub fn with_part<S: AsRef<str>>(mask: Option<&GeometryMask>, part: &str, include: bool, set_parts: &[S]) -> Option<GeometryMask> {
        let mut parts: Vec<String> = match mask {
            Some(GeometryMask::Parts(parts)) => parts.clone(),
            None | Some(GeometryMask::UvTiles(_)) => set_parts.iter().map(|p| p.as_ref().to_owned()).collect(),
        };
        parts.retain(|p| p != part);
        if include {
            parts.push(part.to_owned());
        }
        let mask = GeometryMask::Parts(parts);
        match set_parts.iter().all(|p| mask.includes_part(p.as_ref())) {
            true => None,
            false => Some(mask),
        }
    }

    /// The mask that includes no part (GM-04 "exclude all").
    pub fn nothing() -> GeometryMask {
        GeometryMask::Parts(Vec::new())
    }

    /// The names of the parts on material slot `material` of `mesh`, in mesh order, each once: the parts a set's
    /// geometry mask chooses from.
    pub fn set_parts(mesh: &Mesh, material: usize) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for p in mesh.parts.iter().filter(|p| p.material == material) {
            if !names.contains(&p.name) {
                names.push(p.name.clone());
            }
        }
        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SET: [&str; 3] = ["body", "trim", "glass"];

    #[test]
    fn inclusion_counts_the_sets_parts() {
        assert_eq!(GeometryMask::inclusion(None, &SET), Inclusion::All);
        let some = GeometryMask::Parts(vec!["trim".into(), "gone".into()]);
        assert_eq!(GeometryMask::inclusion(Some(&some), &SET), Inclusion::Some { included: 1, total: 3 });
        assert_eq!(GeometryMask::inclusion(Some(&GeometryMask::nothing()), &SET), Inclusion::None);
        assert_eq!(GeometryMask::inclusion(Some(&GeometryMask::UvTiles(vec![1001])), &SET), Inclusion::All);
    }

    #[test]
    fn switching_parts_keeps_unknown_names_and_collapses_to_everything() {
        let off = GeometryMask::with_part(None, "trim", false, &SET);
        assert_eq!(off, Some(GeometryMask::Parts(vec!["body".into(), "glass".into()])));
        let gone = Some(GeometryMask::Parts(vec!["old".into(), "body".into()]));
        let on = GeometryMask::with_part(gone.as_ref(), "glass", true, &SET);
        assert_eq!(on, Some(GeometryMask::Parts(vec!["old".into(), "body".into(), "glass".into()])));
        assert_eq!(GeometryMask::with_part(on.as_ref(), "trim", true, &SET), None, "all parts included is no mask");
        assert_eq!(GeometryMask::with_part(on.as_ref(), "trim", false, &SET), on, "switching off an excluded part changes nothing");
    }
}

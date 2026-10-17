//! Structural checks: array lengths, triangle indices, material slots and finite values.

use super::error::{Attribute, MeshError, MeshErrorKind};
use super::{Mesh, MeshPart};

impl Mesh {
    /// Checks every part and that each part's material slot exists. Reports the first problem found.
    pub fn validate(&self) -> Result<(), MeshError> {
        for (i, part) in self.parts.iter().enumerate() {
            if part.material >= self.materials.len() {
                let kind = MeshErrorKind::MaterialOutOfRange { material: part.material, material_count: self.materials.len() };
                return Err(MeshError::new(kind).in_part(i));
            }
            part.validate().map_err(|e| e.in_part(i))?;
        }
        Ok(())
    }
}

impl MeshPart {
    /// Checks this part on its own: every vertex array matches `positions` (normals and tangents may be empty),
    /// every triangle index is in range and every value is finite. The material slot is checked by [`Mesh::validate`].
    pub fn validate(&self) -> Result<(), MeshError> {
        let n = self.positions.len();
        check_len(Attribute::Normals, n, self.normals.len(), true)?;
        check_len(Attribute::Tangents, n, self.tangents.len(), true)?;
        for (s, uv) in self.uv_sets.iter().enumerate() {
            check_len(Attribute::UvSet(s), n, uv.len(), false)?;
        }
        for (t, tri) in self.triangles.iter().enumerate() {
            if let Some(&index) = tri.iter().find(|&&i| i as usize >= n) {
                return Err(MeshError::new(MeshErrorKind::IndexOutOfRange { triangle: t, index, vertex_count: n }));
            }
        }
        check_finite(Attribute::Positions, self.positions.iter().map(|v| &v[..]))?;
        check_finite(Attribute::Normals, self.normals.iter().map(|v| &v[..]))?;
        check_finite(Attribute::Tangents, self.tangents.iter().map(|v| &v[..]))?;
        for (s, uv) in self.uv_sets.iter().enumerate() {
            check_finite(Attribute::UvSet(s), uv.iter().map(|v| &v[..]))?;
        }
        Ok(())
    }
}

fn check_len(attribute: Attribute, expected: usize, found: usize, may_be_empty: bool) -> Result<(), MeshError> {
    if found == expected || (may_be_empty && found == 0) { Ok(()) } else { Err(MeshError::new(MeshErrorKind::LengthMismatch { attribute, expected, found })) }
}

fn check_finite<'a>(attribute: Attribute, values: impl Iterator<Item = &'a [f32]>) -> Result<(), MeshError> {
    for (vertex, v) in values.enumerate() {
        if !v.iter().all(|x| x.is_finite()) {
            return Err(MeshError::new(MeshErrorKind::NonFinite { attribute, vertex }));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::test_util::quad;

    fn mesh(part: MeshPart) -> Mesh {
        Mesh { materials: vec!["m".into()], parts: vec![part] }
    }

    #[test]
    fn valid_quad_passes() {
        assert_eq!(mesh(quad()).validate(), Ok(()));
    }

    #[test]
    fn empty_normals_and_tangents_are_allowed() {
        let mut p = quad();
        p.normals.clear();
        p.tangents.clear();
        assert_eq!(p.validate(), Ok(()));
    }

    #[test]
    fn length_mismatch() {
        let mut p = quad();
        p.uv_sets[0].pop();
        let e = mesh(p).validate().unwrap_err();
        assert_eq!(e.part, Some(0));
        assert_eq!(e.kind, MeshErrorKind::LengthMismatch { attribute: Attribute::UvSet(0), expected: 4, found: 3 });
        assert!(e.to_string().contains("UV set 0"));
    }

    #[test]
    fn index_out_of_range() {
        let mut p = quad();
        p.triangles[1][2] = 9;
        let e = p.validate().unwrap_err();
        assert_eq!(e.kind, MeshErrorKind::IndexOutOfRange { triangle: 1, index: 9, vertex_count: 4 });
    }

    #[test]
    fn material_out_of_range() {
        let mut p = quad();
        p.material = 3;
        let e = mesh(p).validate().unwrap_err();
        assert_eq!(e.kind, MeshErrorKind::MaterialOutOfRange { material: 3, material_count: 1 });
    }

    #[test]
    fn non_finite() {
        let mut p = quad();
        p.normals[2][1] = f32::NAN;
        let e = p.validate().unwrap_err();
        assert_eq!(e.kind, MeshErrorKind::NonFinite { attribute: Attribute::Normals, vertex: 2 });
    }
}

//! Axis-aligned bounds, bounding spheres and the transform that fits a mesh into the viewport.

use super::{Mesh, MeshPart, Vec3};
use crate::v3::{add, len, mul, sub};

/// An axis-aligned box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub min: Vec3,
    pub max: Vec3,
}

/// A sphere enclosing a set of points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sphere {
    pub centre: Vec3,
    pub radius: f32,
}

/// Translate, then scale uniformly: `p' = (p + translate) * scale`. Normals and tangents are unaffected.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fit {
    pub translate: Vec3,
    pub scale: f32,
}

impl Bounds {
    /// The box around all finite points, or `None` when there are none.
    pub fn of_points<'a>(points: impl IntoIterator<Item = &'a Vec3>) -> Option<Bounds> {
        points
            .into_iter()
            .filter(|p| p.iter().all(|x| x.is_finite()))
            .fold(None, |b, &p| {
                Some(match b {
                    None => Bounds { min: p, max: p },
                    Some(b) => b.including(p),
                })
            })
    }

    /// The box grown to contain `p`.
    pub fn including(self, p: Vec3) -> Bounds {
        Bounds {
            min: [0, 1, 2].map(|k| self.min[k].min(p[k])),
            max: [0, 1, 2].map(|k| self.max[k].max(p[k])),
        }
    }

    /// The smallest box containing both.
    pub fn union(self, other: Bounds) -> Bounds {
        self.including(other.min).including(other.max)
    }

    pub fn centre(&self) -> Vec3 {
        mul(add(self.min, self.max), 0.5)
    }

    pub fn size(&self) -> Vec3 {
        sub(self.max, self.min)
    }
}

impl Fit {
    pub const IDENTITY: Fit = Fit {
        translate: [0.0; 3],
        scale: 1.0,
    };

    /// Moves `sphere` to the origin and scales it to radius 1. A point-sized sphere is only moved.
    pub fn to_unit_sphere(sphere: Sphere) -> Fit {
        let scale = if sphere.radius > 1e-20 && sphere.radius.is_finite() {
            1.0 / sphere.radius
        } else {
            1.0
        };
        Fit {
            translate: mul(sphere.centre, -1.0),
            scale,
        }
    }

    pub fn apply(&self, p: Vec3) -> Vec3 {
        mul(add(p, self.translate), self.scale)
    }

    /// Maps a fitted point back to the mesh's own space (for picking).
    pub fn invert(&self, p: Vec3) -> Vec3 {
        sub(mul(p, 1.0 / self.scale), self.translate)
    }

    /// Scales an object-space length.
    pub fn apply_length(&self, l: f32) -> f32 {
        l * self.scale
    }
}

impl Default for Fit {
    fn default() -> Fit {
        Fit::IDENTITY
    }
}

impl MeshPart {
    pub fn bounds(&self) -> Option<Bounds> {
        Bounds::of_points(&self.positions)
    }

    /// Distance from `centre` to the farthest finite position (0 when there is none).
    pub fn radius_about(&self, centre: Vec3) -> f32 {
        self.positions
            .iter()
            .map(|&p| len(sub(p, centre)))
            .filter(|d| d.is_finite())
            .fold(0.0, f32::max)
    }
}

impl Mesh {
    pub fn bounds(&self) -> Option<Bounds> {
        self.parts
            .iter()
            .filter_map(MeshPart::bounds)
            .reduce(Bounds::union)
    }

    /// A sphere around the bounds' centre enclosing every vertex of every part.
    pub fn bounding_sphere(&self) -> Option<Sphere> {
        let centre = self.bounds()?.centre();
        let radius = self
            .parts
            .iter()
            .map(|p| p.radius_about(centre))
            .fold(0.0, f32::max);
        Some(Sphere { centre, radius })
    }

    /// The transform that centres the whole mesh at the origin with radius 1, so models of any size appear like
    /// the built-in shapes. Identity for an empty mesh.
    pub fn fit(&self) -> Fit {
        self.bounding_sphere()
            .map_or(Fit::IDENTITY, Fit::to_unit_sphere)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::test_util::{close, quad};

    fn offset_mesh() -> Mesh {
        let mut a = quad();
        let mut b = quad();
        for p in &mut a.positions {
            *p = add(mul(*p, 10.0), [100.0, 50.0, -20.0]);
        }
        for p in &mut b.positions {
            *p = add(mul(*p, 10.0), [100.0, 50.0, 0.0]);
        }
        Mesh {
            materials: vec!["m".into()],
            parts: vec![a, b],
        }
    }

    #[test]
    fn bounds_of_parts() {
        let m = offset_mesh();
        let b = m.bounds().unwrap();
        assert_eq!(b.min, [90.0, 40.0, -20.0]);
        assert_eq!(b.max, [110.0, 60.0, 0.0]);
        assert_eq!(b.centre(), [100.0, 50.0, -10.0]);
        assert_eq!(b.size(), [20.0, 20.0, 20.0]);
    }

    #[test]
    fn fit_off_centre_mesh() {
        let m = offset_mesh();
        let fit = m.fit();
        let fitted: Vec<Vec3> = m
            .parts
            .iter()
            .flat_map(|p| p.positions.iter().map(|&q| fit.apply(q)))
            .collect();
        let r = fitted.iter().map(|&p| len(p)).fold(0.0, f32::max);
        assert!((r - 1.0).abs() < 1e-5, "{r}");
        let c = Bounds::of_points(&fitted).unwrap().centre();
        assert!(close(&c, &[0.0; 3], 1e-5), "{c:?}");
        let p = m.parts[0].positions[0];
        assert!(close(&fit.invert(fit.apply(p)), &p, 1e-3));
    }

    #[test]
    fn empty_and_degenerate_meshes() {
        assert_eq!(Mesh::default().fit(), Fit::IDENTITY);
        let mut p = quad();
        p.positions = vec![[3.0, 3.0, 3.0]; 4];
        p.positions[1][0] = f32::NAN;
        let m = Mesh {
            materials: vec![],
            parts: vec![p],
        };
        assert_eq!(
            m.fit(),
            Fit {
                translate: [-3.0; 3],
                scale: 1.0
            }
        );
    }
}

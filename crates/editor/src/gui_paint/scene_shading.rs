//! What the viewport draws each part with (VS-02 … VS-10): its texture set's shader instance as a
//! [`PartShading`] (program, values, displacement in viewport units) and how far its mesh is subdivided.

use materialcraft::gpu3d::{InstanceDisplacement, PartShading};
use materialcraft::mesh::{Mesh, SubdivisionScheme};
use materialcraft::paint::Document;
use materialcraft::shading::{SceneScale, ShaderInstance, ShaderLibrary, TessellationPlan};

/// How far one texture set's parts are subdivided.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Subdivision {
    pub levels: u32,
    pub scheme: SubdivisionScheme,
}

/// The model's size, as displacement units and tessellation need it.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct ModelScale {
    pub triangles: usize,
    pub longest_edge: f32,
    /// Bounding-box diagonal in mesh units.
    pub diagonal: f32,
    /// Mesh units → viewport units (the model is fitted to unit size).
    pub fit_scale: f32,
}

impl ModelScale {
    pub fn of(mesh: &Mesh) -> ModelScale {
        let diagonal = mesh.bounds().map_or(0.0, |b| {
            let s = b.size();
            (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt()
        });
        ModelScale {
            triangles: mesh.parts.iter().map(|p| p.triangles.len()).sum(),
            longest_edge: mesh.parts.iter().map(|p| p.longest_edge()).fold(0.0, f32::max),
            diagonal: if diagonal.is_finite() { diagonal } else { 0.0 },
            fit_scale: mesh.fit().scale,
        }
    }
}

/// The tessellation plan of `instance` on this model.
pub fn plan(instance: &ShaderInstance, model: &ModelScale) -> TessellationPlan {
    instance.tessellation.plan(model.triangles, model.longest_edge, model.diagonal)
}

/// The subdivision of every texture set, by index (sets without an instance aren't subdivided).
pub fn subdivisions(doc: &Document, model: &ModelScale) -> Vec<Subdivision> {
    doc.texture_sets
        .iter()
        .map(|s| match doc.shader_instance(&s.id) {
            None => Subdivision::default(),
            Some(i) => {
                let scheme = if i.tessellation.smooth { SubdivisionScheme::Loop } else { SubdivisionScheme::Linear };
                Subdivision { levels: plan(i, model).levels, scheme }
            }
        })
        .collect()
}

/// The shading of every scene part (`part_sets[i]`: the texture set index of part `i`).
pub fn part_shading(doc: &Document, library: &ShaderLibrary, model: &ModelScale, part_sets: &[Option<usize>]) -> Vec<PartShading> {
    let scene = SceneScale { diagonal: model.diagonal, unit: doc.shading.mesh_unit };
    part_sets
        .iter()
        .map(|set| {
            let instance = set.and_then(|s| doc.texture_sets.get(s)).and_then(|s| doc.shader_instance(&s.id));
            match instance {
                None => PartShading::default(),
                Some(i) => {
                    let displacement = i
                        .displacement
                        .enabled
                        .then(|| InstanceDisplacement { distance: i.displacement.distance(&scene).0 * model.fit_scale, level: i.displacement.midlevel() });
                    PartShading::of(i, library.find_or_default(&i.shader), displacement)
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft::mesh::{LengthUnit, MeshPart};
    use materialcraft::paint::TextureSet;
    use materialcraft::shading::{Displacement, DisplacementUnit, ParamValue, ShaderEdit, ShaderFile, Tessellation, TessellationMode};

    fn doc() -> Document {
        // a 4 x 2 x 0 rectangle: diagonal sqrt(20), fitted to radius 1
        let part = MeshPart {
            positions: vec![[0.0, 0.0, 0.0], [4.0, 0.0, 0.0], [4.0, 2.0, 0.0], [0.0, 2.0, 0.0]],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
            ..MeshPart::default()
        };
        let mut d = Document::new(Mesh { materials: vec!["a".into(), "b".into()], parts: vec![part] });
        d.texture_sets.push(TextureSet::new("a", "A", (8, 8)));
        d.texture_sets.push(TextureSet::new("b", "B", (8, 8)));
        d
    }

    #[test]
    fn sets_without_an_instance_look_as_before() {
        let d = doc();
        let model = ModelScale::of(&d.mesh);
        assert_eq!(model.triangles, 2);
        assert!((model.diagonal - 20f32.sqrt()).abs() < 1e-5);
        let shading = part_shading(&d, &ShaderLibrary::default(), &model, &[Some(0), None]);
        assert_eq!(shading, vec![PartShading::default(), PartShading::default()]);
        assert_eq!(subdivisions(&d, &model), vec![Subdivision::default(); 2]);
        // an instance left at its defaults draws exactly like no instance: same program, same values
        let mut with = d.clone();
        let inst = ShaderInstance::default_of(&ShaderFile::builtin());
        ShaderEdit::Insert { at: 0, instance: Box::new(inst), sets: vec!["a".into(), "b".into()] }.apply(&mut with.shading).unwrap();
        assert_eq!(part_shading(&with, &ShaderLibrary::default(), &model, &[Some(0), Some(1)]), shading);
        assert_eq!(subdivisions(&with, &model), vec![Subdivision::default(); 2]);
    }

    #[test]
    fn instances_give_values_displacement_and_subdivision() {
        let mut d = doc();
        let file = ShaderFile::builtin();
        let mut inst = ShaderInstance::new("i", "I", &file);
        inst.set_value("ao_strength", Some(ParamValue::Float(0.25)));
        inst.displacement = Displacement { enabled: true, scale: 2.0, unit: DisplacementUnit::Centimetres, midlevel: 0.5, ..Displacement::default() };
        inst.tessellation = Tessellation { mode: TessellationMode::Uniform { levels: 2 }, smooth: true };
        ShaderEdit::Insert { at: 0, instance: Box::new(inst), sets: vec!["b".into()] }.apply(&mut d.shading).unwrap();
        d.shading.mesh_unit = Some(LengthUnit::Centimetre);
        let model = ModelScale::of(&d.mesh);
        let s = part_shading(&d, &ShaderLibrary::default(), &model, &[Some(0), Some(1)]);
        assert_eq!(s[0], PartShading::default());
        assert!(s[1].uniforms.iter().any(|u| u.uniform == "uAoStrength" && u.value == ParamValue::Float(0.25)));
        let disp = s[1].displacement.unwrap();
        // 2 cm on a centimetre mesh = 2 units, times the fit (radius sqrt(5) → 1)
        assert!((disp.distance - 2.0 / 5f32.sqrt()).abs() < 1e-4, "{}", disp.distance);
        assert_eq!(disp.level, 0.5);
        let sub = subdivisions(&d, &model);
        assert_eq!(sub[0], Subdivision::default());
        assert_eq!(sub[1], Subdivision { levels: 2, scheme: SubdivisionScheme::Loop });
    }
}

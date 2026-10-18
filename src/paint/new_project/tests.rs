//! End-to-end tests of new projects: OBJ and glTF files with several objects and materials, baked maps by name,
//! settings through `.mcpaint`.

use super::*;
use crate::camera::{SceneCamera, SceneLens};
use crate::mesh::{LengthUnit, Mesh, MeshPart};
use crate::paint::{BakedKind, Channel, LayerKind, NormalMapFormat, format};
use crate::tex::Tex;
use std::path::{Path, PathBuf};

/// Two objects; `Crate` uses Wood and Metal, `Barrel` uses Metal again and Rope. Wood and Rope overlap in UV space.
const OBJ: &str = "\
o Crate
v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0
vt 0 0\nvt 1 0\nvt 1 1\nvt 0 1
usemtl Wood
f 1/1 2/2 3/3 4/4
usemtl Metal
f 1/1 3/3 2/2
o Barrel
v 0 0 1\nv 1 0 1\nv 1 1 1
usemtl Metal
f 5/1 6/2 7/3
usemtl Rope
f 5/1 7/3 6/2
";

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> TempDir {
        let dir = std::env::temp_dir().join(format!("mc-new-project-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }

    fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let p = self.0.join(name);
        std::fs::write(&p, bytes).unwrap();
        p
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn png(c: [f32; 4]) -> Vec<u8> {
    crate::png::encode(16, 16, &Tex::solid(16, 16, c).px, 16)
}

fn settings(mesh: &Path) -> NewProject {
    NewProject::new(mesh, ProjectTemplate::builtins().remove(0))
}

fn names(created: &Created) -> Vec<(&str, usize)> {
    created.document.texture_sets.iter().map(|s| (s.name.as_str(), s.material_slot)).collect()
}

#[test]
fn obj_with_several_objects_gives_one_set_per_material() {
    let dir = TempDir::new("obj");
    let obj = dir.write("props.obj", OBJ.as_bytes());
    let normal = dir.write("Metal_normal.png", &png([0.5, 0.25, 1.0, 1.0]));
    let ao = dir.write("wood_ambient_occlusion.png", &png([0.75; 4]));
    let stray = dir.write("Glass_normal.png", &png([0.5, 0.5, 1.0, 1.0]));
    let mut s = settings(&obj);
    s.resolution = 512;
    s.normal_map_format = NormalMapFormat::DirectX;
    s.baked_maps = vec![normal, ao, stray];
    let created = create(&s, &mut |_, _| {}).unwrap();
    assert_eq!(names(&created), [("Wood", 0), ("Metal", 1), ("Rope", 2)]);
    let doc = &created.document;
    assert_eq!(doc.mesh.parts.len(), 4, "objects and materials keep their parts");
    for set in &doc.texture_sets {
        assert_eq!(set.resolution, (512, 512));
        assert_eq!(set.bake.common.size, 512);
        assert!(set.has_channel(Channel::BaseColor) && set.has_channel(Channel::Height));
        assert!(matches!(set.layers.as_slice(), [l] if matches!(l.kind, LayerKind::Paint(_))), "one empty paint layer");
    }
    let metal = doc.texture_set("metal").unwrap();
    let green = metal.baked(&BakedKind::Normal).unwrap().to_tex().px[0][1];
    assert!((green - 0.75).abs() < 0.01, "DirectX normal map stored as OpenGL: {green}");
    assert!(doc.texture_set("wood").unwrap().baked(&BakedKind::AmbientOcclusion).is_some(), "set names match ignoring case");
    assert!(doc.texture_set("rope").unwrap().baked.is_empty());
    assert!(created.notes.iter().any(|n| n.contains("Glass_normal.png") && n.contains("\"Glass\"")), "{:?}", created.notes);
    assert!(created.notes.iter().any(|n| n.contains("no unit")), "OBJ states no unit");
    assert_eq!(doc.shading.mesh_unit, None);
    assert_eq!(doc.project.normal_map_format, NormalMapFormat::DirectX);
}

fn triangle(name: &str, material: usize, z: f32) -> MeshPart {
    let positions = vec![[0.0, 0.0, z], [1.0, 0.0, z], [0.0, 1.0, z]];
    let uv = vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]];
    MeshPart { name: name.into(), material, positions, normals: vec![[0.0, 0.0, 1.0]; 3], uv_sets: vec![uv], triangles: vec![[0, 1, 2]], ..MeshPart::default() }
}

fn glb_with_camera() -> Vec<u8> {
    let mesh = Mesh {
        materials: vec!["Skin".into(), "Cloth".into(), "Unused".into()],
        parts: vec![triangle("head", 0, 0.0), triangle("shirt", 1, 0.5), triangle("hands", 0, 1.0)],
    };
    let cam = SceneCamera {
        name: "Shot".into(),
        eye: [0.5, 0.5, 4.0],
        forward: [0.0, 0.0, -1.0],
        up: [0.0, 1.0, 0.0],
        lens: SceneLens::Perspective { fov_y: 0.8 },
        aspect: None,
    };
    mesh.to_glb_with_cameras(&[cam]).unwrap()
}

#[test]
fn gltf_gives_one_set_per_used_material_cameras_and_the_unit() {
    let dir = TempDir::new("glb");
    let glb = dir.write("character.glb", &glb_with_camera());
    let created = create(&settings(&glb), &mut |_, _| {}).unwrap();
    assert_eq!(names(&created), [("Skin", 0), ("Cloth", 1)]);
    let doc = &created.document;
    assert_eq!(doc.shading.mesh_unit, Some(LengthUnit::Metre));
    let view = doc.view.as_ref().unwrap();
    assert_eq!(view.cameras.len(), 2, "default camera plus the file's");
    assert_eq!(view.cameras[1].name, "Shot");
    let mut no_cameras = settings(&glb);
    no_cameras.import_cameras = false;
    no_cameras.unit = UnitScale::Custom(LengthUnit::Centimetre);
    let plain = create(&no_cameras, &mut |_, _| {}).unwrap();
    assert_eq!(plain.document.view.as_ref().unwrap().cameras.len(), 1);
    assert_eq!(plain.document.shading.mesh_unit, Some(LengthUnit::Centimetre));
}

#[test]
fn a_mesh_without_uvs_is_refused_by_material() {
    let source = MeshSource::from_bytes(MeshFormat::Obj, b"v 0 0 0\nv 1 0 0\nv 0 1 0\nusemtl Plain\nf 1 2 3\n", false).unwrap();
    let s = settings(Path::new("x.obj"));
    let e = ProjectBuilder::new(&s).build(source).unwrap_err();
    assert_eq!(e, NewProjectError::NoUvs { materials: vec!["Plain".into()] });
    assert!(e.to_string().contains("\"Plain\""));
}

#[test]
fn settings_and_template_round_trip_through_mcpaint() {
    let source = MeshSource::from_bytes(MeshFormat::Glb, &glb_with_camera(), true).unwrap();
    let mut template = ProjectTemplate::builtins().remove(1);
    template.view.exposure_stops = 0.5;
    template.shader = Some(crate::shading::ShaderInstance::new("pbr", "PBR", &crate::shading::ShaderFile::builtin()));
    let mut s = NewProject::new("c.glb", template);
    s.fill_physical_size = true;
    s.resolution = 256;
    let created = ProjectBuilder::new(&s).build(source).unwrap();
    let doc = created.document;
    assert!(doc.project.tangent_per_fragment && doc.project.fill_physical_size);
    assert_eq!(doc.project.normal_map_format, NormalMapFormat::DirectX);
    assert!(doc.texture_sets.iter().all(|t| doc.shading.assigned(&t.id) == Some("pbr") && t.has_channel(Channel::Opacity)));
    let mut bytes = Vec::new();
    format::save(&doc, &mut bytes).unwrap();
    let back = format::load(std::io::Cursor::new(bytes)).unwrap();
    assert_eq!(back.project, doc.project);
    assert_eq!(back.shading, doc.shading);
    assert_eq!(back.view, doc.view);
    assert_eq!(back.texture_sets, doc.texture_sets);
    // the document's defaults as a template, and back
    let t = ProjectTemplate::from_document(&back, "Mine").unwrap();
    assert_eq!(format::template::from_text(&format::template::to_text(&t)).unwrap(), t);
    assert_eq!((t.set.resolution, t.view.exposure_stops, t.project), ((256, 256), 0.5, doc.project));
}

#[test]
fn physically_sized_projects_drop_physically_sized_fills() {
    use crate::paint::edit::LayerSlot;
    use crate::paint::edit::ops::{DropPayload, DropTarget, MaterialAsset, drop_onto};
    use crate::projection::SizeMode;
    for physical in [false, true] {
        let source = MeshSource::from_bytes(MeshFormat::Glb, &glb_with_camera(), false).unwrap();
        let mut s = settings(Path::new("c.glb"));
        s.fill_physical_size = physical;
        let mut doc = ProjectBuilder::new(&s).build(source).unwrap().document;
        let mut tx = crate::paint::edit::Transaction::new(&mut doc);
        let target = DropTarget::Between { set: "skin".into(), at: LayerSlot::top(1) };
        let payload = DropPayload::Material(MaterialAsset { name: "Rust".into(), path: "materials/rust.mca".into(), params: Vec::new() });
        drop_onto(&mut tx, &target, &payload, false).unwrap();
        tx.commit();
        let fill = doc.texture_set("skin").unwrap().layers.iter().find_map(|l| match &l.kind {
            LayerKind::Fill(f) => Some(f.projection.image.size_mode),
            _ => None,
        });
        assert_eq!(fill, Some(if physical { SizeMode::Physical } else { SizeMode::Tiling }));
    }
}

/// A real mesh end to end: `MC_NEW_PROJECT_MESH` names an OBJ or `.glb` file (e.g. a high-poly mesh of the corpus).
/// Makes the project, saves it as `.mcpaint`, loads it back and prints what it got.
#[test]
fn real_mesh_end_to_end() {
    let Some(path) = std::env::var_os("MC_NEW_PROJECT_MESH") else { return };
    let start = std::time::Instant::now();
    let created = create(&settings(Path::new(&path)), &mut |_, _| {}).unwrap();
    let doc = &created.document;
    let triangles: usize = doc.mesh.parts.iter().map(|p| p.triangles.len()).sum();
    let mut bytes = Vec::new();
    format::save(doc, &mut bytes).unwrap();
    let back = format::load(std::io::Cursor::new(&bytes)).unwrap();
    assert_eq!(back.texture_sets, doc.texture_sets);
    assert_eq!(back.project, doc.project);
    let sets: Vec<&str> = doc.texture_sets.iter().map(|s| s.name.as_str()).collect();
    eprintln!(
        "{}: {} parts, {triangles} triangles, sets {sets:?}, unit {:?}, {} bytes saved, {:.1} s; notes {:?}",
        Path::new(&path).display(),
        doc.mesh.parts.len(),
        doc.shading.mesh_unit,
        bytes.len(),
        start.elapsed().as_secs_f64(),
        created.notes
    );
}

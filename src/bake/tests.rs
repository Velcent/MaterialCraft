//! End-to-end tests of the baker on synthetic meshes.

use super::*;
use crate::diag::{Entry, Level, Sink};
use crate::mesh::{self, MeshPart};
use std::sync::Mutex;

#[derive(Default)]
struct Log(Mutex<Vec<Entry>>);

impl Sink for Log {
    fn emit(&self, entry: Entry) {
        if let Ok(mut v) = self.0.lock() {
            v.push(entry);
        }
    }
}

impl Log {
    fn levels(&self) -> Vec<Level> {
        self.0.lock().map(|v| v.iter().map(|e| e.level).collect()).unwrap_or_default()
    }
}

/// A square in the z = `z` plane covering UV 0..1, facing +z.
fn plane(z: f32, material: usize) -> MeshPart {
    MeshPart {
        name: format!("plane{z}"),
        material,
        positions: vec![[-1.0, -1.0, z], [1.0, -1.0, z], [1.0, 1.0, z], [-1.0, 1.0, z]],
        normals: vec![[0.0, 0.0, 1.0]; 4],
        uv_sets: vec![vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]],
        triangles: vec![[0, 1, 2], [0, 2, 3]],
        ..MeshPart::default()
    }
}

fn settings(size: usize) -> BakeSettings {
    BakeSettings { size, ao_rays: 8, padding: 2, ..BakeSettings::default() }
}

fn centre(t: &Tex) -> [f32; 4] {
    t.px[(t.h / 2) * t.w + t.w / 2]
}

#[test]
fn low_as_high_plane_is_flat_and_open() {
    let low = mesh::Mesh { materials: vec!["m".into()], parts: vec![plane(0.0, 0)] };
    let high = HighScene::from_mesh(&low);
    let log = Log::default();
    let maps = bake_set(&low, &TextureSetRef::slot(0), &high, &settings(32), &CancelToken::new(), &log).unwrap();
    assert_eq!(maps.maps.len(), 7);
    let n = centre(maps.get(MapKind::Normal).unwrap());
    assert!((n[0] - 0.5).abs() < 1e-3 && (n[1] - 0.5).abs() < 1e-3 && n[2] > 0.999, "{n:?}");
    assert!(centre(maps.get(MapKind::AmbientOcclusion).unwrap())[0] > 0.999);
    let c = centre(maps.get(MapKind::Curvature).unwrap())[0];
    assert!((c - 0.5).abs() < 1e-3, "{c}");
    let w = centre(maps.get(MapKind::WorldNormal).unwrap());
    assert!(w[2] > 0.999);
    // uniform position over the scene: x and y span 0..1, z (flat) sits at 0.5
    let p = centre(maps.get(MapKind::Position).unwrap());
    assert!((p[0] - 0.5).abs() < 0.05 && (p[2] - 0.5).abs() < 1e-3, "{p:?}");
    assert!(maps.hit_mask.iter().all(|&h| h > 0.99));
    assert!(log.levels().contains(&Level::Info) && !log.levels().contains(&Level::Warning));
}

#[test]
fn front_and_rear_distances_decide_what_is_found() {
    let low = mesh::Mesh { materials: vec!["m".into()], parts: vec![plane(0.0, 0)] };
    let mut tilted = plane(0.0, 0);
    // a high plane slightly below the low one, tilted so its normal differs
    tilted.positions = vec![[-1.0, -1.0, -0.22], [1.0, -1.0, -0.02], [1.0, 1.0, -0.02], [-1.0, 1.0, -0.22]];
    tilted.normals.clear(); // flat normals from the faces
    let high = HighScene::from_mesh(&mesh::Mesh { materials: vec!["m".into()], parts: vec![tilted] });
    let set = TextureSetRef::slot(0);
    let near = BakeSettings { maps: MapSelection::of(&[MapKind::Normal]), rear_dist: Some(0.001), ..settings(16) };
    let log = Log::default();
    let missed = bake_set(&low, &set, &high, &near, &CancelToken::new(), &log).unwrap();
    assert!(missed.hit_mask.iter().all(|&h| h == 0.0));
    assert!(log.levels().contains(&Level::Warning), "few hits are reported");
    let far = BakeSettings { rear_dist: Some(0.1), ..near };
    let found = bake_set(&low, &set, &high, &far, &CancelToken::new(), &Log::default()).unwrap();
    assert_eq!(found.maps.len(), 1);
    assert!(found.hit_mask.iter().filter(|&&h| h > 0.99).count() > 200);
    let red = centre(found.get(MapKind::Normal).unwrap())[0];
    assert!(red < 0.49, "tilted normal shows in red: {red}");
}

#[test]
fn occluder_darkens_ao_and_cancel_stops() {
    let low = mesh::Mesh { materials: vec!["m".into()], parts: vec![plane(0.0, 0)] };
    let mut roof = plane(0.05, 1);
    roof.normals = vec![[0.0, 0.0, -1.0]; 4];
    roof.triangles = vec![[0, 2, 1], [0, 3, 2]];
    let both = mesh::Mesh { materials: vec!["m".into(), "roof".into()], parts: vec![plane(0.0, 0), roof] };
    let high = HighScene::from_mesh(&both);
    let s = BakeSettings { maps: MapSelection::of(&[MapKind::AmbientOcclusion, MapKind::Id]), ..settings(16) };
    let maps = bake_set(&low, &TextureSetRef::slot(0), &high, &s, &CancelToken::new(), &NullSink).unwrap();
    assert!(centre(maps.get(MapKind::AmbientOcclusion).unwrap())[0] < 0.5);
    let cancel = CancelToken::new();
    cancel.cancel();
    let log = Log::default();
    assert_eq!(bake_set(&low, &TextureSetRef::slot(0), &high, &s, &cancel, &log).unwrap_err(), BakeError::Cancelled);
}

#[test]
fn errors() {
    let low = mesh::Mesh { materials: vec!["m".into()], parts: vec![plane(0.0, 0)] };
    let high = HighScene::from_mesh(&low);
    let none = BakeSettings { maps: MapSelection::none(), ..settings(16) };
    assert_eq!(bake_set(&low, &TextureSetRef::slot(0), &high, &none, &CancelToken::new(), &NullSink).unwrap_err(), BakeError::NothingToBake);
    assert_eq!(bake_set(&low, &TextureSetRef::slot(3), &high, &settings(16), &CancelToken::new(), &NullSink).unwrap_err(), BakeError::NoLowTriangles);
    let mut flat = low.clone();
    flat.parts[0].uv_sets[0] = vec![[0.0, 0.0]; 4];
    assert_eq!(bake_set(&flat, &TextureSetRef::slot(0), &high, &settings(16), &CancelToken::new(), &NullSink).unwrap_err(), BakeError::NoUvs);
    let empty = HighScene::from_mesh(&mesh::Mesh::default());
    let log = Log::default();
    assert_eq!(bake_set(&low, &TextureSetRef::slot(0), &empty, &settings(16), &CancelToken::new(), &log).unwrap_err(), BakeError::NoHighTriangles);
    assert!(log.levels().contains(&Level::Error));
}

#[test]
fn obj_soup_entry_point_and_supersampling() {
    let low = parse_obj(&uv_sphere_obj(12, 8, 1.0, 0.0)).unwrap();
    let high = parse_obj(&uv_sphere_obj(48, 32, 1.0, 0.05)).unwrap();
    let s = BakeSettings { samples: 2, ..settings(32) };
    let maps = bake(&low, &high, &s).unwrap();
    assert_eq!(maps.len(), 7);
    assert!(maps.values().all(|t| t.w == 32 && t.h == 32));
    let n = &maps["normal"];
    assert!(n.px.iter().all(|p| p[2] > 0.3));
    assert!(n.px.iter().any(|p| (p[0] - 0.5).abs() > 0.02));
}

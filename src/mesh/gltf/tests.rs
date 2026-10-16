use super::*;
use crate::mesh::test_util::quad;
use crate::mesh::MeshPart;

/// A unit cube with per-face normals and tangents and two UV sets.
fn cube(name: &str, material: usize) -> MeshPart {
    let mut p = MeshPart {
        name: name.into(),
        material,
        uv_sets: vec![vec![], vec![]],
        ..MeshPart::default()
    };
    // (normal, tangent): bitangent = cross(n, t) points along -v.
    let faces: [([f32; 3], [f32; 3]); 6] = [
        ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0]),
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0]),
        ([0.0, 0.0, -1.0], [-1.0, 0.0, 0.0]),
    ];
    for (f, (n, t)) in faces.iter().enumerate() {
        let b = [
            n[1] * t[2] - n[2] * t[1],
            n[2] * t[0] - n[0] * t[2],
            n[0] * t[1] - n[1] * t[0],
        ];
        let base = p.positions.len() as u32;
        for (u, v) in [(0.0f32, 1.0f32), (1.0, 1.0), (1.0, 0.0), (0.0, 0.0)] {
            let (su, sv) = (u * 2.0 - 1.0, 1.0 - v * 2.0);
            p.positions.push(std::array::from_fn(|i| {
                0.5 * (n[i] + su * t[i] + sv * b[i])
            }));
            p.normals.push(*n);
            p.tangents.push([t[0], t[1], t[2], 1.0]);
            p.uv_sets[0].push([u, v]);
            p.uv_sets[1].push([(f as f32 + u) / 6.0, v]);
        }
        p.triangles.push([base, base + 1, base + 2]);
        p.triangles.push([base, base + 2, base + 3]);
    }
    p
}

pub(crate) fn sample() -> Mesh {
    let mut floor = quad();
    floor.name = "Floor".into();
    floor.material = 1;
    Mesh {
        materials: vec!["Body".into(), "Ground plane".into(), "Unused".into()],
        parts: vec![
            cube("Box", 0),
            floor,
            MeshPart {
                name: "empty".into(),
                material: 2,
                ..MeshPart::default()
            },
            MeshPart {
                name: "points only".into(),
                material: 0,
                positions: vec![[0.0, 2.0, 0.0]],
                uv_sets: vec![vec![[0.5, 0.5]]],
                ..MeshPart::default()
            },
        ],
    }
}

#[test]
fn round_trip_is_exact_and_deterministic() {
    let mesh = sample();
    let glb = mesh.to_glb().unwrap();
    assert_eq!(glb, mesh.to_glb().unwrap());
    assert_eq!(Mesh::from_glb(&glb).unwrap(), mesh);
    let empty = Mesh::default();
    assert_eq!(Mesh::from_glb(&empty.to_glb().unwrap()).unwrap(), empty);
    if let Some(dir) = std::env::var_os("MATERIALCRAFT_GLB_OUT") {
        std::fs::write(std::path::Path::new(&dir).join("sample.glb"), &glb).unwrap();
    }
}

#[test]
fn json_names_slots_and_bounds() {
    let glb = sample().to_glb().unwrap();
    let g = glb::Glb::parse(&glb).unwrap();
    let root = json::parse(std::str::from_utf8(g.json).unwrap()).unwrap();
    assert_eq!(root["asset"]["version"].as_str(), Some("2.0"));
    assert_eq!(root["nodes"][1]["name"].as_str(), Some("Floor"));
    assert_eq!(root["materials"][1]["name"].as_str(), Some("Ground plane"));
    assert_eq!(
        root["meshes"][1]["primitives"][0]["material"].as_u64(),
        Some(1)
    );
    let pos = &root["accessors"][0];
    assert_eq!(pos["min"][0].as_f64(), Some(-0.5));
    assert_eq!(pos["max"][2].as_f64(), Some(0.5));
    assert!(root["meshes"][0]["primitives"][0]["attributes"]
        .get("TEXCOORD_1")
        .is_some());
}

#[test]
fn rejects_invalid_meshes_and_unsupported_files() {
    let mut bad = sample();
    bad.parts[0].material = 9;
    assert!(matches!(bad.to_glb(), Err(GltfError::Mesh(_))));
    let glb = |json: &str| glb::Glb::assemble(json, &[0; 64]).unwrap();
    let unsupported = [
        r#"{"asset":{"version":"2.0"},"extensionsRequired":["KHR_draco_mesh_compression"]}"#,
        r#"{"asset":{"version":"1.0"}}"#,
        r#"{"asset":{"version":"2.0"},"buffers":[{"uri":"a.bin","byteLength":4}]}"#,
    ];
    for j in unsupported {
        assert!(
            matches!(Mesh::from_glb(&glb(j)), Err(GltfError::Unsupported(_))),
            "{j}"
        );
    }
    let lines = r#"{"asset":{"version":"2.0"},"buffers":[{"byteLength":64}],
        "bufferViews":[{"buffer":0,"byteLength":36}],
        "accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3"}],
        "meshes":[{"primitives":[{"attributes":{"POSITION":0},"mode":1}]}],
        "nodes":[{"mesh":0}]}"#;
    assert!(matches!(
        Mesh::from_glb(&glb(lines)),
        Err(GltfError::Unsupported(_))
    ));
    let out_of_range = lines
        .replace("\"count\":3", "\"count\":30")
        .replace(",\"mode\":1", "");
    assert!(matches!(
        Mesh::from_glb(&glb(&out_of_range)),
        Err(GltfError::Invalid { .. })
    ));
    let cyclic = lines.replace(",\"mode\":1", "").replace(
        "\"nodes\":[{\"mesh\":0}]",
        "\"scenes\":[{\"nodes\":[0]}],\"nodes\":[{\"mesh\":0,\"children\":[0]}]",
    );
    assert!(Mesh::from_glb(&glb(&cyclic)).is_err());
}

#[test]
fn reads_hierarchies_strips_and_missing_materials() {
    // Two triangles as a strip under a translated parent, no material.
    let mut bin = Vec::new();
    for v in [
        [0.0f32, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 1.0, 0.0],
    ] {
        v.iter()
            .for_each(|f| bin.extend_from_slice(&f.to_le_bytes()));
    }
    bin.extend_from_slice(&[0, 1, 2, 3]);
    let json = r#"{"asset":{"version":"2.0"},"scene":0,"scenes":[{"nodes":[0]}],
        "buffers":[{"byteLength":52}],
        "bufferViews":[{"buffer":0,"byteLength":48},{"buffer":0,"byteOffset":48,"byteLength":4}],
        "accessors":[{"bufferView":0,"componentType":5126,"count":4,"type":"VEC3"},
                     {"bufferView":1,"componentType":5121,"count":4,"type":"SCALAR"}],
        "meshes":[{"name":"strip","primitives":[{"attributes":{"POSITION":0},"indices":1,"mode":5}]}],
        "nodes":[{"name":"parent","translation":[0,0,5],"children":[1]},{"mesh":0}]}"#;
    let mesh = Mesh::from_glb(&glb::Glb::assemble(json, &bin).unwrap()).unwrap();
    assert_eq!(mesh.materials, ["default"]);
    let p = &mesh.parts[0];
    assert_eq!(p.name, "strip");
    assert_eq!(p.triangles, [[0, 1, 2], [2, 1, 3]]);
    assert_eq!(p.positions[3], [1.0, 1.0, 5.0]);
}

#[test]
fn corrupt_files_error_without_panicking() {
    let glb = sample().to_glb().unwrap();
    for n in (0..glb.len()).step_by(7) {
        assert!(Mesh::from_glb(&glb[..n]).is_err(), "prefix {n}");
    }
    let mut s = 1u32;
    for _ in 0..2000 {
        let mut b = glb.clone();
        for _ in 0..4 {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let i = (s >> 8) as usize % b.len();
            b[i] ^= (s >> 3) as u8 | 1;
        }
        let _ = Mesh::from_glb(&b);
    }
}

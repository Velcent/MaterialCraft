use materialcraft::render3d::{SHAPES, build_mesh, shape_culls};
use materialcraft::v3::*;

#[test]
fn every_shape_builds_a_consistent_mesh() {
    for (name, id) in SHAPES {
        let m = build_mesh(id, 24, 1.0, None, 0.0, 0.0);
        assert!(!m.tris.is_empty(), "{name}: no triangles");
        let nv = m.pos.len();
        assert!(m.nrm.len() == nv && m.tan.len() == nv && m.uv.len() == nv && m.fade.len() == nv, "{name}: attribute length mismatch");
        let mut agree = 0usize;
        let mut total = 0usize;
        for t in &m.tris {
            for &i in t {
                assert!(i < nv as u32, "{name}: index out of range");
            }
            let (a, b, c) = (m.pos[t[0] as usize], m.pos[t[1] as usize], m.pos[t[2] as usize]);
            let g = cross(sub(b, a), sub(c, a));
            if len(g) < 1e-5 {
                continue;
            }
            let n = add(add(m.nrm[t[0] as usize], m.nrm[t[1] as usize]), m.nrm[t[2] as usize]);
            total += 1;
            if dot(g, n) > 0.0 {
                agree += 1;
            }
        }
        assert!(total > 0, "{name}: only degenerate triangles");
        assert!(agree as f32 >= 0.97 * total as f32, "{name}: winding disagrees with normals ({agree}/{total})");
        for i in 0..nv {
            assert!(m.pos[i].iter().all(|x| x.is_finite() && x.abs() < 3.0), "{name}: vertex {i} out of range {:?}", m.pos[i]);
            assert!((len(m.nrm[i]) - 1.0).abs() < 1e-3, "{name}: normal {i} not unit");
            assert!(m.uv[i][0].is_finite() && m.uv[i][1].is_finite() && m.su[i] > 0.0 && m.sv[i] > 0.0, "{name}: bad uv/scale at {i}");
        }
    }
    assert!(shape_culls(5) && !shape_culls(1));
}

#[test]
fn closed_shapes_point_outwards() {
    for id in [0u32, 1, 3, 9, 10, 11, 12] {
        let m = build_mesh(id, 24, 1.0, None, 0.0, 0.0);
        let bad = (0..m.pos.len()).filter(|&i| dot(m.nrm[i], m.pos[i]) < -1e-3 && id != 12).count();
        assert!(bad == 0, "shape {id}: {bad} normals point inwards");
    }
}

// SPDX-License-Identifier: MIT OR Apache-2.0
//! Software PBR renderer for the 3D viewer. Every shape (sphere, cube, plane) is a real tessellated triangle mesh that is
//! rasterised with a depth buffer, so height maps can displace actual geometry (true displacement), not just fake it.
//! GGX specular, hemisphere environment lighting, tangent-space normal mapping, bump, outward parallax relief, wireframe.
use crate::camera::{Basis, Camera, CameraSettings, Orbit};
use crate::tex::{Tex, lum};
use crate::v3::*;
use std::f32::consts::{PI, TAU};

pub struct Maps<'a> {
    pub base: &'a Tex,
    pub normal: &'a Tex,
    pub rough: &'a Tex,
    pub metal: &'a Tex,
    pub ao: &'a Tex,
    pub emissive: Option<&'a Tex>,
    pub height: Option<&'a Tex>,
}

/// How the height map is shown.
pub const RELIEF_NONE: u32 = 0; // normal map only
pub const RELIEF_BUMP: u32 = 1; // normals derived from the height map
pub const RELIEF_PARALLAX: u32 = 2; // per-pixel relief that rises OUT of the surface
pub const RELIEF_DISPLACE: u32 = 3; // real geometry displacement along the surface normal

#[derive(Clone, Debug, PartialEq)]
pub struct View {
    pub yaw: f32,
    pub pitch: f32,
    pub dist: f32,
    pub pan_x: f32,
    pub pan_y: f32,
    pub light_yaw: f32,
    pub light_pitch: f32,
    pub light_intensity: f32,
    /// The light is placed relative to the camera (yaw/pitch are offsets from the view direction), so the object stays
    /// lit while it is orbited. Off: the light is fixed in the scene.
    pub light_follows_camera: bool,
    pub exposure: f32,
    pub ambient: f32,
    pub shape: u32, // see SHAPES: 0 sphere, 1 cube, 2 plane, 3.. more (all tessellated meshes)
    pub tiling: f32,
    /// One of the RELIEF_* modes.
    pub relief: u32,
    /// Height amount in object units: displacement distance or parallax depth. 0 = off.
    pub height_scale: f32,
    /// Strength of normals derived from the height map (RELIEF_BUMP). 0 = off.
    pub bump: f32,
    /// Height value that sits ON the original surface. 0 = everything rises outward, 0.5 = centred relief.
    pub height_level: f32,
    /// Strength of the graph's normal map (0..1).
    pub normal_amt: f32,
    /// Mesh resolution: grid segments along one side of the plane (sphere uses 2x1, cube faces 5/8).
    pub tess: u32,
    pub wireframe: bool,
}

impl Default for View {
    fn default() -> Self {
        View {
            yaw: 0.5,
            pitch: 0.3,
            dist: 3.4,
            pan_x: 0.0,
            pan_y: 0.0,
            light_yaw: 0.9,
            light_pitch: 0.7,
            light_intensity: 3.0,
            light_follows_camera: true,
            exposure: 1.0,
            ambient: 1.0,
            shape: 0,
            tiling: 1.0,
            relief: RELIEF_DISPLACE,
            height_scale: 0.08,
            bump: 1.0,
            height_level: 0.0,
            normal_amt: 1.0,
            tess: 256,
            wireframe: false,
        }
    }
}

impl View {
    /// The camera pose as an orbit controller.
    pub fn orbit(&self) -> Orbit {
        Orbit { yaw: self.yaw, pitch: self.pitch, distance: self.dist, pan: [self.pan_x, self.pan_y] }
    }

    /// Takes the camera pose from an orbit controller.
    pub fn set_orbit(&mut self, o: &Orbit) {
        (self.yaw, self.pitch, self.dist, [self.pan_x, self.pan_y]) = (o.yaw, o.pitch, o.distance, o.pan);
    }

    /// The camera drawing this view through `settings` into a viewport of `viewport` units.
    pub fn camera(&self, settings: &CameraSettings, viewport: [f32; 2]) -> Camera {
        settings.camera(&self.orbit(), viewport)
    }
}

/// Key light direction and the opposite soft fill light (unit vectors pointing TO the lights).
pub fn light_dirs(v: &View) -> (V, V) {
    let (ly, lp) = (v.light_yaw, v.light_pitch);
    let (lsy, lcy) = ly.sin_cos();
    let (lsp, lcp) = lp.sin_cos();
    let local = [lsy * lcp, lsp, lcy * lcp];
    let ldir = if v.light_follows_camera {
        // the camera's axes (they don't depend on distance or pan)
        let b = Orbit { distance: 1.0, pan: [0.0, 0.0], ..v.orbit() }.basis();
        norm(add(add(mul(b.right, local[0]), mul(b.up, local[1])), mul(b.forward, -local[2])))
    } else {
        norm(local)
    };
    // the fill comes from the opposite side, a little lower, so shadowed sides never go black
    let fill = norm([-ldir[0] * 0.7, -0.35 * ldir[1].signum() * 0.5 - 0.15, -ldir[2] * 0.7]);
    (ldir, fill)
}

// ------------------------------------------------------------------------------------------------ mesh
pub struct Mesh {
    pub pos: Vec<V>,
    pub nrm: Vec<V>,
    pub tan: Vec<V>,
    pub uv: Vec<[f32; 2]>,
    /// object-space length of one uv unit along u and v (used by bump and parallax)
    pub su: Vec<f32>,
    pub sv: Vec<f32>,
    /// 0..1 displacement fade per vertex (0 at sphere poles, ramps to 0 at cube edges) so displaced meshes stay closed
    pub fade: Vec<f32>,
    /// normals of the undisplaced surface (used to decide which walls face the camera)
    pub base_nrm: Vec<V>,
    pub tris: Vec<[u32; 3]>,
}

struct Raw {
    pos: Vec<V>,
    nrm: Vec<V>,
    tan: Vec<V>,
    uv: Vec<[f32; 2]>,
    su: Vec<f32>,
    sv: Vec<f32>,
    tris: Vec<[u32; 3]>,
    /// vertices with equal weld ids sit at the same place (seams, poles) and must displace and shade identically
    weld: Vec<u32>,
    /// 0..1 displacement fade (cube edges stay closed)
    fade: Vec<f32>,
}

impl Raw {
    fn new() -> Raw {
        Raw { pos: vec![], nrm: vec![], tan: vec![], uv: vec![], su: vec![], sv: vec![], tris: vec![], weld: vec![], fade: vec![] }
    }
    #[allow(clippy::too_many_arguments)]
    fn push(&mut self, p: V, n: V, t: V, uv: [f32; 2], su: f32, sv: f32, weld: u32, fade: f32) {
        self.pos.push(p);
        self.nrm.push(n);
        self.tan.push(t);
        self.uv.push(uv);
        self.su.push(su);
        self.sv.push(sv);
        self.weld.push(weld);
        self.fade.push(fade);
    }
    /// Triangulate a (nu+1) x (nv+1) vertex grid starting at vertex `first`. Winding is outward for all shapes.
    fn grid_tris(&mut self, first: u32, nu: u32, nv: u32) {
        let stride = nu + 1;
        for j in 0..nv {
            for i in 0..nu {
                let a = first + j * stride + i;
                let (b, c, d) = (a + stride, a + 1, a + stride + 1);
                self.tris.push([a, b, c]);
                self.tris.push([c, b, d]);
            }
        }
    }
}

const PLANE_HALF: f32 = 1.3;
const CUBE_HALF: f32 = 0.8;

fn smoothstep01(x: f32) -> f32 {
    let t = x.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn raw_plane(n: u32, tiling: f32) -> Raw {
    let mut r = Raw::new();
    let size = PLANE_HALF * 2.0;
    for j in 0..=n {
        for i in 0..=n {
            let (u0, v0) = (i as f32 / n as f32, j as f32 / n as f32);
            let p = [(u0 - 0.5) * size, (0.5 - v0) * size, 0.0];
            r.push(p, [0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [u0 * tiling, v0 * tiling], size / tiling, size / tiling, r.pos.len() as u32, 1.0);
        }
    }
    r.grid_tris(0, n, n);
    r
}

fn raw_sphere(n: u32, tiling: f32) -> Raw {
    let (nu, nv) = (n * 2, n);
    let mut r = Raw::new();
    for j in 0..=nv {
        let theta = j as f32 / nv as f32 * PI;
        let (st, ct) = theta.sin_cos();
        for i in 0..=nu {
            let lon = i as f32 / nu as f32 * TAU - PI;
            let (sl, cl) = lon.sin_cos();
            let p = [st * sl, ct, st * cl];
            // seam column (i == nu) welds to column 0; both poles collapse to one id per pole
            let weld = if j == 0 {
                0
            } else if j == nv {
                1
            } else {
                2 + j * nu + (i % nu)
            };
            let sin_t = st.max(0.05);
            r.push(
                p,
                p,
                [cl, 0.0, -sl],
                [(i as f32 / nu as f32) * tiling * 2.0, theta / PI * tiling],
                TAU * sin_t / (tiling * 2.0),
                PI / tiling,
                weld,
                smoothstep01(st / 0.05),
            );
        }
    }
    r.grid_tris(0, nu, nv);
    r
}

fn raw_cube(n: u32, tiling: f32) -> Raw {
    let mut r = Raw::new();
    let h = CUBE_HALF;
    // (normal, tangent, bitangent) per face; tangent x bitangent = normal
    let faces: [(V, V, V); 6] = [
        ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
    ];
    for (nrm, tg, bt) in faces {
        let first = r.pos.len() as u32;
        for j in 0..=n {
            for i in 0..=n {
                let (u0, v0) = (i as f32 / n as f32, j as f32 / n as f32);
                let p = add(add(mul(nrm, h), mul(tg, (u0 - 0.5) * 2.0 * h)), mul(bt, (0.5 - v0) * 2.0 * h));
                let edge = u0.min(1.0 - u0).min(v0).min(1.0 - v0);
                r.push(p, nrm, tg, [u0 * tiling, v0 * tiling], 2.0 * h / tiling, 2.0 * h / tiling, r.pos.len() as u32, smoothstep01(edge / 0.02));
            }
        }
        r.grid_tris(first, n, n);
    }
    r
}

// ------------------------------------------------------------------------------------------------ more shapes
/// Shapes the viewer can show, in menu order: (name, shape id). Ids 0..2 are the original sphere, cube and plane.
pub const SHAPES: [(&str, u32); 13] = [
    ("Cube", 1),
    ("Cylinder", 3),
    ("Hollow box", 4),
    ("Inner box", 5),
    ("Plane", 2),
    ("Plane (hi-res)", 6),
    ("Sphere", 0),
    ("Cloth", 7),
    ("Mat ball", 8),
    ("Rounded cube", 9),
    ("Rounded cylinder", 10),
    ("Sphere 2 tiles", 11),
    ("Torus", 12),
];

pub fn shape_name(id: u32) -> &'static str {
    SHAPES.iter().find(|s| s.1 == id).map(|s| s.0).unwrap_or("Sphere")
}

/// Shapes viewed from the inside: faces turned away from the camera are not drawn, so the far walls show.
pub fn shape_culls(id: u32) -> bool {
    id == 5
}

/// Give vertices that sit at the same place the same weld id, so displacement and shading stay closed across seams.
fn weld_by_position(r: &mut Raw, smooth: bool) {
    let mut ids: std::collections::HashMap<[i32; 3], u32> = std::collections::HashMap::new();
    for i in 0..r.pos.len() {
        let p = r.pos[i];
        let k = [(p[0] * 4000.0).round() as i32, (p[1] * 4000.0).round() as i32, (p[2] * 4000.0).round() as i32];
        let next = ids.len() as u32;
        r.weld[i] = *ids.entry(k).or_insert(next);
    }
    if smooth {
        // one shared normal per spot, so smooth surfaces stay crack-free when displaced
        let mut acc = vec![[0.0f32; 3]; ids.len()];
        for i in 0..r.pos.len() {
            acc[r.weld[i] as usize] = add(acc[r.weld[i] as usize], r.nrm[i]);
        }
        for i in 0..r.pos.len() {
            let a = acc[r.weld[i] as usize];
            if dot(a, a) > 1e-6 {
                r.nrm[i] = norm(a);
            }
        }
    }
}

/// A (nu+1) x (nv+1) grid sampled from a parametric surface `f(u, v)`, u and v in 0..1. Normals and tangents come from
/// finite differences. The surface must be oriented so that dP/dv x dP/du points to the visible side (the plane
/// convention: u runs right, v runs down). `fade_edge` > 0 ramps the displacement to zero at the patch borders.
fn patch<F: Fn(f32, f32) -> V>(r: &mut Raw, nu: u32, nv: u32, tiling: f32, uvs: (f32, f32), fade_edge: f32, f: F) {
    let first = r.pos.len() as u32;
    let (eu, ev) = (0.5 / nu as f32, 0.5 / nv as f32);
    for j in 0..=nv {
        for i in 0..=nu {
            let (u, v) = (i as f32 / nu as f32, j as f32 / nv as f32);
            let (u0, u1) = ((u - eu).max(0.0), (u + eu).min(1.0));
            let (v0, v1) = ((v - ev).max(0.0), (v + ev).min(1.0));
            let dpu = mul(sub(f(u1, v), f(u0, v)), 1.0 / (u1 - u0));
            let dpv = mul(sub(f(u, v1), f(u, v0)), 1.0 / (v1 - v0));
            let n = norm(cross(dpv, dpu));
            let t = norm(dpu);
            let edge = u.min(1.0 - u).min(v).min(1.0 - v);
            let fade = if fade_edge > 0.0 { smoothstep01(edge / fade_edge) } else { 1.0 };
            let su = (len(dpu) / (uvs.0 * tiling)).max(0.05);
            let sv = (len(dpv) / (uvs.1 * tiling)).max(0.05);
            r.push(f(u, v), n, t, [u * uvs.0 * tiling, v * uvs.1 * tiling], su, sv, 0, fade);
        }
    }
    r.grid_tris(first, nu, nv);
}

/// One flat rectangle: centre `c`, axes `tg` (u) and `bt` (v, running down), half sizes `hu`, `hv`. tg x bt is the normal.
fn quad(r: &mut Raw, n: u32, tiling: f32, c: V, tg: V, bt: V, hu: f32, hv: f32, fade_edge: f32) {
    let base = 1.6; // object length one uv tile spans at tiling 1
    let nu = ((n as f32 * hu / 0.8).ceil() as u32).max(2);
    let nv = ((n as f32 * hv / 0.8).ceil() as u32).max(2);
    patch(r, nu, nv, tiling, (2.0 * hu / base, 2.0 * hv / base), fade_edge, |u, v| add(add(c, mul(tg, (u - 0.5) * 2.0 * hu)), mul(bt, (0.5 - v) * 2.0 * hv)));
}

/// A revolved profile around the Y axis. `segs` are polylines/curves (radius, y) given so that the outward normal is the
/// profile direction rotated to the left; the result is welded along seams and between segments.
struct Seg {
    pts: Vec<[f32; 2]>,
    /// ramp the displacement to zero at the start / end (hard edges)
    fade0: bool,
    fade1: bool,
}

fn seg_line(a: [f32; 2], b: [f32; 2], steps: usize, fade0: bool, fade1: bool) -> Seg {
    Seg {
        pts: (0..=steps)
            .map(|i| {
                let t = i as f32 / steps as f32;
                [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
            })
            .collect(),
        fade0,
        fade1,
    }
}

fn seg_arc(c: [f32; 2], rad: f32, a0: f32, a1: f32, steps: usize, fade0: bool, fade1: bool) -> Seg {
    Seg {
        pts: (0..=steps)
            .map(|i| {
                let a = a0 + (a1 - a0) * i as f32 / steps as f32;
                [c[0] + rad * a.cos(), c[1] + rad * a.sin()]
            })
            .collect(),
        fade0,
        fade1,
    }
}

fn revolve(r: &mut Raw, n: u32, tiling: f32, ku: f32, segs: &[Seg]) {
    let nu = n * 2;
    // arc length along the whole profile gives the v coordinate, so texels stay square
    let mut acc = 0.0f32;
    for sg in segs {
        let m = sg.pts.len();
        let first = r.pos.len() as u32;
        let lens: Vec<f32> =
            (0..m).map(|k| if k == 0 { 0.0 } else { ((sg.pts[k][0] - sg.pts[k - 1][0]).powi(2) + (sg.pts[k][1] - sg.pts[k - 1][1]).powi(2)).sqrt() }).collect();
        let total: f32 = lens.iter().sum();
        let mut run = 0.0f32;
        for k in 0..m {
            run += lens[k];
            let (a, b) = (sg.pts[k.saturating_sub(1)], sg.pts[(k + 1).min(m - 1)]);
            let d = [b[0] - a[0], b[1] - a[1]];
            let dl = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-9);
            let (nr, ny) = (-d[1] / dl, d[0] / dl);
            let rad = sg.pts[k][0];
            let y = sg.pts[k][1];
            let edge = run.min(total - run);
            let mut fade: f32 = 1.0;
            if sg.fade0 && run < edge + 1e-6 {
                fade = fade.min(smoothstep01(run / 0.03));
            }
            if sg.fade1 && (total - run) <= run + 1e-6 {
                fade = fade.min(smoothstep01((total - run) / 0.03));
            }
            for i in 0..=nu {
                let lon = i as f32 / nu as f32 * TAU - PI;
                let (sl, cl) = lon.sin_cos();
                let pos = [rad * sl, y, rad * cl];
                let nrm = norm([nr * sl, ny, nr * cl]);
                r.push(
                    pos,
                    nrm,
                    [cl, 0.0, -sl],
                    [i as f32 / nu as f32 * tiling * ku, (acc + run) * tiling / 3.14],
                    (TAU * rad.max(0.05) / (ku * tiling)).max(0.05),
                    3.14 / tiling,
                    0,
                    fade,
                );
            }
        }
        r.grid_tris(first, nu, (m - 1) as u32);
        acc += total;
    }
}

fn raw_plane_hires(n: u32, tiling: f32) -> Raw {
    raw_plane((n * 2).min(1024), tiling)
}

fn raw_cylinder(n: u32, tiling: f32) -> Raw {
    let (rad, h) = (0.8, 0.8);
    let st = (n / 2).max(4) as usize;
    let mut r = Raw::new();
    let segs = [
        seg_line([0.0, h], [rad, h], st, false, true),
        seg_line([rad, h], [rad, -h], (n as usize).max(8), true, true),
        seg_line([rad, -h], [0.0, -h], st, true, false),
    ];
    revolve(&mut r, n, tiling, 2.0, &segs);
    weld_by_position(&mut r, false);
    r
}

fn raw_rounded_cylinder(n: u32, tiling: f32) -> Raw {
    let (rad, h, rr) = (0.8, 0.8, 0.2);
    let st = (n / 2).max(4) as usize;
    let arc = (n / 4).max(6) as usize;
    let mut r = Raw::new();
    let segs = [
        seg_line([0.0, h], [rad - rr, h], st, false, false),
        seg_arc([rad - rr, h - rr], rr, PI / 2.0, 0.0, arc, false, false),
        seg_line([rad, h - rr], [rad, -h + rr], (n as usize).max(8), false, false),
        seg_arc([rad - rr, -h + rr], rr, 0.0, -PI / 2.0, arc, false, false),
        seg_line([rad - rr, -h], [0.0, -h], st, false, false),
    ];
    revolve(&mut r, n, tiling, 2.0, &segs);
    weld_by_position(&mut r, true);
    r
}

fn raw_torus(n: u32, tiling: f32) -> Raw {
    let (big, small) = (0.68, 0.34);
    let steps = (n as usize).max(12);
    let seg = Seg {
        pts: (0..=steps)
            .map(|i| {
                let t = i as f32 / steps as f32 * TAU;
                [big + small * t.cos(), -small * t.sin()]
            })
            .collect(),
        fade0: false,
        fade1: false,
    };
    let mut r = Raw::new();
    revolve(&mut r, n, tiling, 4.0, &[seg]);
    weld_by_position(&mut r, true);
    r
}

fn raw_mat_ball(n: u32, tiling: f32) -> Raw {
    let (rad, cy) = (0.8, 0.2);
    let steps = (n as usize).max(12);
    let ball = Seg {
        pts: (0..=steps)
            .map(|i| {
                let t = i as f32 / steps as f32 * PI;
                [rad * t.sin(), cy + rad * t.cos()]
            })
            .collect(),
        fade0: false,
        fade1: false,
    };
    let (top, bot, pr) = (cy - rad, cy - rad - 0.18, 1.2);
    let st = (n / 2).max(4) as usize;
    let segs = [
        ball,
        seg_line([0.0, top], [pr, top], st, true, true),
        seg_line([pr, top], [pr, bot], 8, true, true),
        seg_line([pr, bot], [0.0, bot], st, true, false),
    ];
    let mut r = Raw::new();
    revolve(&mut r, n, tiling, 2.0, &segs);
    weld_by_position(&mut r, false);
    r
}

/// Cube with rounded edges: every cube-face point is pulled onto the surface of a box with rounded corners.
fn raw_rounded_cube(n: u32, tiling: f32) -> Raw {
    let (h, rr) = (0.8f32, 0.22f32);
    let inner = h - rr;
    let faces: [(V, V, V); 6] = [
        ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
    ];
    let mut r = Raw::new();
    let nn = (n * 5 / 8).max(12);
    for (nrm, tg, bt) in faces {
        patch(&mut r, nn, nn, tiling, (1.0, 1.0), 0.03, |u, v| {
            let q = add(add(mul(nrm, h), mul(tg, (u - 0.5) * 2.0 * h)), mul(bt, (0.5 - v) * 2.0 * h));
            let c = [q[0].clamp(-inner, inner), q[1].clamp(-inner, inner), q[2].clamp(-inner, inner)];
            add(c, mul(norm(sub(q, c)), rr))
        });
    }
    weld_by_position(&mut r, true);
    r
}

/// Box open at the top with a thick wall: outer faces, inner faces (normals point into the cavity) and the rim.
fn raw_hollow_box(n: u32, tiling: f32) -> Raw {
    let (h, hi) = (0.8f32, 0.62f32);
    let faces: [(V, V, V); 5] = [
        ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
    ];
    let nn = (n * 5 / 8).max(8);
    let mut r = Raw::new();
    for (nrm, tg, bt) in faces {
        quad(&mut r, nn, tiling, mul(nrm, h), tg, bt, h, h, 0.02);
        // the cavity wall faces the other way: mirror one axis so that tg x bt = -nrm
        quad(&mut r, nn, tiling, mul(nrm, hi), mul(tg, -1.0), bt, hi, hi, 0.02);
    }
    let mid = (h + hi) * 0.5;
    let half = (h - hi) * 0.5;
    let (tg, bt) = ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0]);
    quad(&mut r, nn, tiling, [0.0, h, mid], tg, bt, h, half, 0.02);
    quad(&mut r, nn, tiling, [0.0, h, -mid], tg, bt, h, half, 0.02);
    quad(&mut r, nn, tiling, [mid, h, 0.0], tg, bt, half, hi, 0.02);
    quad(&mut r, nn, tiling, [-mid, h, 0.0], tg, bt, half, hi, 0.02);
    weld_by_position(&mut r, false);
    r
}

/// The six walls of a box seen from the inside (outer faces are culled, so the far walls show).
fn raw_inner_box(n: u32, tiling: f32) -> Raw {
    let h = 0.8f32;
    let faces: [(V, V, V); 6] = [
        ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
    ];
    let mut r = Raw::new();
    let nn = (n * 5 / 8).max(8);
    for (nrm, tg, bt) in faces {
        quad(&mut r, nn, tiling, mul(nrm, h), mul(tg, -1.0), bt, h, h, 0.02);
    }
    weld_by_position(&mut r, false);
    r
}

/// A draped sheet: a plane with soft folds.
fn raw_cloth(n: u32, tiling: f32) -> Raw {
    let size = PLANE_HALF * 2.0;
    let mut r = Raw::new();
    patch(&mut r, n, n, tiling, (size / 1.6, size / 1.6), 0.0, |u, v| {
        let (x, y) = ((u - 0.5) * size, (0.5 - v) * size);
        let z = 0.17 * (2.3 * x + 0.9 * (1.4 * y).sin()).sin() + 0.09 * (1.9 * y - 1.2 * x + 0.6).sin() + 0.04 * (6.0 * x + 3.0 * y).sin();
        [x, y, z]
    });
    weld_by_position(&mut r, true);
    r
}

/// Build the shape mesh, displacing it along the surface normal when a height map is given.
pub fn build_mesh(shape: u32, tess: u32, tiling: f32, height: Option<&Tex>, amount: f32, level: f32) -> Mesh {
    let n = tess.clamp(8, 768);
    let tiling = tiling.max(0.1);
    let r = match shape {
        0 => raw_sphere(n, tiling),
        1 => raw_cube((n * 5 / 8).max(8), tiling),
        3 => raw_cylinder(n, tiling),
        4 => raw_hollow_box(n, tiling),
        5 => raw_inner_box(n, tiling),
        6 => raw_plane_hires(n, tiling),
        7 => raw_cloth(n, tiling),
        8 => raw_mat_ball(n, tiling),
        9 => raw_rounded_cube(n, tiling),
        10 => raw_rounded_cylinder(n, tiling),
        11 => raw_sphere(n, tiling * 2.0),
        12 => raw_torus(n, tiling),
        _ => raw_plane(n, tiling),
    };
    let nv = r.pos.len();
    let displace = amount.abs() > 1e-6 && height.is_some();
    let mut pos = r.pos.clone();
    let mut nrm = r.nrm.clone();
    if let (true, Some(hm)) = (displace, height) {
        // one height per weld group so seams and poles stay closed
        let groups = r.weld.iter().copied().max().unwrap_or(0) as usize + 1;
        let mut sum = vec![0.0f32; groups];
        let mut cnt = vec![0u32; groups];
        let hv: Vec<f32> = (0..nv).map(|i| lum(hm.sample(r.uv[i][0], r.uv[i][1]))).collect();
        for i in 0..nv {
            sum[r.weld[i] as usize] += hv[i];
            cnt[r.weld[i] as usize] += 1;
        }
        for i in 0..nv {
            let hh = sum[r.weld[i] as usize] / cnt[r.weld[i] as usize].max(1) as f32;
            pos[i] = add(r.pos[i], mul(r.nrm[i], (hh - level) * amount * r.fade[i]));
        }
        // smooth normals of the displaced surface (area weighted, accumulated per weld group)
        let mut acc = vec![[0.0f32; 3]; groups];
        for t in &r.tris {
            let (a, b, c) = (pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]);
            let fnrm = cross(sub(b, a), sub(c, a));
            for k in 0..3 {
                let g = r.weld[t[k] as usize] as usize;
                acc[g] = add(acc[g], fnrm);
            }
        }
        for i in 0..nv {
            let a = acc[r.weld[i] as usize];
            nrm[i] = if dot(a, a) > 1e-20 { norm(a) } else { r.nrm[i] };
        }
    }
    // re-orthogonalise the analytic tangents against the (possibly displaced) normals
    let mut tan = r.tan.clone();
    for i in 0..nv {
        let t = sub(r.tan[i], mul(nrm[i], dot(r.tan[i], nrm[i])));
        tan[i] = if dot(t, t) > 1e-12 { norm(t) } else { r.tan[i] };
    }
    Mesh { pos, nrm, tan, uv: r.uv, su: r.su, sv: r.sv, fade: r.fade, base_nrm: r.nrm, tris: r.tris }
}

// ------------------------------------------------------------------------------------------------ rasteriser
#[derive(Clone, Copy)]
struct Frag {
    z: f32,
    tri: u32,
    b1: f32,
    b2: f32,
}
const NO_TRI: u32 = u32::MAX;
const NEAR: f32 = 0.05;

struct Cam {
    eye: V,
    fwd: V,
    right: V,
    up: V,
    tf: f32,
    aspect: f32,
    w: usize,
    h: usize,
}

impl Cam {
    fn view(&self, p: V) -> V {
        let d = sub(p, self.eye);
        [dot(d, self.right), dot(d, self.up), dot(d, self.fwd)]
    }
    /// view-space point with z > 0 -> (screen x, screen y, 1/z)
    fn project(&self, v: V) -> [f32; 3] {
        let iz = 1.0 / v[2];
        [(v[0] * iz / (self.aspect * self.tf) * 0.5 + 0.5) * self.w as f32, (0.5 - v[1] * iz / self.tf * 0.5) * self.h as f32, iz]
    }
}

#[derive(Clone, Copy)]
struct Pv {
    s: [f32; 3],
    a: [f32; 3], // barycentric weights of the ORIGINAL triangle at this (possibly clipped) vertex
}

fn raster(band: &mut [Frag], y_start: usize, w: usize, tri: u32, v: [Pv; 3]) {
    let rows = band.len() / w;
    let y_end = y_start + rows;
    let (x0, x1, x2) = (v[0].s[0], v[1].s[0], v[2].s[0]);
    let (y0, y1, y2) = (v[0].s[1], v[1].s[1], v[2].s[1]);
    if !(x0.is_finite() && x1.is_finite() && x2.is_finite() && y0.is_finite() && y1.is_finite() && y2.is_finite()) {
        return;
    }
    let area = (x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0);
    if area.abs() < 1e-9 {
        return;
    }
    let (minx, maxx) = (x0.min(x1).min(x2), x0.max(x1).max(x2));
    let (miny, maxy) = (y0.min(y1).min(y2), y0.max(y1).max(y2));
    if maxx < 0.0 || minx > w as f32 || maxy < y_start as f32 || miny > y_end as f32 {
        return;
    }
    let xa = (minx.max(0.0).floor() as usize).min(w - 1);
    let xb = (maxx.min(w as f32 - 1.0).ceil().max(0.0) as usize).min(w - 1);
    let ya = (miny.max(y_start as f32).floor() as usize).max(y_start);
    let yb = (maxy.min(y_end as f32 - 1.0).ceil().max(0.0) as usize).min(y_end - 1);
    let inv = 1.0 / area;
    let (iz0, iz1, iz2) = (v[0].s[2], v[1].s[2], v[2].s[2]);
    for y in ya..=yb {
        let py = y as f32 + 0.5;
        for x in xa..=xb {
            let px = x as f32 + 0.5;
            let l0 = ((x1 - px) * (y2 - py) - (x2 - px) * (y1 - py)) * inv;
            let l1 = ((x2 - px) * (y0 - py) - (x0 - px) * (y2 - py)) * inv;
            let l2 = 1.0 - l0 - l1;
            if l0 < -1e-5 || l1 < -1e-5 || l2 < -1e-5 {
                continue;
            }
            let (k0, k1, k2) = (l0 * iz0, l1 * iz1, l2 * iz2);
            let iz = k0 + k1 + k2;
            if iz <= 0.0 {
                continue;
            }
            let z = 1.0 / iz;
            let f = &mut band[(y - y_start) * w + x];
            if z < f.z {
                let s = 1.0 / iz;
                let b1 = (k0 * v[0].a[1] + k1 * v[1].a[1] + k2 * v[2].a[1]) * s;
                let b2 = (k0 * v[0].a[2] + k1 * v[1].a[2] + k2 * v[2].a[2]) * s;
                *f = Frag { z, tri, b1, b2 };
            }
        }
    }
}

/// Clip a triangle that crosses the near plane and rasterise the (1 or 2) resulting triangles.
fn raster_clipped(band: &mut [Frag], y_start: usize, w: usize, cam: &Cam, tri: u32, vv: [V; 3]) {
    let attrs = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let mut poly: Vec<(V, [f32; 3])> = Vec::with_capacity(4);
    for k in 0..3 {
        let (a, b) = ((vv[k], attrs[k]), (vv[(k + 1) % 3], attrs[(k + 1) % 3]));
        let (ina, inb) = (a.0[2] >= NEAR, b.0[2] >= NEAR);
        if ina {
            poly.push(a);
        }
        if ina != inb {
            let t = (NEAR - a.0[2]) / (b.0[2] - a.0[2]);
            let p = [a.0[0] + (b.0[0] - a.0[0]) * t, a.0[1] + (b.0[1] - a.0[1]) * t, NEAR];
            let at = [a.1[0] + (b.1[0] - a.1[0]) * t, a.1[1] + (b.1[1] - a.1[1]) * t, a.1[2] + (b.1[2] - a.1[2]) * t];
            poly.push((p, at));
        }
    }
    if poly.len() < 3 {
        return;
    }
    let pv: Vec<Pv> = poly.iter().map(|(p, a)| Pv { s: cam.project(*p), a: *a }).collect();
    for k in 1..pv.len() - 1 {
        raster(band, y_start, w, tri, [pv[0], pv[k], pv[k + 1]]);
    }
}

fn rasterize(mesh: &Mesh, cam: &Cam, cull: bool) -> Vec<Frag> {
    let (w, h) = (cam.w, cam.h);
    let none = Frag { z: f32::INFINITY, tri: NO_TRI, b1: 0.0, b2: 0.0 };
    let mut frags = vec![none; w * h];
    let view: Vec<V> = mesh.pos.iter().map(|p| cam.view(*p)).collect();
    let proj: Vec<[f32; 3]> = view.iter().map(|v| if v[2] >= NEAR { cam.project(*v) } else { [0.0, 0.0, 0.0] }).collect();
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).max(1);
    let rows_per = (h + threads - 1) / threads;
    let (view, proj) = (&view, &proj);
    std::thread::scope(|s| {
        for (ci, band) in frags.chunks_mut(rows_per * w).enumerate() {
            s.spawn(move || {
                let y_start = ci * rows_per;
                let y_end = y_start + band.len() / w;
                for (ti, t) in mesh.tris.iter().enumerate() {
                    let (i0, i1, i2) = (t[0] as usize, t[1] as usize, t[2] as usize);
                    if cull {
                        // walls whose (smoothed) normal faces away from the camera are not drawn
                        let nn = add(add(mesh.base_nrm[i0], mesh.base_nrm[i1]), mesh.base_nrm[i2]);
                        if dot(nn, sub(cam.eye, mesh.pos[i0])) <= 0.0 {
                            continue;
                        }
                    }
                    if view[i0][2] >= NEAR && view[i1][2] >= NEAR && view[i2][2] >= NEAR {
                        let (p0, p1, p2) = (proj[i0], proj[i1], proj[i2]);
                        if p0[1].max(p1[1]).max(p2[1]) < y_start as f32 || p0[1].min(p1[1]).min(p2[1]) > y_end as f32 {
                            continue;
                        }
                        raster(
                            band,
                            y_start,
                            w,
                            ti as u32,
                            [Pv { s: p0, a: [1.0, 0.0, 0.0] }, Pv { s: p1, a: [0.0, 1.0, 0.0] }, Pv { s: p2, a: [0.0, 0.0, 1.0] }],
                        );
                    } else if view[i0][2] >= NEAR || view[i1][2] >= NEAR || view[i2][2] >= NEAR {
                        raster_clipped(band, y_start, w, cam, ti as u32, [view[i0], view[i1], view[i2]]);
                    }
                }
            });
        }
    });
    frags
}

// ------------------------------------------------------------------------------------------------ shading
struct Surf {
    n: V,
    t: V,
    b: V,
    uv: [f32; 2],
    su: f32,
    sv: f32,
    back: bool,
}

fn env(d: V) -> V {
    let sky = [0.42, 0.52, 0.68];
    let ground = [0.30, 0.28, 0.26];
    let hor = [0.58, 0.58, 0.58];
    let base = if d[1] > 0.0 { add(mul(hor, 1.0 - d[1]), mul(sky, d[1])) } else { add(mul(hor, 1.0 + d[1] * 0.8), mul(ground, -d[1] * 0.8)) };
    [base[0].max(0.0), base[1].max(0.0), base[2].max(0.0)]
}

fn lin(c: f32) -> f32 {
    c.max(0.0).powf(2.2)
}

/// Outward relief mapping. The height field is a slab of thickness `depth` that sits on the surface: height `level`
/// is flush with the polygon, brighter pixels rise out of it. The view ray enters the slab at its top and is marched
/// down until it meets the height field. Returns (uv, fraction 0..1 of how deep below the top the hit is).
fn parallax(hm: &Tex, s: &Surf, vt: V, depth: f32, level: f32) -> ([f32; 2], f32) {
    let vz = vt[2].max(0.2);
    let du = -vt[0] / vz / s.su;
    let dv = vt[1] / vz / s.sv; // uv v runs opposite to the bitangent
    let top = (1.0 - level.clamp(0.0, 1.0)) * depth; // distance (along the ray) from slab top to the polygon plane
    let at = |d: f32| -> (f32, [f32; 2]) {
        let uv = [s.uv[0] + du * (d - top), s.uv[1] + dv * (d - top)];
        ((1.0 - lum(hm.sample(uv[0], uv[1]))) * depth - d, uv)
    };
    let steps = 48;
    let (mut prev_d, (mut prev_f, _)) = (0.0f32, at(0.0));
    if prev_f <= 0.0 {
        return (at(0.0).1, 0.0);
    }
    for i in 1..=steps {
        let d = depth * i as f32 / steps as f32;
        let (f, _) = at(d);
        if f <= 0.0 {
            let (mut lo, mut hi) = (prev_d, d);
            for _ in 0..5 {
                let mid = 0.5 * (lo + hi);
                if at(mid).0 > 0.0 {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            let dh = 0.5 * (lo + hi);
            return (at(dh).1, (dh / depth).clamp(0.0, 1.0));
        }
        prev_d = d;
        prev_f = f;
    }
    let _ = prev_f;
    (at(depth).1, 1.0)
}

pub fn render(m: &Maps, v: &View, w: usize, h: usize) -> Tex {
    let (w, h) = (w.max(1), h.max(1));
    let Basis { eye, right, up, forward: fwd } = v.orbit().basis();
    let tf = (20.0f32).to_radians().tan();
    let aspect = w as f32 / h as f32;
    let cam = Cam { eye, fwd, right, up, tf, aspect, w, h };
    let (ldir, fill) = light_dirs(v);

    let tiling = v.tiling.max(0.1);
    let displace = v.relief == RELIEF_DISPLACE && v.height_scale > 0.0 && m.height.is_some();
    let mesh = build_mesh(v.shape, v.tess, tiling, m.height, if displace { v.height_scale } else { 0.0 }, v.height_level);
    let frags = rasterize(&mesh, &cam, shape_culls(v.shape));
    let wire_on = v.wireframe;

    Tex::from_fn(w, h, |px, py| {
        let x = (px * 2.0 - 1.0) * aspect * tf;
        let y = (1.0 - py * 2.0) * tf;
        let dir = norm(add(add(fwd, mul(right, x)), mul(up, y)));
        let (ix, iy) = (((px * w as f32) as usize).min(w - 1), ((py * h as f32) as usize).min(h - 1));
        let fr = frags[iy * w + ix];
        if fr.tri == NO_TRI {
            let g = 0.12 + 0.08 * (1.0 - py);
            return [g, g, g * 1.05, 1.0];
        }
        let tri = mesh.tris[fr.tri as usize];
        let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
        let (b1, b2) = (fr.b1, fr.b2);
        let b0 = 1.0 - b1 - b2;
        let mix3 = |a: V, b: V, c: V| add(add(mul(a, b0), mul(b, b1)), mul(c, b2));
        let n = norm(mix3(mesh.nrm[i0], mesh.nrm[i1], mesh.nrm[i2]));
        let t0 = mix3(mesh.tan[i0], mesh.tan[i1], mesh.tan[i2]);
        let t = norm(sub(t0, mul(n, dot(t0, n))));
        let bt = cross(n, t);
        let uv = [b0 * mesh.uv[i0][0] + b1 * mesh.uv[i1][0] + b2 * mesh.uv[i2][0], b0 * mesh.uv[i0][1] + b1 * mesh.uv[i1][1] + b2 * mesh.uv[i2][1]];
        let su = (b0 * mesh.su[i0] + b1 * mesh.su[i1] + b2 * mesh.su[i2]).max(1e-4);
        let sv = (b0 * mesh.sv[i0] + b1 * mesh.sv[i1] + b2 * mesh.sv[i2]).max(1e-4);
        let vv = norm(mul(dir, -1.0));
        // triangle winding is outward, so the true face normal tells us whether we look at the back of the surface
        let (pa, pb, pc) = (mesh.pos[i0], mesh.pos[i1], mesh.pos[i2]);
        let back = dot(cross(sub(pb, pa), sub(pc, pa)), vv) < 0.0;
        let s = Surf { n, t, b: bt, uv, su, sv, back };

        let mut uv = s.uv;
        let mut cavity = 1.0;
        if let (Some(hm), true, true) = (m.height, v.relief == RELIEF_PARALLAX, v.height_scale > 0.0) {
            let vt = [dot(vv, s.t), dot(vv, s.b), dot(vv, s.n) * if s.back { -1.0 } else { 1.0 }];
            let (nuv, d) = parallax(hm, &s, vt, v.height_scale, v.height_level);
            uv = nuv;
            cavity = 1.0 - 0.45 * d;
        }
        let base = m.base.sample(uv[0], uv[1]);
        let albedo = [lin(base[0]), lin(base[1]), lin(base[2])];
        let rough = lum(m.rough.sample(uv[0], uv[1])).clamp(0.04, 1.0);
        let metal = lum(m.metal.sample(uv[0], uv[1])).clamp(0.0, 1.0);
        let ao = lum(m.ao.sample(uv[0], uv[1])).clamp(0.0, 1.0) * cavity;
        let nm = m.normal.sample(uv[0], uv[1]);
        let amt = v.normal_amt.clamp(0.0, 1.0);
        let mut tn = [(nm[0] * 2.0 - 1.0) * amt, (nm[1] * 2.0 - 1.0) * amt, (nm[2] * 2.0 - 1.0).max(0.05)];
        if let (Some(hm), true) = (m.height, v.relief == RELIEF_BUMP && v.bump > 0.0) {
            let e = 1.0 / hm.w.max(1) as f32;
            let hs = |du: f32, dv: f32| lum(hm.sample(uv[0] + du, uv[1] + dv));
            let (dhu, dhv) = ((hs(e, 0.0) - hs(-e, 0.0)) / (2.0 * e), (hs(0.0, e) - hs(0.0, -e)) / (2.0 * e));
            let k = v.bump * 0.12;
            tn[0] -= k * dhu / s.su;
            tn[1] += k * dhv / s.sv;
        }
        let tn = norm(tn);
        let mut n = norm(add(add(mul(s.t, tn[0]), mul(s.b, tn[1])), mul(s.n, tn[2])));
        if s.back {
            n = mul(n, -1.0);
        }
        let ndv = dot(n, vv).max(1e-3);
        let f0 = [0.04 * (1.0 - metal) + albedo[0] * metal, 0.04 * (1.0 - metal) + albedo[1] * metal, 0.04 * (1.0 - metal) + albedo[2] * metal];
        let a = rough * rough;
        let hv = norm(add(ldir, vv));
        let (ndl, ndh, vdh) = (dot(n, ldir).max(0.0), dot(n, hv).max(0.0), dot(vv, hv).max(0.0));
        let d = a * a / (PI * (ndh * ndh * (a * a - 1.0) + 1.0).powi(2)).max(1e-6);
        let k = (rough + 1.0).powi(2) / 8.0;
        let g = (ndl / (ndl * (1.0 - k) + k)) * (ndv / (ndv * (1.0 - k) + k));
        let fr_ = (1.0 - vdh).powi(5);
        let refl = norm(sub(mul(n, 2.0 * dot(n, vv)), vv));
        let envr = env(refl);
        let envd = env(n);
        let ndl2 = dot(n, fill).max(0.0);
        let mut out = [0.0; 3];
        for c in 0..3 {
            let f = f0[c] + (1.0 - f0[c]) * fr_;
            let spec = d * g * f / (4.0 * ndl.max(1e-3) * ndv) * ndl;
            let diff = albedo[c] * (1.0 - metal) * (1.0 - f) / PI * ndl * v.light_intensity * 3.0;
            let diff_fill = albedo[c] * (1.0 - metal) / PI * ndl2 * v.light_intensity * 0.9;
            let amb_d = albedo[c] * (1.0 - metal) * envd[c] * v.ambient;
            let rf = f0[c] + (1.0 - f0[c]) * fr_ * (1.0 - rough);
            let amb_s = envr[c] * rf * (1.0 - rough * 0.65) * v.ambient;
            out[c] = (diff + diff_fill + spec * v.light_intensity * 3.0 + (amb_d + amb_s) * ao) * v.exposure;
        }
        if let Some(e) = m.emissive {
            let ec = e.sample(uv[0], uv[1]);
            for (c, o) in out.iter_mut().enumerate() {
                *o += lin(ec[c]);
            }
        }
        let tm = |c: f32| (c / (1.0 + c)).max(0.0).powf(1.0 / 2.2) * 1.15;
        let mut col = [tm(out[0]).min(1.0), tm(out[1]).min(1.0), tm(out[2]).min(1.0)];
        if wire_on {
            let cov = wire_coverage(&frags, w, h, ix, iy, fr);
            if cov > 0.0 {
                let line = [1.0, 0.62, 0.12];
                for c in 0..3 {
                    col[c] = col[c] * (1.0 - 0.85 * cov) + line[c] * 0.85 * cov;
                }
            }
        }
        [col[0], col[1], col[2], 1.0]
    })
}

/// Anti-aliased triangle-edge coverage for the wireframe overlay (0..1), from the screen-space gradient of the
/// barycentric coordinates measured against a neighbouring pixel of the same triangle.
fn wire_coverage(frags: &[Frag], w: usize, h: usize, x: usize, y: usize, f: Frag) -> f32 {
    let b = [1.0 - f.b1 - f.b2, f.b1, f.b2];
    let nb = |dx: i64, dy: i64| -> Option<[f32; 3]> {
        let (nx, ny) = (x as i64 + dx, y as i64 + dy);
        if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
            return None;
        }
        let g = frags[ny as usize * w + nx as usize];
        if g.tri == f.tri { Some([1.0 - g.b1 - g.b2, g.b1, g.b2]) } else { None }
    };
    let gx = nb(1, 0)
        .map(|o| (0..3).map(|i| (o[i] - b[i]).abs()).collect::<Vec<_>>())
        .or_else(|| nb(-1, 0).map(|o| (0..3).map(|i| (o[i] - b[i]).abs()).collect::<Vec<_>>()));
    let gy = nb(0, 1)
        .map(|o| (0..3).map(|i| (o[i] - b[i]).abs()).collect::<Vec<_>>())
        .or_else(|| nb(0, -1).map(|o| (0..3).map(|i| (o[i] - b[i]).abs()).collect::<Vec<_>>()));
    let (Some(gx), Some(gy)) = (gx, gy) else { return 0.0 };
    let mut dmin = f32::MAX;
    for i in 0..3 {
        let g = (gx[i] * gx[i] + gy[i] * gy[i]).sqrt();
        if g > 1e-9 {
            dmin = dmin.min(b[i].max(0.0) / g);
        }
    }
    1.0 - smoothstep01((dmin - 0.4) / 0.9)
}

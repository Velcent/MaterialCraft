//! Mesh map baking: high-poly -> low-poly textures (normal, world normal, position, AO,
//! curvature, thickness, ID). OBJ input, BVH ray tracing, multi-threaded.
use crate::noise::rnd;
use crate::tex::{Tex, par_map};
use crate::v3::*;
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct Tri {
    pub p: [V; 3],
    pub n: [V; 3],
    pub uv: [[f32; 2]; 3],
    pub group: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub tris: Vec<Tri>,
    pub groups: Vec<String>,
}

fn idx(tok: &str, len: usize) -> Result<Option<usize>, String> {
    if tok.is_empty() {
        return Ok(None);
    }
    let i: i64 = tok.parse().map_err(|_| format!("bad index '{tok}'"))?;
    let r = if i > 0 { i - 1 } else { len as i64 + i };
    if r < 0 || r as usize >= len {
        return Err(format!("index {i} out of range"));
    }
    Ok(Some(r as usize))
}

pub fn parse_obj(text: &str) -> Result<Mesh, String> {
    let (mut vs, mut vts, mut vns): (Vec<V>, Vec<[f32; 2]>, Vec<V>) = (vec![], vec![], vec![]);
    let mut mesh = Mesh::default();
    mesh.groups.push("default".into());
    let mut group = 0u32;
    for (ln, line) in text.lines().enumerate() {
        let t: Vec<&str> = line.split_whitespace().collect();
        if t.is_empty() {
            continue;
        }
        let num = |s: &str| s.parse::<f32>().map_err(|_| format!("line {}: bad number '{s}'", ln + 1));
        match t[0] {
            "v" if t.len() >= 4 => vs.push([num(t[1])?, num(t[2])?, num(t[3])?]),
            "vt" if t.len() >= 3 => vts.push([num(t[1])?, num(t[2])?]),
            "vn" if t.len() >= 4 => vns.push([num(t[1])?, num(t[2])?, num(t[3])?]),
            "o" | "g" if t.len() >= 2 => {
                let name = t[1..].join(" ");
                group = match mesh.groups.iter().position(|g| *g == name) {
                    Some(i) => i as u32,
                    None => {
                        mesh.groups.push(name);
                        (mesh.groups.len() - 1) as u32
                    }
                };
            }
            "f" if t.len() >= 4 => {
                let mut corners = Vec::new();
                for tok in &t[1..] {
                    let parts: Vec<&str> = tok.split('/').collect();
                    let p = idx(parts[0], vs.len()).map_err(|e| format!("line {}: {e}", ln + 1))?.ok_or_else(|| format!("line {}: missing vertex", ln + 1))?;
                    let uv = match parts.get(1) {
                        Some(s) => idx(s, vts.len()).map_err(|e| format!("line {}: {e}", ln + 1))?,
                        None => None,
                    };
                    let n = match parts.get(2) {
                        Some(s) => idx(s, vns.len()).map_err(|e| format!("line {}: {e}", ln + 1))?,
                        None => None,
                    };
                    corners.push((p, uv, n));
                }
                for k in 1..corners.len() - 1 {
                    let c = [corners[0], corners[k], corners[k + 1]];
                    let p = [vs[c[0].0], vs[c[1].0], vs[c[2].0]];
                    let fnrm = norm(cross(sub(p[1], p[0]), sub(p[2], p[0])));
                    let mut tri = Tri { p, n: [fnrm; 3], uv: [[0.0; 2]; 3], group };
                    for j in 0..3 {
                        if let Some(u) = c[j].1 {
                            tri.uv[j] = vts[u];
                        }
                        if let Some(n) = c[j].2 {
                            tri.n[j] = norm(vns[n]);
                        }
                    }
                    mesh.tris.push(tri);
                }
            }
            _ => {}
        }
    }
    if mesh.tris.is_empty() {
        return Err("mesh has no faces".into());
    }
    Ok(mesh)
}

// ---------------- BVH ----------------
#[derive(Clone, Copy)]
struct BNode {
    min: V,
    max: V,
    a: u32, // leaf: start, inner: left
    b: u32, // leaf: count (>0), inner: right (count==0 flagged by leaf=false)
    leaf: bool,
}

pub struct Bvh {
    nodes: Vec<BNode>,
    idx: Vec<u32>,
}

#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub t: f32,
    pub tri: u32,
    pub u: f32,
    pub v: f32,
}

impl Bvh {
    pub fn build(tris: &[Tri]) -> Bvh {
        let cent: Vec<V> = tris.iter().map(|t| mul(add(add(t.p[0], t.p[1]), t.p[2]), 1.0 / 3.0)).collect();
        let mut b = Bvh { nodes: Vec::new(), idx: (0..tris.len() as u32).collect() };
        b.rec(tris, &cent, 0, tris.len());
        b
    }

    fn rec(&mut self, tris: &[Tri], cent: &[V], lo: usize, hi: usize) -> u32 {
        let (mut mn, mut mx) = ([f32::MAX; 3], [f32::MIN; 3]);
        let (mut cmn, mut cmx) = ([f32::MAX; 3], [f32::MIN; 3]);
        for &i in &self.idx[lo..hi] {
            for p in &tris[i as usize].p {
                for k in 0..3 {
                    mn[k] = mn[k].min(p[k]);
                    mx[k] = mx[k].max(p[k]);
                }
            }
            for k in 0..3 {
                cmn[k] = cmn[k].min(cent[i as usize][k]);
                cmx[k] = cmx[k].max(cent[i as usize][k]);
            }
        }
        let me = self.nodes.len() as u32;
        self.nodes.push(BNode { min: mn, max: mx, a: lo as u32, b: (hi - lo) as u32, leaf: true });
        if hi - lo <= 4 {
            return me;
        }
        let ext = sub(cmx, cmn);
        let axis = if ext[0] >= ext[1] && ext[0] >= ext[2] {
            0
        } else if ext[1] >= ext[2] {
            1
        } else {
            2
        };
        self.idx[lo..hi].sort_by(|&a, &b| cent[a as usize][axis].partial_cmp(&cent[b as usize][axis]).unwrap_or(std::cmp::Ordering::Equal));
        let mid = (lo + hi) / 2;
        let l = self.rec(tris, cent, lo, mid);
        let r = self.rec(tris, cent, mid, hi);
        self.nodes[me as usize].leaf = false;
        self.nodes[me as usize].a = l;
        self.nodes[me as usize].b = r;
        me
    }

    pub fn intersect(&self, tris: &[Tri], o: V, d: V, tmax: f32) -> Option<Hit> {
        let inv = [
            1.0 / if d[0].abs() < 1e-12 { 1e-12 } else { d[0] },
            1.0 / if d[1].abs() < 1e-12 { 1e-12 } else { d[1] },
            1.0 / if d[2].abs() < 1e-12 { 1e-12 } else { d[2] },
        ];
        let mut best: Option<Hit> = None;
        let mut tm = tmax;
        let mut stack = [0u32; 64];
        let mut sp = 1;
        while sp > 0 {
            sp -= 1;
            let n = self.nodes[stack[sp] as usize];
            let (mut t0, mut t1) = (0.0f32, tm);
            for k in 0..3 {
                let a = (n.min[k] - o[k]) * inv[k];
                let b = (n.max[k] - o[k]) * inv[k];
                t0 = t0.max(a.min(b));
                t1 = t1.min(a.max(b));
            }
            if t0 > t1 {
                continue;
            }
            if n.leaf {
                for &ti in &self.idx[n.a as usize..(n.a + n.b) as usize] {
                    let t = &tris[ti as usize];
                    let e1 = sub(t.p[1], t.p[0]);
                    let e2 = sub(t.p[2], t.p[0]);
                    let pv = cross(d, e2);
                    let det = dot(e1, pv);
                    if det.abs() < 1e-14 {
                        continue;
                    }
                    let id = 1.0 / det;
                    let tv = sub(o, t.p[0]);
                    let u = dot(tv, pv) * id;
                    if !(0.0..=1.0).contains(&u) {
                        continue;
                    }
                    let q = cross(tv, e1);
                    let v = dot(d, q) * id;
                    if v < 0.0 || u + v > 1.0 {
                        continue;
                    }
                    let tt = dot(e2, q) * id;
                    if tt > 1e-6 && tt < tm {
                        tm = tt;
                        best = Some(Hit { t: tt, tri: ti, u, v });
                    }
                }
            } else if sp + 2 <= 64 {
                stack[sp] = n.a;
                stack[sp + 1] = n.b;
                sp += 2;
            }
        }
        best
    }
}

fn interp_n(t: &Tri, h: &Hit) -> V {
    norm(add(add(mul(t.n[0], 1.0 - h.u - h.v), mul(t.n[1], h.u)), mul(t.n[2], h.v)))
}

#[derive(Clone, Debug)]
pub struct BakeSettings {
    pub size: usize,
    pub max_dist: f32,
    pub ao_rays: u32,
    pub ao_dist: f32,
    pub padding: usize,
}

impl Default for BakeSettings {
    fn default() -> Self {
        BakeSettings { size: 1024, max_dist: 0.1, ao_rays: 32, ao_dist: 0.5, padding: 8 }
    }
}

#[derive(Clone, Copy, Default)]
struct Texel {
    ok: bool,
    p: V,
    n: V,
    t: V,
    b: V,
}

#[derive(Clone, Copy, Default)]
struct HighInfo {
    ok: bool,
    h: V,
    n: V,
    group: u32,
}

fn hammersley(i: u32, n: u32) -> (f32, f32) {
    let mut b = i;
    b = (b << 16) | (b >> 16);
    b = ((b & 0x5555_5555) << 1) | ((b & 0xAAAA_AAAA) >> 1);
    b = ((b & 0x3333_3333) << 2) | ((b & 0xCCCC_CCCC) >> 2);
    b = ((b & 0x0F0F_0F0F) << 4) | ((b & 0xF0F0_F0F0) >> 4);
    b = ((b & 0x00FF_00FF) << 8) | ((b & 0xFF00_FF00) >> 8);
    (i as f32 / n as f32, b as f32 * 2.328_306_4e-10)
}

pub fn bake(low: &Mesh, high: &Mesh, s: &BakeSettings) -> Result<BTreeMap<String, Tex>, String> {
    let size = s.size.clamp(16, 8192);
    if low.tris.is_empty() || high.tris.is_empty() {
        return Err("both meshes need faces".into());
    }
    if low.tris.iter().all(|t| t.uv.iter().all(|u| u[0] == 0.0 && u[1] == 0.0)) {
        return Err("the low-poly mesh has no UV coordinates".into());
    }
    let bvh = Bvh::build(&high.tris);
    // bounds of the high mesh (for the position map)
    let (mut bmin, mut bmax) = ([f32::MAX; 3], [f32::MIN; 3]);
    for t in &high.tris {
        for p in &t.p {
            for k in 0..3 {
                bmin[k] = bmin[k].min(p[k]);
                bmax[k] = bmax[k].max(p[k]);
            }
        }
    }
    let diag = len(sub(bmax, bmin)).max(1e-6);
    let (max_dist, ao_dist) = (s.max_dist * diag, s.ao_dist * diag);

    // 1. rasterise the low-poly mesh into UV space
    let mut texels = vec![Texel::default(); size * size];
    for t in &low.tris {
        let px: Vec<[f32; 2]> = t.uv.iter().map(|u| [u[0] * size as f32, (1.0 - u[1]) * size as f32]).collect();
        let (minx, maxx) = (px.iter().map(|p| p[0]).fold(f32::MAX, f32::min), px.iter().map(|p| p[0]).fold(f32::MIN, f32::max));
        let (miny, maxy) = (px.iter().map(|p| p[1]).fold(f32::MAX, f32::min), px.iter().map(|p| p[1]).fold(f32::MIN, f32::max));
        let den = (px[1][1] - px[2][1]) * (px[0][0] - px[2][0]) + (px[2][0] - px[1][0]) * (px[0][1] - px[2][1]);
        if den.abs() < 1e-9 {
            continue;
        }
        // tangent frame from UV derivatives (v up)
        let e1 = sub(t.p[1], t.p[0]);
        let e2 = sub(t.p[2], t.p[0]);
        if len(cross(e1, e2)) < 1e-4 * len(e1) * len(e2) || len(cross(e1, e2)) < 1e-10 {
            continue; // zero-area triangle (e.g. sphere poles): padding fills it in
        }
        let (du1, dv1) = (t.uv[1][0] - t.uv[0][0], t.uv[1][1] - t.uv[0][1]);
        let (du2, dv2) = (t.uv[2][0] - t.uv[0][0], t.uv[2][1] - t.uv[0][1]);
        let r = du1 * dv2 - du2 * dv1;
        let (tan, bit) = if r.abs() > 1e-12 {
            (mul(sub(mul(e1, dv2), mul(e2, dv1)), 1.0 / r), mul(sub(mul(e2, du1), mul(e1, du2)), 1.0 / r))
        } else {
            ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0])
        };
        let x0 = (minx.floor().max(0.0)) as usize;
        let x1 = (maxx.ceil().min(size as f32)) as usize;
        let y0 = (miny.floor().max(0.0)) as usize;
        let y1 = (maxy.ceil().min(size as f32)) as usize;
        for y in y0..y1 {
            for x in x0..x1 {
                let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
                let l0 = ((px[1][1] - px[2][1]) * (cx - px[2][0]) + (px[2][0] - px[1][0]) * (cy - px[2][1])) / den;
                let l1 = ((px[2][1] - px[0][1]) * (cx - px[2][0]) + (px[0][0] - px[2][0]) * (cy - px[2][1])) / den;
                let l2 = 1.0 - l0 - l1;
                if l0 < -1e-4 || l1 < -1e-4 || l2 < -1e-4 {
                    continue;
                }
                let p = add(add(mul(t.p[0], l0), mul(t.p[1], l1)), mul(t.p[2], l2));
                let n = norm(add(add(mul(t.n[0], l0), mul(t.n[1], l1)), mul(t.n[2], l2)));
                let tt = norm(sub(tan, mul(n, dot(n, tan))));
                let mut bb = cross(n, tt);
                if dot(bb, bit) < 0.0 {
                    bb = mul(bb, -1.0);
                }
                texels[y * size + x] = Texel { ok: true, p, n, t: tt, b: bb };
            }
        }
    }

    // 2. project each texel onto the high-poly mesh
    let hits: Vec<HighInfo> = par_map(size * size, |i| {
        let tx = texels[i];
        if !tx.ok {
            return HighInfo::default();
        }
        let o = add(tx.p, mul(tx.n, max_dist));
        match bvh.intersect(&high.tris, o, mul(tx.n, -1.0), 2.0 * max_dist) {
            Some(h) => {
                let tr = &high.tris[h.tri as usize];
                let hp = add(o, mul(mul(tx.n, -1.0), h.t));
                HighInfo { ok: true, h: hp, n: interp_n(tr, &h), group: tr.group }
            }
            None => HighInfo::default(),
        }
    });

    let cover: Vec<bool> = texels.iter().map(|t| t.ok).collect();
    let mk = |f: &(dyn Fn(usize) -> [f32; 4] + Sync)| -> Vec<[f32; 4]> { par_map(size * size, |i| f(i)) };

    let tangent_n = mk(&|i| {
        let (tx, h) = (texels[i], hits[i]);
        if !tx.ok {
            return [0.5, 0.5, 1.0, 1.0];
        }
        if !h.ok {
            return [0.5, 0.5, 1.0, 1.0];
        }
        [dot(h.n, tx.t) * 0.5 + 0.5, dot(h.n, tx.b) * 0.5 + 0.5, dot(h.n, tx.n) * 0.5 + 0.5, 1.0]
    });
    let world_n = mk(&|i| {
        let (tx, h) = (texels[i], hits[i]);
        let n = if h.ok { h.n } else { tx.n };
        if !tx.ok {
            return [0.5, 0.5, 1.0, 1.0];
        }
        [n[0] * 0.5 + 0.5, n[1] * 0.5 + 0.5, n[2] * 0.5 + 0.5, 1.0]
    });
    let position = mk(&|i| {
        let tx = texels[i];
        if !tx.ok {
            return [0.0, 0.0, 0.0, 1.0];
        }
        let p = if hits[i].ok { hits[i].h } else { tx.p };
        let f = |k: usize| ((p[k] - bmin[k]) / (bmax[k] - bmin[k]).max(1e-9)).clamp(0.0, 1.0);
        [f(0), f(1), f(2), 1.0]
    });
    let rays = s.ao_rays.clamp(1, 512);
    let ao = mk(&|i| {
        let (tx, h) = (texels[i], hits[i]);
        if !tx.ok {
            return [1.0, 1.0, 1.0, 1.0];
        }
        let (o, n) = if h.ok { (h.h, h.n) } else { (tx.p, tx.n) };
        let o = add(o, mul(n, 1e-3 * diag));
        let up = if n[1].abs() < 0.99 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
        let tg = norm(cross(up, n));
        let bt = cross(n, tg);
        let rot = rnd((i % size) as i32, (i / size) as i32, 7) * std::f32::consts::TAU;
        let mut occ = 0.0;
        for k in 0..rays {
            let (a, b) = hammersley(k, rays);
            let phi = a * std::f32::consts::TAU + rot;
            let ct = (1.0 - b).sqrt();
            let st = b.sqrt();
            let d = add(add(mul(tg, st * phi.cos()), mul(bt, st * phi.sin())), mul(n, ct));
            if bvh.intersect(&high.tris, o, d, ao_dist).is_some() {
                occ += 1.0;
            }
        }
        let v = 1.0 - occ / rays as f32;
        [v, v, v, 1.0]
    });
    let thick = mk(&|i| {
        let (tx, h) = (texels[i], hits[i]);
        if !tx.ok {
            return [0.0, 0.0, 0.0, 1.0];
        }
        if !h.ok {
            return [1.0, 1.0, 1.0, 1.0];
        }
        let o = sub(h.h, mul(h.n, 1e-3 * diag));
        let v = match bvh.intersect(&high.tris, o, mul(h.n, -1.0), ao_dist) {
            Some(x) => (x.t / ao_dist).clamp(0.0, 1.0),
            None => 1.0,
        };
        [v, v, v, 1.0]
    });
    let ids = mk(&|i| {
        let h = hits[i];
        if !h.ok {
            return [0.0, 0.0, 0.0, 1.0];
        }
        let g = h.group as i32;
        [0.2 + 0.8 * rnd(g, 1, 11), 0.2 + 0.8 * rnd(g, 2, 11), 0.2 + 0.8 * rnd(g, 3, 11), 1.0]
    });
    let curv = {
        let at = |x: i64, y: i64| -> [f32; 4] {
            let (xi, yi) = (x.rem_euclid(size as i64) as usize, y.rem_euclid(size as i64) as usize);
            tangent_n[yi * size + xi]
        };
        mk(&|i| {
            let (x, y) = ((i % size) as i64, (i / size) as i64);
            if !texels[i].ok {
                return [0.5, 0.5, 0.5, 1.0];
            }
            let div = (at(x + 1, y)[0] - at(x - 1, y)[0]) - (at(x, y + 1)[1] - at(x, y - 1)[1]);
            let v = (0.5 + div * 1.5).clamp(0.0, 1.0);
            [v, v, v, 1.0]
        })
    };

    let mut maps: BTreeMap<String, Vec<[f32; 4]>> = BTreeMap::new();
    maps.insert("normal".into(), tangent_n);
    maps.insert("world_normal".into(), world_n);
    maps.insert("position".into(), position);
    maps.insert("ambient_occlusion".into(), ao);
    maps.insert("thickness".into(), thick);
    maps.insert("curvature".into(), curv);
    maps.insert("id".into(), ids);

    // 3. padding (dilate into empty texels to avoid seams)
    let mut out = BTreeMap::new();
    for (name, mut px) in maps {
        let mut mask = cover.clone();
        for _ in 0..s.padding.min(64) {
            let prev = px.clone();
            let pm = mask.clone();
            for y in 0..size {
                for x in 0..size {
                    if pm[y * size + x] {
                        continue;
                    }
                    'n: for dy in -1i64..=1 {
                        for dx in -1i64..=1 {
                            let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                            if nx < 0 || ny < 0 || nx >= size as i64 || ny >= size as i64 {
                                continue;
                            }
                            let j = ny as usize * size + nx as usize;
                            if pm[j] {
                                px[y * size + x] = prev[j];
                                mask[y * size + x] = true;
                                break 'n;
                            }
                        }
                    }
                }
            }
        }
        out.insert(name, Tex { w: size, h: size, px });
    }
    Ok(out)
}

/// A unit sphere with UVs, handy for testing the baker and 3D viewer.
pub fn uv_sphere_obj(lon: usize, lat: usize, radius: f32, bump: f32) -> String {
    let mut s = String::new();
    for j in 0..=lat {
        for i in 0..=lon {
            let (u, v) = (i as f32 / lon as f32, j as f32 / lat as f32);
            let (th, ph) = (u * std::f32::consts::TAU, v * std::f32::consts::PI);
            let r = radius * (1.0 + bump * (th * 6.0).sin() * (ph * 5.0).sin());
            if j == 0 || j == lat {
                s.push_str(&format!("v 0 {} 0\n", if j == 0 { r } else { -r }));
            } else {
                s.push_str(&format!("v {} {} {}\n", r * ph.sin() * th.sin(), r * ph.cos(), r * ph.sin() * th.cos()));
            }
            s.push_str(&format!("vt {} {}\n", u, 1.0 - v));
        }
    }
    let w = lon + 1;
    for j in 0..lat {
        for i in 0..lon {
            let (a, b, c, d) = (j * w + i + 1, j * w + i + 2, (j + 1) * w + i + 2, (j + 1) * w + i + 1);
            s.push_str(&format!("f {a}/{a} {d}/{d} {c}/{c}\nf {a}/{a} {c}/{c} {b}/{b}\n"));
        }
    }
    s
}

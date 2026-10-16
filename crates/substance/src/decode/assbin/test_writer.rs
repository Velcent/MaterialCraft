//! Test-only writer for small synthetic assbin files in Painter's layout.

pub struct TestMesh<'a> {
    pub name: &'a str,
    pub material_index: u32,
    pub vertices: u32,
    pub triangles: u32,
    pub uv_sets: u32,
    pub color_sets: u32,
}

#[derive(Default)]
pub struct Buf(pub Vec<u8>);

impl Buf {
    pub fn u16(&mut self, v: u16) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn u32(&mut self, v: u32) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn zeros(&mut self, n: usize) -> &mut Self {
        self.0.resize(self.0.len() + n, 0);
        self
    }
    pub fn string(&mut self, s: &str) -> &mut Self {
        self.u32(s.len() as u32);
        self.0.extend_from_slice(s.as_bytes());
        self
    }
    /// Appends `content` as a block whose `u64` size counts itself.
    pub fn block(&mut self, content: Buf) -> &mut Self {
        self.0.extend_from_slice(&(content.0.len() as u64 + 8).to_le_bytes());
        self.0.extend_from_slice(&content.0);
        self
    }
}

pub fn header(minor: u32, compressed: bool) -> Buf {
    let mut b = Buf::default();
    let mut signature = b"ASSIMP.binary-dump.Thu Jan  1 00:00:00 1970\n".to_vec();
    signature.resize(44, 0);
    b.0.extend_from_slice(&signature);
    b.u32(1).u32(minor).u32(0).u32(0x11).u16(0).u16(compressed as u16);
    b.0.extend_from_slice(b"file:///test.fbx");
    b.zeros(256 - 16 + 128);
    b.0.resize(512, 0xcd);
    b
}

fn mesh(m: &TestMesh) -> Buf {
    let mut b = Buf::default();
    let mut components = 0x1 | 0x2 | 0x4;
    for n in 0..m.uv_sets {
        components |= 0x100 << n;
    }
    for n in 0..m.color_sets {
        components |= 0x1_0000 << n;
    }
    b.string(m.name).u32(4).u32(m.material_index).u32(components);
    let vec3s = m.vertices as usize * 12;
    for _ in 0..4 {
        b.u32(m.vertices).zeros(vec3s);
    }
    for _ in 0..m.color_sets {
        b.u32(m.vertices).zeros(m.vertices as usize * 16);
    }
    for _ in 0..m.uv_sets {
        b.u32(2).u32(m.vertices).zeros(vec3s);
    }
    b.u32(m.triangles);
    for _ in 0..m.triangles {
        b.u32(0).u32(3).u32(0).u32(1).u32(2);
    }
    b.u32(0); // bones
    b
}

fn property(key: &str, kind: u32, data: Buf) -> Buf {
    let mut b = Buf::default();
    b.string(key).u32(0).u32(0).u32(kind).u32(data.0.len() as u32);
    b.0.extend_from_slice(&data.0);
    b
}

fn material(name: &str) -> Buf {
    let mut colour = Buf::default();
    colour.zeros(12);
    let mut text = Buf::default();
    text.string(name).zeros(1);
    let mut b = Buf::default();
    b.u32(2)
        .block(property("$clr.diffuse", 1, colour))
        .block(property("?mat.name", 3, text));
    b
}

/// The scene block: flags, a root node, meshes, materials and four empty trailing lists.
pub fn scene(meshes: &[TestMesh], materials: &[&str]) -> Buf {
    let mut node = Buf::default();
    node.string("RootNode").zeros(64).u32(0).u32(0).u32(0);
    let mut s = Buf::default();
    s.u32(8).block(node).u32(meshes.len() as u32);
    for m in meshes {
        s.block(mesh(m));
    }
    s.u32(materials.len() as u32);
    for name in materials {
        s.block(material(name));
    }
    s.zeros(16);
    let mut out = Buf::default();
    out.block(s);
    out
}

/// A complete uncompressed file.
pub fn file(meshes: &[TestMesh], materials: &[&str]) -> Vec<u8> {
    let mut out = header(7, false);
    out.0.extend_from_slice(&scene(meshes, materials).0);
    out.0
}

/// A complete compressed file, using a zlib stream of stored DEFLATE blocks.
pub fn compressed_file(meshes: &[TestMesh], materials: &[&str]) -> Vec<u8> {
    let raw = scene(meshes, materials).0;
    let mut out = header(7, true);
    out.u32(raw.len() as u32);
    out.0.extend_from_slice(&[0x78, 0x01]);
    let chunks: Vec<&[u8]> = raw.chunks(0xffff).collect();
    for (i, chunk) in chunks.iter().enumerate() {
        out.0.push((i + 1 == chunks.len()) as u8);
        out.u16(chunk.len() as u16).u16(!(chunk.len() as u16));
        out.0.extend_from_slice(chunk);
    }
    out.zeros(4); // Adler-32, not checked
    out.0
}

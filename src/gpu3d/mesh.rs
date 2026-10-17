//! One triangle mesh on the GPU: interleaved vertices and u32 indices in its own vertex array.
use super::bytes_of;
use crate::render3d::Mesh;
use eframe::egui_glow::glow;
use glow::HasContext;

/// Floats per vertex: position, normal, tangent, uv, (su, sv, displacement fade), bitangent sign.
const STRIDE: usize = 15;

pub(super) struct GpuMesh {
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    ebo: glow::Buffer,
    index_count: i32,
}

impl GpuMesh {
    /// `tangent_w`: bitangent sign per vertex (-1 on mirrored uv islands); empty = all +1.
    pub unsafe fn upload(gl: &glow::Context, m: &Mesh, tangent_w: &[f32]) -> Result<GpuMesh, String> { unsafe {
        let mut data: Vec<f32> = Vec::with_capacity(m.pos.len() * STRIDE);
        for i in 0..m.pos.len() {
            data.extend_from_slice(&m.pos[i]);
            data.extend_from_slice(&m.nrm[i]);
            data.extend_from_slice(&m.tan[i]);
            data.extend_from_slice(&m.uv[i]);
            data.extend_from_slice(&[m.su[i], m.sv[i], m.fade[i]]);
            data.push(tangent_w.get(i).copied().unwrap_or(1.0));
        }
        let vao = gl.create_vertex_array()?;
        let vbo = gl.create_buffer()?;
        let ebo = gl.create_buffer()?;
        gl.bind_vertex_array(Some(vao));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes_of(&data), glow::STATIC_DRAW);
        let stride = (STRIDE * 4) as i32;
        for (loc, size, off) in [(0u32, 3, 0), (1, 3, 3), (2, 3, 6), (3, 2, 9), (4, 3, 11), (5, 1, 14)] {
            gl.enable_vertex_attrib_array(loc);
            gl.vertex_attrib_pointer_f32(loc, size, glow::FLOAT, false, stride, off * 4);
        }
        gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ebo));
        gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, bytes_of(&m.tris), glow::STATIC_DRAW);
        gl.bind_vertex_array(None);
        Ok(GpuMesh { vao, vbo, ebo, index_count: (m.tris.len() * 3) as i32 })
    }}

    pub unsafe fn draw(&self, gl: &glow::Context) { unsafe {
        gl.bind_vertex_array(Some(self.vao));
        gl.draw_elements(glow::TRIANGLES, self.index_count, glow::UNSIGNED_INT, 0);
    }}

    pub unsafe fn free(&self, gl: &glow::Context) { unsafe {
        gl.delete_vertex_array(self.vao);
        gl.delete_buffer(self.vbo);
        gl.delete_buffer(self.ebo);
    }}
}

/// Vertices per uv unit along u and v, estimated from triangle count and covered uv area (an n x n grid of quads over
/// one uv unit gives n). Drives the mip level the vertex shader samples the height map at.
pub(super) fn uv_density(m: &Mesh) -> f32 {
    let area: f64 = m
        .tris
        .iter()
        .map(|t| {
            let (a, b, c) = (m.uv[t[0] as usize], m.uv[t[1] as usize], m.uv[t[2] as usize]);
            (((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])) as f64).abs() * 0.5
        })
        .sum();
    ((m.tris.len() as f64 / (2.0 * area.max(1e-6))).sqrt() as f32).max(1.0)
}

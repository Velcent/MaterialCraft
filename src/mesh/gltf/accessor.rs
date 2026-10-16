//! Reading accessor data out of the binary buffer, with every offset and length checked before
//! anything is allocated.

use super::error::GltfError;
use crate::json::Value;

const BYTE: u64 = 5120;
const UNSIGNED_BYTE: u64 = 5121;
const SHORT: u64 = 5122;
const UNSIGNED_SHORT: u64 = 5123;
const UNSIGNED_INT: u64 = 5125;
const FLOAT: u64 = 5126;

/// Resolves accessors of one glTF document against its GLB binary chunk.
pub(super) struct Accessors<'a> {
    root: &'a Value,
    bin: &'a [u8],
}

/// Where an accessor's elements are: `count` elements of `size` bytes, `stride` apart.
struct Layout<'a> {
    data: &'a [u8],
    count: usize,
    stride: usize,
    component: u64,
    normalized: bool,
}

impl<'a> Accessors<'a> {
    /// `bin` is the GLB binary chunk (empty if there is none).
    pub fn new(root: &'a Value, bin: &'a [u8]) -> Result<Accessors<'a>, GltfError> {
        let buffers = root.get("buffers").and_then(Value::as_array);
        for (i, b) in buffers.into_iter().flatten().enumerate() {
            if b.get("uri").is_some() {
                return Err(GltfError::Unsupported(format!(
                    "buffers[{i}] refers to external data; only GLB-embedded buffers are read"
                )));
            }
            let len = b.get("byteLength").and_then(Value::as_usize);
            if i > 0 || len.is_none_or(|l| l > bin.len()) {
                return Err(GltfError::invalid(
                    format!("buffers[{i}]"),
                    "byteLength does not match the GLB binary chunk",
                ));
            }
        }
        Ok(Accessors { root, bin })
    }

    /// Float vectors of `N` components. Normalised integer components are accepted too (as
    /// allowed for texture coordinates).
    pub fn vectors<const N: usize>(&self, index: usize) -> Result<Vec<[f32; N]>, GltfError> {
        let at = format!("accessors[{index}]");
        let l = self.layout(index, N)?;
        if l.component != FLOAT && !l.normalized {
            return Err(GltfError::invalid(
                at,
                "expected float or normalised components",
            ));
        }
        let size = component_size(l.component);
        let mut out = Vec::with_capacity(l.count);
        for e in 0..l.count {
            let mut v = [0.0f32; N];
            for (c, slot) in v.iter_mut().enumerate() {
                *slot = decode(l.component, &l.data[e * l.stride + c * size..]);
            }
            out.push(v);
        }
        Ok(out)
    }

    /// Unsigned integer scalars (indices).
    pub fn indices(&self, index: usize) -> Result<Vec<u32>, GltfError> {
        let l = self.layout(index, 1)?;
        if ![UNSIGNED_BYTE, UNSIGNED_SHORT, UNSIGNED_INT].contains(&l.component) || l.normalized {
            return Err(GltfError::invalid(
                format!("accessors[{index}]"),
                "indices must be unsigned byte, short or int",
            ));
        }
        Ok((0..l.count)
            .map(|e| {
                let b = &l.data[e * l.stride..];
                match l.component {
                    UNSIGNED_BYTE => b[0] as u32,
                    UNSIGNED_SHORT => u16::from_le_bytes([b[0], b[1]]) as u32,
                    _ => u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
                }
            })
            .collect())
    }

    /// Number of elements of accessor `index`.
    pub fn count(&self, index: usize) -> Result<usize, GltfError> {
        let a = self.accessor(index)?;
        a.get("count")
            .and_then(Value::as_usize)
            .ok_or_else(|| GltfError::invalid(format!("accessors[{index}]"), "missing count"))
    }

    fn accessor(&self, index: usize) -> Result<&'a Value, GltfError> {
        self.root
            .get("accessors")
            .and_then(|a| a.at(index))
            .ok_or_else(|| GltfError::invalid(format!("accessors[{index}]"), "does not exist"))
    }

    fn layout(&self, index: usize, components: usize) -> Result<Layout<'a>, GltfError> {
        let at = format!("accessors[{index}]");
        let a = self.accessor(index)?;
        let inv = |m: &str| GltfError::invalid(at.clone(), m);
        if a.get("sparse").is_some() {
            return Err(GltfError::Unsupported(format!("{at} is sparse")));
        }
        let kind = a.get("type").and_then(Value::as_str).unwrap_or("");
        if type_components(kind) != Some(components) {
            return Err(inv(&format!(
                "type {kind:?}, expected {components} components"
            )));
        }
        let component = a.get("componentType").and_then(Value::as_u64).unwrap_or(0);
        let size = component_size(component);
        if size == 0 {
            return Err(inv("unknown componentType"));
        }
        let count = self.count(index)?;
        let normalized = a.get("normalized").and_then(Value::as_bool) == Some(true);
        let offset = usize_or(a.get("byteOffset"), 0).ok_or_else(|| inv("bad byteOffset"))?;
        let Some(view_index) = a.get("bufferView").and_then(Value::as_usize) else {
            return Err(GltfError::Unsupported(format!("{at} has no bufferView")));
        };
        let view = self.view(view_index)?;
        let elem = size * components;
        let stride = view.1.unwrap_or(elem);
        if stride < elem {
            return Err(inv("byteStride is smaller than an element"));
        }
        let end = match count {
            0 => Some(offset),
            n => (n - 1)
                .checked_mul(stride)
                .and_then(|b| b.checked_add(elem))
                .and_then(|b| b.checked_add(offset)),
        }
        .filter(|&end| end <= view.0.len())
        .ok_or_else(|| inv("data extends past its bufferView"))?;
        Ok(Layout {
            data: &view.0[offset..end],
            count,
            stride,
            component,
            normalized,
        })
    }

    /// The bytes of buffer view `index` and its stride.
    fn view(&self, index: usize) -> Result<(&'a [u8], Option<usize>), GltfError> {
        let at = format!("bufferViews[{index}]");
        let v = self
            .root
            .get("bufferViews")
            .and_then(|v| v.at(index))
            .ok_or_else(|| GltfError::invalid(at.clone(), "does not exist"))?;
        let inv = |m: &str| GltfError::invalid(at.clone(), m);
        if v.get("buffer").and_then(Value::as_usize) != Some(0) {
            return Err(inv("must use buffer 0 (the GLB binary chunk)"));
        }
        let offset = usize_or(v.get("byteOffset"), 0).ok_or_else(|| inv("bad byteOffset"))?;
        let len = v
            .get("byteLength")
            .and_then(Value::as_usize)
            .ok_or_else(|| inv("missing byteLength"))?;
        let stride = match v.get("byteStride") {
            None => None,
            Some(s) => Some(s.as_usize().ok_or_else(|| inv("bad byteStride"))?),
        };
        let data = offset
            .checked_add(len)
            .and_then(|end| self.bin.get(offset..end))
            .ok_or_else(|| inv("extends past the binary chunk"))?;
        Ok((data, stride))
    }
}

fn usize_or(v: Option<&Value>, default: usize) -> Option<usize> {
    v.map_or(Some(default), Value::as_usize)
}

fn type_components(kind: &str) -> Option<usize> {
    Some(match kind {
        "SCALAR" => 1,
        "VEC2" => 2,
        "VEC3" => 3,
        "VEC4" => 4,
        _ => return None,
    })
}

fn component_size(component: u64) -> usize {
    match component {
        BYTE | UNSIGNED_BYTE => 1,
        SHORT | UNSIGNED_SHORT => 2,
        UNSIGNED_INT | FLOAT => 4,
        _ => 0,
    }
}

/// One component as float; integer types are normalised as the glTF spec defines.
fn decode(component: u64, b: &[u8]) -> f32 {
    match component {
        FLOAT => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        UNSIGNED_BYTE => b[0] as f32 / 255.0,
        BYTE => (b[0] as i8 as f32 / 127.0).max(-1.0),
        UNSIGNED_SHORT => u16::from_le_bytes([b[0], b[1]]) as f32 / 65535.0,
        SHORT => (i16::from_le_bytes([b[0], b[1]]) as f32 / 32767.0).max(-1.0),
        _ => u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32,
    }
}

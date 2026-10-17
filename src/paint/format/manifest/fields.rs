//! [`Obj`]: typed, path-aware access to a manifest object that remembers which keys were read,
//! so the rest can be kept as unknown fields. Plus [`num`], exact `f32` → JSON numbers.

use crate::json::Value;
use crate::paint::format::FormatError;

/// One JSON object being read. Errors name the object's path, e.g. `texture_sets[0].layers[2]`.
pub struct Obj<'v> {
    entries: &'v [(String, Value)],
    at: String,
    read: Vec<bool>,
}

impl<'v> Obj<'v> {
    pub fn new(v: &'v Value, at: impl Into<String>) -> Result<Obj<'v>, FormatError> {
        let at = at.into();
        let entries = v.as_object().ok_or_else(|| FormatError::manifest(at.clone(), "expected an object"))?;
        Ok(Obj { entries, read: vec![false; entries.len()], at })
    }

    /// Path of this object.
    pub fn at(&self) -> &str {
        &self.at
    }

    /// Path of `key` in this object.
    pub fn path(&self, key: &str) -> String {
        if self.at.is_empty() { key.to_string() } else { format!("{}.{key}", self.at) }
    }

    pub fn error(&self, key: &str, message: impl Into<String>) -> FormatError {
        FormatError::manifest(self.path(key), message)
    }

    /// The value of `key`, marking it as read.
    pub fn opt(&mut self, key: &str) -> Option<&'v Value> {
        let i = self.entries.iter().position(|(k, _)| k == key)?;
        self.read[i] = true;
        Some(&self.entries[i].1)
    }

    pub fn req(&mut self, key: &str) -> Result<&'v Value, FormatError> {
        self.opt(key).ok_or_else(|| self.error(key, "missing"))
    }

    pub fn str(&mut self, key: &str) -> Result<&'v str, FormatError> {
        let v = self.req(key)?;
        v.as_str().ok_or_else(|| self.error(key, "expected a string"))
    }

    pub fn opt_str(&mut self, key: &str) -> Result<Option<&'v str>, FormatError> {
        match self.opt(key) {
            None => Ok(None),
            Some(v) => v.as_str().map(Some).ok_or_else(|| self.error(key, "expected a string")),
        }
    }

    pub fn usize(&mut self, key: &str) -> Result<usize, FormatError> {
        let v = self.req(key)?;
        v.as_usize().ok_or_else(|| self.error(key, "expected a non-negative integer"))
    }

    pub fn bool_or(&mut self, key: &str, default: bool) -> Result<bool, FormatError> {
        match self.opt(key) {
            None => Ok(default),
            Some(v) => v.as_bool().ok_or_else(|| self.error(key, "expected true or false")),
        }
    }

    pub fn f32_or(&mut self, key: &str, default: f32) -> Result<f32, FormatError> {
        match self.opt(key) {
            None => Ok(default),
            Some(v) => as_f32(v).ok_or_else(|| self.error(key, "expected a number")),
        }
    }

    /// An array (missing = empty).
    pub fn array(&mut self, key: &str) -> Result<&'v [Value], FormatError> {
        match self.opt(key) {
            None => Ok(&[]),
            Some(v) => v.as_array().map(Vec::as_slice).ok_or_else(|| self.error(key, "expected an array")),
        }
    }

    /// An object's entries (missing = empty).
    pub fn entries(&mut self, key: &str) -> Result<&'v [(String, Value)], FormatError> {
        match self.opt(key) {
            None => Ok(&[]),
            Some(v) => v.as_object().map(Vec::as_slice).ok_or_else(|| self.error(key, "expected an object")),
        }
    }

    /// `[width, height]`, both at least 1.
    pub fn size(&mut self, key: &str) -> Result<(u32, u32), FormatError> {
        let v = self.req(key)?;
        let dims: Option<Vec<u32>> =
            v.as_array().filter(|a| a.len() == 2).and_then(|a| a.iter().map(|d| d.as_u64().and_then(|d| u32::try_from(d).ok())).collect());
        match dims.as_deref() {
            Some(&[w, h]) if w > 0 && h > 0 => Ok((w, h)),
            _ => Err(self.error(key, "expected [width, height] of positive integers")),
        }
    }

    /// The keys not read so far, in document order.
    pub fn unread(&self) -> Vec<(String, Value)> {
        self.entries.iter().zip(&self.read).filter(|(_, r)| !**r).map(|(e, _)| e.clone()).collect()
    }
}

/// A finite JSON number as `f32` (rounded to nearest).
pub fn as_f32(v: &Value) -> Option<f32> {
    v.as_f64().map(|n| n as f32).filter(|n| n.is_finite())
}

/// `x` as a JSON number that reads back as exactly `x`, written as short as possible
/// (`0.7`, not `0.699999988079071`).
pub fn num(x: f32) -> Value {
    let short: Option<f64> = format!("{x}").parse().ok();
    match short {
        Some(s) if (s as f32).to_bits() == x.to_bits() => Value::Number(s),
        _ => Value::Number(x as f64),
    }
}

/// Four numbers as a JSON array.
pub fn num4(v: [f32; 4]) -> Value {
    Value::Array(v.into_iter().map(num).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_short_and_exact() {
        assert_eq!(num(0.7).to_string(), "0.7");
        assert_eq!(num(1.0).to_string(), "1");
        let mut s = 1u32;
        for _ in 0..100_000 {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let x = f32::from_bits(s);
            if x.is_finite() {
                let back = crate::json::parse(&num(x).to_string()).unwrap();
                assert_eq!(as_f32(&back).map(f32::to_bits), Some(x.to_bits()), "{x:e}");
            }
        }
    }

    #[test]
    fn tracks_unread_keys() {
        let v = crate::json::parse(r#"{"a": 1, "b": "x", "c": [1, 2]}"#).unwrap();
        let mut o = Obj::new(&v, "root").unwrap();
        assert_eq!(o.usize("a").unwrap(), 1);
        assert!(o.usize("b").is_err());
        assert_eq!(o.size("c").unwrap(), (1, 2));
        assert!(o.unread().is_empty());
        let mut o = Obj::new(&v, "").unwrap();
        o.opt("a");
        assert_eq!(o.unread().len(), 2);
        assert!(matches!(o.str("zz"), Err(FormatError::Manifest { at, .. }) if at == "zz"));
    }
}

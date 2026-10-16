//! Minimal JSON (RFC 8259): an order-preserving value tree, a strict parser and compact/pretty writers.
//!
//! ```
//! use materialcraft::json::{self, Value};
//! let v = json::parse(r#"{"name": "vase", "size": [2048, 2048]}"#).unwrap();
//! assert_eq!(v["size"][0].as_u64(), Some(2048));
//! let doc = Value::object().with("name", "vase").with("layers", Value::array());
//! assert_eq!(doc.to_string(), r#"{"name":"vase","layers":[]}"#);
//! ```

mod parse;
mod write;

pub use parse::{parse, JsonError, MAX_DEPTH};

/// A JSON value. Objects keep their keys in insertion (or document) order.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum Value {
    #[default]
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

/// Largest integer an f64 holds exactly (2^53).
const MAX_SAFE_INT: f64 = 9_007_199_254_740_992.0;

impl Value {
    /// An empty object.
    pub fn object() -> Value {
        Value::Object(Vec::new())
    }

    /// An empty array.
    pub fn array() -> Value {
        Value::Array(Vec::new())
    }

    /// Builder: set `key` on an object (replacing an existing entry in place). No effect on other values.
    pub fn with(mut self, key: impl Into<String>, value: impl Into<Value>) -> Value {
        self.insert(key, value);
        self
    }

    /// Set `key` on an object, replacing an existing entry in place or appending. Returns false if not an object.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<Value>) -> bool {
        let Value::Object(entries) = self else {
            return false;
        };
        let (key, value) = (key.into(), value.into());
        match entries.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => slot.1 = value,
            None => entries.push((key, value)),
        }
        true
    }

    /// Append to an array. Returns false if not an array.
    pub fn push(&mut self, value: impl Into<Value>) -> bool {
        let Value::Array(items) = self else {
            return false;
        };
        items.push(value.into());
        true
    }

    /// Remove `key` from an object, returning its value.
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        let Value::Object(entries) = self else {
            return None;
        };
        let i = entries.iter().position(|(k, _)| k == key)?;
        Some(entries.remove(i).1)
    }

    /// Member of an object. With duplicate keys the last one wins, as in JavaScript.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.as_object()?
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.as_object_mut()?
            .iter_mut()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }

    /// Element of an array.
    pub fn at(&self, index: usize) -> Option<&Value> {
        self.as_array()?.get(index)
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// The number if it is a non-negative integer that an f64 holds exactly (at most 2^53).
    pub fn as_u64(&self) -> Option<u64> {
        let n = self.as_f64()?;
        (n.fract() == 0.0 && (0.0..=MAX_SAFE_INT).contains(&n)).then_some(n as u64)
    }

    /// The number if it is an integer that an f64 holds exactly (magnitude at most 2^53).
    pub fn as_i64(&self) -> Option<i64> {
        let n = self.as_f64()?;
        (n.fract() == 0.0 && n.abs() <= MAX_SAFE_INT).then_some(n as i64)
    }

    pub fn as_usize(&self) -> Option<usize> {
        self.as_u64().and_then(|n| usize::try_from(n).ok())
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&Vec<Value>> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_array_mut(&mut self) -> Option<&mut Vec<Value>> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&Vec<(String, Value)>> {
        match self {
            Value::Object(o) => Some(o),
            _ => None,
        }
    }

    pub fn as_object_mut(&mut self) -> Option<&mut Vec<(String, Value)>> {
        match self {
            Value::Object(o) => Some(o),
            _ => None,
        }
    }

    /// Indented with two spaces; arrays holding only numbers, booleans and nulls stay on one line.
    pub fn to_string_pretty(&self) -> String {
        let mut out = String::new();
        write::pretty(self, 0, &mut out);
        out
    }
}

/// Compact form (no whitespace); `to_string()` comes from this.
impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut out = String::new();
        write::compact(self, &mut out);
        f.write_str(&out)
    }
}

/// `value["key"]`: the member, or `Null` when missing or not an object.
impl std::ops::Index<&str> for Value {
    type Output = Value;
    fn index(&self, key: &str) -> &Value {
        self.get(key).unwrap_or(&Value::Null)
    }
}

/// `value[i]`: the element, or `Null` when out of range or not an array.
impl std::ops::Index<usize> for Value {
    type Output = Value;
    fn index(&self, i: usize) -> &Value {
        self.at(i).unwrap_or(&Value::Null)
    }
}

impl std::str::FromStr for Value {
    type Err = JsonError;
    fn from_str(s: &str) -> Result<Value, JsonError> {
        parse(s)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Value {
        Value::Bool(b)
    }
}

macro_rules! from_number {
    ($($t:ty),*) => {$(
        impl From<$t> for Value {
            /// Integers beyond 2^53 lose precision.
            fn from(n: $t) -> Value {
                Value::Number(n as f64)
            }
        }
    )*};
}
from_number!(f64, f32, i8, i16, i32, i64, u8, u16, u32, u64, usize, isize);

impl From<&str> for Value {
    fn from(s: &str) -> Value {
        Value::String(s.to_string())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Value {
        Value::String(s)
    }
}

impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(v: Vec<T>) -> Value {
        Value::Array(v.into_iter().map(Into::into).collect())
    }
}

impl<T: Into<Value> + Clone> From<&[T]> for Value {
    fn from(v: &[T]) -> Value {
        Value::Array(v.iter().cloned().map(Into::into).collect())
    }
}

impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(v: Option<T>) -> Value {
        v.map_or(Value::Null, Into::into)
    }
}

impl<T: Into<Value>> FromIterator<T> for Value {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Value {
        Value::Array(iter.into_iter().map(Into::into).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builders_and_accessors() {
        let mut v = Value::object()
            .with("a", 1)
            .with("b", "x")
            .with("c", vec![1.5, 2.0])
            .with("d", Option::<i32>::None);
        assert!(v.insert("a", true));
        assert_eq!(v.to_string(), r#"{"a":true,"b":"x","c":[1.5,2],"d":null}"#);
        assert_eq!(v["b"].as_str(), Some("x"));
        assert_eq!(v["c"][1].as_u64(), Some(2));
        assert_eq!(v["c"][0].as_u64(), None);
        assert!(v["missing"]["deeper"][3].is_null());
        assert_eq!(v.remove("b"), Some(Value::from("x")));
        assert!(v.get("b").is_none());
        let mut a: Value = (0..3).collect();
        assert!(a.push(-4));
        assert_eq!(a[3].as_i64(), Some(-4));
        assert_eq!(a[3].as_u64(), None);
        assert!(!a.insert("k", 1));
        assert_eq!(
            Value::Number(9_007_199_254_740_993.0).as_u64(),
            Some(9_007_199_254_740_992)
        );
        assert_eq!(Value::Number(1e19).as_u64(), None);
    }

    #[test]
    fn duplicate_keys_last_wins() {
        let v = parse(r#"{"k": 1, "k": 2}"#).unwrap();
        assert_eq!(v["k"].as_u64(), Some(2));
        assert_eq!(v.as_object().unwrap().len(), 2);
    }
}

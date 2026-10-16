//! Serialisation: compact and pretty-printed forms.

use super::Value;
use std::fmt::Write;

const INDENT: &str = "  ";

pub(super) fn compact(v: &Value, out: &mut String) {
    match v {
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                compact(item, out);
            }
            out.push(']');
        }
        Value::Object(entries) => {
            out.push('{');
            for (i, (k, item)) in entries.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                string(k, out);
                out.push(':');
                compact(item, out);
            }
            out.push('}');
        }
        scalar => write_scalar(scalar, out),
    }
}

pub(super) fn pretty(v: &Value, depth: usize, out: &mut String) {
    match v {
        Value::Array(items) if items.is_empty() => out.push_str("[]"),
        Value::Object(entries) if entries.is_empty() => out.push_str("{}"),
        Value::Array(items)
            if items
                .iter()
                .all(|i| matches!(i, Value::Null | Value::Bool(_) | Value::Number(_))) =>
        {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_scalar(item, out);
            }
            out.push(']');
        }
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                out.push_str(if i > 0 { ",\n" } else { "\n" });
                indent(depth + 1, out);
                pretty(item, depth + 1, out);
            }
            out.push('\n');
            indent(depth, out);
            out.push(']');
        }
        Value::Object(entries) => {
            out.push('{');
            for (i, (k, item)) in entries.iter().enumerate() {
                out.push_str(if i > 0 { ",\n" } else { "\n" });
                indent(depth + 1, out);
                string(k, out);
                out.push_str(": ");
                pretty(item, depth + 1, out);
            }
            out.push('\n');
            indent(depth, out);
            out.push('}');
        }
        scalar => write_scalar(scalar, out),
    }
}

fn indent(depth: usize, out: &mut String) {
    for _ in 0..depth {
        out.push_str(INDENT);
    }
}

fn write_scalar(v: &Value, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => number(*n, out),
        Value::String(s) => string(s, out),
        Value::Array(_) | Value::Object(_) => compact(v, out),
    }
}

/// Shortest text that parses back to exactly `n`. Integers print without a fraction; NaN and infinities, which
/// JSON cannot represent, become `null`.
fn number(n: f64, out: &mut String) {
    if !n.is_finite() {
        out.push_str("null");
    } else if n == 0.0 && n.is_sign_negative() {
        out.push_str("-0.0");
    } else if n.fract() == 0.0 && n.abs() < 1e16 {
        let _ = write!(out, "{}", n as i64);
    } else {
        // Debug is shortest-round-trip and switches to exponent notation for very large or small magnitudes.
        let _ = write!(out, "{n:?}");
    }
}

fn string(s: &str, out: &mut String) {
    out.push('"');
    let mut start = 0;
    for (i, c) in s.char_indices() {
        let esc = match c {
            '"' => "\\\"",
            '\\' => "\\\\",
            '\n' => "\\n",
            '\r' => "\\r",
            '\t' => "\\t",
            '\u{8}' => "\\b",
            '\u{c}' => "\\f",
            c if (c as u32) < 0x20 => "",
            _ => continue,
        };
        out.push_str(&s[start..i]);
        if esc.is_empty() {
            let _ = write!(out, "\\u{:04x}", c as u32);
        } else {
            out.push_str(esc);
        }
        start = i + c.len_utf8();
    }
    out.push_str(&s[start..]);
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::super::{parse, Value};

    #[test]
    fn numbers_round_trip_exactly() {
        let cases = [
            0.0,
            -0.0,
            1.0,
            -1.0,
            0.1,
            1.0 / 3.0,
            1e300,
            -2.5e-300,
            5e-324,
            f64::MAX,
            f64::MIN_POSITIVE,
            123456789012345.0,
            1e16,
            9007199254740993.0,
            0.30000000000000004,
        ];
        for n in cases {
            let text = Value::Number(n).to_string();
            let back = parse(&text).unwrap().as_f64().unwrap();
            assert_eq!(back.to_bits(), n.to_bits(), "{n} -> {text}");
        }
        assert_eq!(Value::Number(42.0).to_string(), "42");
        assert_eq!(Value::Number(-7.0).to_string(), "-7");
        assert_eq!(Value::Number(0.5).to_string(), "0.5");
        assert_eq!(Value::Number(1e300).to_string(), "1e300");
        assert_eq!(Value::Number(f64::NAN).to_string(), "null");
    }

    #[test]
    fn strings_escape() {
        let s = "a\"b\\c\nd\u{1}e\u{7f}é😀\u{2028}";
        let text = Value::from(s).to_string();
        assert_eq!(text, "\"a\\\"b\\\\c\\nd\\u0001e\u{7f}é😀\u{2028}\"");
        assert_eq!(parse(&text).unwrap().as_str(), Some(s));
    }

    #[test]
    fn pretty_layout() {
        let v = Value::object()
            .with("name", "vase")
            .with("size", vec![1, 2])
            .with("empty", Value::array())
            .with("layers", vec![Value::object().with("id", 1)]);
        let expect = "{\n  \"name\": \"vase\",\n  \"size\": [1, 2],\n  \"empty\": [],\n  \"layers\": [\n    {\n      \"id\": 1\n    }\n  ]\n}";
        assert_eq!(v.to_string_pretty(), expect);
        assert_eq!(parse(expect).unwrap(), v);
    }
}

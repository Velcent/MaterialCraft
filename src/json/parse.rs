//! Strict RFC 8259 parser with line/column error positions.

use super::Value;

/// Maximum nesting of arrays and objects; deeper documents are rejected instead of exhausting the stack.
pub const MAX_DEPTH: usize = 256;

/// A parse error at a 1-based line and column (columns count characters).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsonError {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

impl std::fmt::Display for JsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "JSON error at line {}, column {}: {}",
            self.line, self.column, self.message
        )
    }
}

impl std::error::Error for JsonError {}

/// Parse one JSON document (surrounding whitespace and a leading byte-order mark are allowed).
pub fn parse(text: &str) -> Result<Value, JsonError> {
    let mut p = Parser {
        s: text,
        b: text.as_bytes(),
        pos: 0,
    };
    if p.b.starts_with("\u{feff}".as_bytes()) {
        p.pos = 3;
    }
    p.skip_ws();
    let v = p.value(0)?;
    p.skip_ws();
    if p.pos < p.b.len() {
        return Err(p.error("unexpected data after the document"));
    }
    Ok(v)
}

struct Parser<'a> {
    s: &'a str,
    b: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn error(&self, msg: &str) -> JsonError {
        self.error_at(self.pos, msg)
    }

    fn error_at(&self, pos: usize, msg: &str) -> JsonError {
        let pos = pos.min(self.b.len());
        let before = &self.b[..pos];
        let line = 1 + before.iter().filter(|&&c| c == b'\n').count();
        let line_start = before
            .iter()
            .rposition(|&c| c == b'\n')
            .map_or(0, |i| i + 1);
        // Count characters, not bytes: skip UTF-8 continuation bytes.
        let column = 1 + before[line_start..]
            .iter()
            .filter(|&&c| c & 0xc0 != 0x80)
            .count();
        JsonError {
            line,
            column,
            message: msg.to_string(),
        }
    }

    fn peek(&self) -> Option<u8> {
        self.b.get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.peek() {
            self.pos += 1;
        }
    }

    fn expect_word(&mut self, word: &str, v: Value) -> Result<Value, JsonError> {
        if self.b[self.pos..].starts_with(word.as_bytes()) {
            self.pos += word.len();
            Ok(v)
        } else {
            Err(self.error("invalid literal"))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, JsonError> {
        match self.peek() {
            None => Err(self.error("unexpected end of input")),
            Some(b'{') => self.object(depth + 1),
            Some(b'[') => self.array(depth + 1),
            Some(b'"') => self.string().map(Value::String),
            Some(b't') => self.expect_word("true", Value::Bool(true)),
            Some(b'f') => self.expect_word("false", Value::Bool(false)),
            Some(b'n') => self.expect_word("null", Value::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.error("expected a value")),
        }
    }

    fn enter(&self, depth: usize) -> Result<(), JsonError> {
        if depth > MAX_DEPTH {
            Err(self.error("nesting too deep"))
        } else {
            Ok(())
        }
    }

    fn array(&mut self, depth: usize) -> Result<Value, JsonError> {
        self.enter(depth)?;
        self.pos += 1;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Value::Array(items));
        }
        loop {
            self.skip_ws();
            items.push(self.value(depth)?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err(self.error("expected ',' or ']'")),
            }
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value, JsonError> {
        self.enter(depth)?;
        self.pos += 1;
        let mut entries = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Value::Object(entries));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some(b'"') {
                return Err(self.error("expected a string key"));
            }
            let key = self.string()?;
            self.skip_ws();
            if self.peek() != Some(b':') {
                return Err(self.error("expected ':'"));
            }
            self.pos += 1;
            self.skip_ws();
            entries.push((key, self.value(depth)?));
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Value::Object(entries));
                }
                _ => return Err(self.error("expected ',' or '}'")),
            }
        }
    }

    fn digits(&mut self) -> usize {
        let start = self.pos;
        while let Some(b'0'..=b'9') = self.peek() {
            self.pos += 1;
        }
        self.pos - start
    }

    fn number(&mut self) -> Result<Value, JsonError> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        match self.peek() {
            Some(b'0') => self.pos += 1,
            Some(b'1'..=b'9') => {
                self.digits();
            }
            _ => return Err(self.error("invalid number")),
        }
        if self.peek() == Some(b'.') {
            self.pos += 1;
            if self.digits() == 0 {
                return Err(self.error("expected digits after '.'"));
            }
        }
        if let Some(b'e' | b'E') = self.peek() {
            self.pos += 1;
            if let Some(b'+' | b'-') = self.peek() {
                self.pos += 1;
            }
            if self.digits() == 0 {
                return Err(self.error("expected exponent digits"));
            }
        }
        // The grammar above is a subset of what Rust's (correctly rounding) float parser accepts.
        let n: f64 = self.s[start..self.pos]
            .parse()
            .map_err(|_| self.error_at(start, "invalid number"))?;
        if !n.is_finite() {
            return Err(self.error_at(start, "number out of range"));
        }
        Ok(Value::Number(n))
    }

    fn hex4(&mut self) -> Result<u32, JsonError> {
        let digits = self
            .b
            .get(self.pos..self.pos + 4)
            .ok_or_else(|| self.error("truncated \\u escape"))?;
        let mut v = 0u32;
        for &d in digits {
            let x = (d as char)
                .to_digit(16)
                .ok_or_else(|| self.error("invalid \\u escape"))?;
            v = v * 16 + x;
        }
        self.pos += 4;
        Ok(v)
    }

    /// A string starting at the opening quote.
    fn string(&mut self) -> Result<String, JsonError> {
        let open = self.pos;
        self.pos += 1;
        let mut out = String::new();
        loop {
            // Copy the run up to the next quote, backslash or control character in one go.
            let run = self.b[self.pos..]
                .iter()
                .position(|&c| c == b'"' || c == b'\\' || c < 0x20)
                .ok_or_else(|| self.error_at(open, "unterminated string"))?;
            out.push_str(&self.s[self.pos..self.pos + run]);
            self.pos += run;
            match self.b[self.pos] {
                b'"' => {
                    self.pos += 1;
                    return Ok(out);
                }
                b'\\' => {
                    let at = self.pos;
                    self.pos += 1;
                    let c = self
                        .peek()
                        .ok_or_else(|| self.error_at(open, "unterminated string"))?;
                    self.pos += 1;
                    match c {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => out.push(self.unicode_escape(at)?),
                        _ => return Err(self.error_at(at, "invalid escape")),
                    }
                }
                _ => return Err(self.error("control character in string")),
            }
        }
    }

    /// The code point of a `\uXXXX` escape (already past the `u`), combining a UTF-16 surrogate pair.
    fn unicode_escape(&mut self, at: usize) -> Result<char, JsonError> {
        let hi = self.hex4()?;
        let cp = match hi {
            0xd800..=0xdbff => {
                if !self.b[self.pos..].starts_with(b"\\u") {
                    return Err(self.error_at(at, "unpaired surrogate"));
                }
                self.pos += 2;
                let lo = self.hex4()?;
                if !(0xdc00..=0xdfff).contains(&lo) {
                    return Err(self.error_at(at, "unpaired surrogate"));
                }
                0x10000 + ((hi - 0xd800) << 10) + (lo - 0xdc00)
            }
            0xdc00..=0xdfff => return Err(self.error_at(at, "unpaired surrogate")),
            _ => hi,
        };
        char::from_u32(cp).ok_or_else(|| self.error_at(at, "invalid code point"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_documents() {
        assert!(parse("\u{feff}").is_err());
        let v =
            parse("\u{feff} {\"a\": [1, -2.5e3, true, false, null, \"x\"], \"b\": {}} ").unwrap();
        assert_eq!(v["a"][1].as_f64(), Some(-2500.0));
        assert_eq!(v["a"][5].as_str(), Some("x"));
        assert_eq!(v["b"], Value::object());
        assert_eq!(parse("0").unwrap(), Value::Number(0.0));
        assert_eq!(
            parse("-0").unwrap().as_f64().map(f64::is_sign_negative),
            Some(true)
        );
        assert_eq!(
            parse("\"\\ud83d\\ude00\\u00e9\\/\"").unwrap().as_str(),
            Some("😀é/")
        );
    }

    #[test]
    fn rejects_invalid() {
        for bad in [
            "",
            " ",
            "01",
            "1.",
            ".5",
            "-",
            "+1",
            "1e",
            "1e+",
            "0x10",
            "NaN",
            "Infinity",
            "1e400",
            "tru",
            "nul",
            "[1,]",
            "[1 2]",
            "{\"a\" 1}",
            "{\"a\":1,}",
            "{a:1}",
            "'x'",
            "\"abc",
            "\"a\nb\"",
            "\"\\x\"",
            "\"\\u12\"",
            "\"\\ud800\"",
            "\"\\ud800\\u0041\"",
            "\"\\udc00\"",
            "[",
            "{",
            "[1] 2",
            "{\"a\":}",
        ] {
            assert!(parse(bad).is_err(), "accepted {bad:?}");
        }
    }

    #[test]
    fn error_positions() {
        let e = parse("{\n  \"a\": 1,\n  \"é\": x\n}").unwrap_err();
        assert_eq!((e.line, e.column), (3, 8));
        assert!(e.to_string().contains("line 3, column 8"));
        let e = parse("[1, 2").unwrap_err();
        assert_eq!((e.line, e.column), (1, 6));
    }

    #[test]
    fn depth_limit() {
        let ok = "[".repeat(MAX_DEPTH) + &"]".repeat(MAX_DEPTH);
        assert!(parse(&ok).is_ok());
        let deep = "[".repeat(MAX_DEPTH + 1) + &"]".repeat(MAX_DEPTH + 1);
        assert!(parse(&deep).is_err());
        assert!(parse(&"{\"a\":".repeat(100_000)).is_err());
    }

    #[test]
    fn truncations_error() {
        let doc = r#"{"layers": [{"name": "base \u00e9", "opacity": 0.75, "mask": null, "on": true}], "size": [1024, 2048]}"#;
        assert!(parse(doc).is_ok());
        for n in 0..doc.len() {
            if doc.is_char_boundary(n) {
                assert!(parse(&doc[..n]).is_err(), "prefix {n}");
            }
        }
    }
}

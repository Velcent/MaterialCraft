//! [`Params`]: a generator's parameter values (name → text, as stored in documents) read as numbers, with
//! defaults for missing or unreadable values.

pub struct Params<'a> {
    values: &'a [(String, String)],
}

impl<'a> Params<'a> {
    pub fn new(values: &'a [(String, String)]) -> Params<'a> {
        Params { values }
    }

    fn raw(&self, name: &str) -> Option<&'a str> {
        self.values.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    /// A number (the first component of a list), or `default`.
    pub fn f(&self, name: &str, default: f32) -> f32 {
        self.raw(name).and_then(|v| v.split(',').next()).and_then(|v| v.trim().parse::<f32>().ok()).filter(|v| v.is_finite()).unwrap_or(default)
    }

    /// A number clamped to `lo..=hi`.
    pub fn range(&self, name: &str, default: f32, lo: f32, hi: f32) -> f32 {
        self.f(name, default).clamp(lo, hi)
    }

    /// A switch: any non-zero number is on.
    pub fn on(&self, name: &str, default: bool) -> bool {
        self.f(name, if default { 1.0 } else { 0.0 }) != 0.0
    }

    /// A whole number (rounded), or `default`.
    pub fn int(&self, name: &str, default: i32) -> i32 {
        let v = self.f(name, default as f32).round();
        if v.abs() < i32::MAX as f32 { v as i32 } else { default }
    }

    /// A seed: any whole number, wrapped into `u32`.
    pub fn seed(&self, name: &str) -> u32 {
        self.int(name, 0) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_switches_and_defaults() {
        let v = vec![("a".to_string(), "0.25,1".to_string()), ("b".to_string(), "nan".to_string()), ("c".to_string(), "-3".to_string())];
        let p = Params::new(&v);
        assert_eq!(p.f("a", 0.0), 0.25);
        assert_eq!(p.f("b", 0.5), 0.5);
        assert_eq!(p.f("missing", 0.7), 0.7);
        assert_eq!(p.range("c", 0.0, 0.0, 1.0), 0.0);
        assert!(p.on("a", false) && !p.on("missing", false));
        assert_eq!(p.int("c", 0), -3);
        assert_eq!(p.seed("c"), (-3i32) as u32);
    }
}

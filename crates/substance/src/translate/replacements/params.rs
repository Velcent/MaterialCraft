//! Reading Painter's parameter values (as the importer stores them: numbers comma-separated, and
//! `input:<identifier>` for what feeds an image input) and writing ours.

use crate::translate::layers::effect_data::INPUT_PREFIX;

/// An imported procedural's parameter values.
pub struct Painter<'a> {
    params: &'a [(String, String)],
}

impl<'a> Painter<'a> {
    pub fn new(params: &'a [(String, String)]) -> Painter<'a> {
        Painter { params }
    }

    fn raw(&self, name: &str) -> Option<&'a str> {
        self.params.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    /// All components of a number parameter.
    pub fn floats(&self, name: &str) -> Option<Vec<f32>> {
        let parsed: Result<Vec<f32>, _> = self.raw(name)?.split(',').map(|s| s.trim().parse::<f32>()).collect();
        parsed.ok().filter(|v| !v.is_empty() && v.iter().all(|x| x.is_finite()))
    }

    /// The first component of a number parameter.
    pub fn f(&self, name: &str) -> Option<f32> {
        self.floats(name)?.first().copied()
    }

    /// The first of `names` that has a value (parameters renamed between versions).
    pub fn first(&self, names: &[&str]) -> Option<f32> {
        names.iter().find_map(|n| self.f(n))
    }

    /// What feeds image input `input`, if anything does.
    pub fn input(&self, input: &str) -> Option<&'a str> {
        self.raw(&format!("{INPUT_PREFIX}{input}"))
    }
}

/// Our parameter list being built.
#[derive(Default)]
pub struct Ours {
    out: Vec<(String, String)>,
}

impl Ours {
    /// A number, if the Painter value was there.
    pub fn num(mut self, name: &str, v: Option<f32>) -> Ours {
        if let Some(v) = v.filter(|v| v.is_finite()) {
            self.out.push((name.to_string(), format_num(v)));
        }
        self
    }

    /// A text value (an id).
    pub fn text(mut self, name: &str, v: &str) -> Ours {
        self.out.push((name.to_string(), v.to_string()));
        self
    }

    /// Several numbers at once: `(ours, Painter's)` pairs.
    pub fn nums(self, p: &Painter, pairs: &[(&str, &str)]) -> Ours {
        pairs.iter().fold(self, |o, (ours, theirs)| o.num(ours, p.f(theirs)))
    }

    pub fn done(self) -> Vec<(String, String)> {
        self.out
    }
}

/// Shortest text that reads back as the same `f32`.
fn format_num(v: f32) -> String {
    let s = format!("{v}");
    if s == "-0" { "0".to_string() } else { s }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_components_and_writes_numbers() {
        let params = vec![
            ("a".to_string(), "0.25,1".to_string()),
            ("b".to_string(), "x".to_string()),
            ("input:texture".to_string(), "dirt_1".to_string()),
        ];
        let p = Painter::new(&params);
        assert_eq!(p.floats("a"), Some(vec![0.25, 1.0]));
        assert_eq!(p.f("a"), Some(0.25));
        assert_eq!(p.f("b"), None);
        assert_eq!(p.first(&["b", "a"]), Some(0.25));
        assert_eq!((p.input("texture"), p.input("other")), (Some("dirt_1"), None));
        let ours = Ours::default()
            .num("x", Some(0.5))
            .num("y", None)
            .num("z", Some(0.0))
            .nums(&p, &[("w", "a")])
            .done();
        assert_eq!(
            ours,
            vec![
                ("x".to_string(), "0.5".to_string()),
                ("z".to_string(), "0".to_string()),
                ("w".to_string(), "0.25".to_string())
            ]
        );
        assert_eq!(format_num(-0.0), "0");
    }
}

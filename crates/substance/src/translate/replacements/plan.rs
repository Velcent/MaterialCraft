//! [`ProceduralPlan`]: what the user chose per Substance procedural before an import: MaterialCraft's
//! replacement (editable, approximate) or Painter's own result (exact look, flattened). Read from the import
//! review or from a `choices.json`:
//!
//! ```json
//! {"default": "keep", "procedurals": {"mg_dirt": "replace", "mg_mask_editor": "keep"}}
//! ```
//!
//! `procedurals` may also be the `procedurals` array `mcsub analyze` prints, each entry with a `"choice"`.

use materialcraft::json::{self, Value};

/// One choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    /// Painter's own result: the procedural stays as it is (not computable; Painter's flattened result shows).
    Keep,
    /// MaterialCraft's replacement, where one exists.
    Replace,
}

impl Choice {
    pub fn id(self) -> &'static str {
        match self {
            Choice::Keep => "keep",
            Choice::Replace => "replace",
        }
    }

    pub fn from_id(id: &str) -> Option<Choice> {
        match id {
            "keep" => Some(Choice::Keep),
            "replace" => Some(Choice::Replace),
            _ => None,
        }
    }
}

/// Choices by procedural (graph) name, with a default for the rest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProceduralPlan {
    pub default: Choice,
    pub choices: Vec<(String, Choice)>,
}

impl Default for ProceduralPlan {
    /// Keep everything: the import is what it was before replacements existed.
    fn default() -> ProceduralPlan {
        ProceduralPlan::all(Choice::Keep)
    }
}

impl ProceduralPlan {
    /// The same choice for every procedural.
    pub fn all(choice: Choice) -> ProceduralPlan {
        ProceduralPlan {
            default: choice,
            choices: Vec::new(),
        }
    }

    /// The choice for graph `name`.
    pub fn choice(&self, name: &str) -> Choice {
        self.choices
            .iter()
            .find(|(n, _)| n == name)
            .map_or(self.default, |(_, c)| *c)
    }

    /// Builder: sets the choice for `name`.
    pub fn with(mut self, name: &str, choice: Choice) -> ProceduralPlan {
        self.choices.retain(|(n, _)| n != name);
        self.choices.push((name.to_string(), choice));
        self
    }

    /// A short stable text for cache keys (`keep;mg_dirt=replace`).
    pub fn key(&self) -> String {
        let mut parts: Vec<String> = self.choices.iter().map(|(n, c)| format!("{n}={}", c.id())).collect();
        parts.sort();
        parts.insert(0, self.default.id().to_string());
        parts.join(";")
    }

    pub fn parse(text: &str) -> Result<ProceduralPlan, String> {
        let v = json::parse(text).map_err(|e| format!("choices: {e}"))?;
        let mut plan = ProceduralPlan::default();
        if let Some(d) = v.get("default") {
            plan.default = choice_of(d, "default")?;
        }
        match v.get("procedurals") {
            Some(Value::Object(entries)) => {
                for (name, c) in entries {
                    plan = plan.with(name, choice_of(c, name)?);
                }
            }
            Some(Value::Array(entries)) => {
                for e in entries {
                    let name = e["name"].as_str().ok_or("choices: an entry without \"name\"")?;
                    if let Some(c) = e.get("choice") {
                        plan = plan.with(name, choice_of(c, name)?);
                    }
                }
            }
            Some(_) => return Err("choices: \"procedurals\" must be an object or an array".into()),
            None => {}
        }
        Ok(plan)
    }

    pub fn to_json(&self) -> Value {
        let choices = self
            .choices
            .iter()
            .map(|(n, c)| (n.clone(), Value::from(c.id())))
            .collect();
        Value::object()
            .with("default", self.default.id())
            .with("procedurals", Value::Object(choices))
    }
}

fn choice_of(v: &Value, what: &str) -> Result<Choice, String> {
    v.as_str()
        .and_then(Choice::from_id)
        .ok_or_else(|| format!("choices: {what}: expected \"keep\" or \"replace\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_objects_and_analyze_arrays() {
        let plan = ProceduralPlan::parse(r#"{"default": "replace", "procedurals": {"mg_dirt": "keep"}}"#).unwrap();
        assert_eq!(plan.choice("mg_dirt"), Choice::Keep);
        assert_eq!(plan.choice("mg_curvature"), Choice::Replace);
        assert_eq!(plan.key(), "replace;mg_dirt=keep");
        let again = ProceduralPlan::parse(&plan.to_json().to_string()).unwrap();
        assert_eq!(again, plan);
        let array = r#"{"procedurals": [{"name": "mg_dirt", "choice": "replace"}, {"name": "blur"}]}"#;
        let plan = ProceduralPlan::parse(array).unwrap();
        assert_eq!(
            (plan.choice("mg_dirt"), plan.choice("blur")),
            (Choice::Replace, Choice::Keep)
        );
        assert!(ProceduralPlan::parse(r#"{"default": "maybe"}"#).is_err());
        assert!(ProceduralPlan::parse("[").is_err());
    }
}

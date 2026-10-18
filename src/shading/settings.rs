//! [`ShaderSettings`]: a document's shader instances, which texture set uses which (VS-03, VS-12), and the mesh's
//! length unit the displacement units convert through (VS-08).

use super::instance::ShaderInstance;
use crate::mesh::LengthUnit;

/// A texture set's instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assignment {
    /// Texture set id.
    pub set: String,
    /// Shader instance id.
    pub instance: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ShaderSettings {
    pub instances: Vec<ShaderInstance>,
    /// At most one per set, sorted by set id. A set without one (or whose instance is gone) uses the built-in default shader with
    /// its defaults, which is how documents from before shader instances look.
    pub assignments: Vec<Assignment>,
    /// The mesh's length unit; `None` when unknown.
    pub mesh_unit: Option<LengthUnit>,
}

impl ShaderSettings {
    /// True when the settings say nothing (old documents): nothing is written for them.
    pub fn is_empty(&self) -> bool {
        self.instances.is_empty() && self.assignments.is_empty() && self.mesh_unit.is_none()
    }

    pub fn instance(&self, id: &str) -> Option<&ShaderInstance> {
        self.instances.iter().find(|i| i.id == id)
    }

    pub fn instance_mut(&mut self, id: &str) -> Option<&mut ShaderInstance> {
        self.instances.iter_mut().find(|i| i.id == id)
    }

    /// The id of the instance assigned to texture set `set` (even if that instance doesn't exist).
    pub fn assigned(&self, set: &str) -> Option<&str> {
        self.assignments.iter().find(|a| a.set == set).map(|a| a.instance.as_str())
    }

    /// The instance texture set `set` is shaded with; `None` for the built-in defaults.
    pub fn instance_for(&self, set: &str) -> Option<&ShaderInstance> {
        self.assigned(set).and_then(|id| self.instance(id))
    }

    /// The texture sets (ids) using instance `id`.
    pub fn sets_using(&self, id: &str) -> Vec<&str> {
        self.assignments.iter().filter(|a| a.instance == id).map(|a| a.set.as_str()).collect()
    }

    /// Assigns `instance` to `set` (`None`: back to the built-in defaults); returns the previous assignment.
    pub fn assign(&mut self, set: &str, instance: Option<String>) -> Option<String> {
        let at = self.assignments.iter().position(|a| a.set == set);
        match (at, instance) {
            (Some(i), Some(id)) => self.assignments.get_mut(i).map(|a| std::mem::replace(&mut a.instance, id)),
            (Some(i), None) => Some(self.assignments.remove(i).instance),
            (None, Some(id)) => {
                let at = self.assignments.partition_point(|a| a.set.as_str() < set);
                self.assignments.insert(at, Assignment { set: set.to_string(), instance: id });
                None
            }
            (None, None) => None,
        }
    }

    /// An instance id no instance has yet: `shader`, `shader_2`, …
    pub fn new_instance_id(&self) -> String {
        (1..).map(|n: usize| if n == 1 { "shader".to_string() } else { format!("shader_{n}") }).find(|id| self.instance(id).is_none()).unwrap_or_default()
    }
}

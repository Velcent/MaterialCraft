//! [`ShaderEdit`]: reversible changes to [`ShaderSettings`], recorded on the shader settings' own undo history
//! (VS-06; `crate::history::ShaderHistory`), never on the project timeline (ADR 0008 q-undo-stacks).

use super::displacement::{Displacement, Tessellation};
use super::instance::ShaderInstance;
use super::param::ParamValue;
use super::settings::{Assignment, ShaderSettings};
use crate::mesh::LengthUnit;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum ShaderEdit {
    /// Sets a parameter (`None`: back to its default).
    SetValue {
        instance: String,
        param: String,
        value: Option<ParamValue>,
    },
    SetDisplacement {
        instance: String,
        value: Displacement,
    },
    SetTessellation {
        instance: String,
        value: Tessellation,
    },
    Rename {
        instance: String,
        name: String,
    },
    /// Replaces an instance's whole content, keeping its id (restore defaults, another shader file, update).
    Replace {
        instance: Box<ShaderInstance>,
    },
    /// Inserts an instance at `at` (clamped) and assigns it to `sets`.
    Insert {
        at: usize,
        instance: Box<ShaderInstance>,
        sets: Vec<String>,
    },
    /// Removes an instance; the sets using it go back to the built-in defaults, then `restore` is assigned (the
    /// assignments an insert replaced; empty for a plain delete).
    Remove {
        instance: String,
        restore: Vec<Assignment>,
    },
    /// Gives texture set `set` an instance (`None`: the built-in defaults).
    Assign {
        set: String,
        instance: Option<String>,
    },
    SetMeshUnit(Option<LengthUnit>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShaderEditError {
    UnknownInstance(String),
    DuplicateId(String),
}

impl fmt::Display for ShaderEditError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ShaderEditError::UnknownInstance(id) => write!(f, "there is no shader instance {id:?}"),
            ShaderEditError::DuplicateId(id) => write!(f, "a shader instance {id:?} exists already"),
        }
    }
}

impl std::error::Error for ShaderEditError {}

fn instance_mut<'a>(s: &'a mut ShaderSettings, id: &str) -> Result<&'a mut ShaderInstance, ShaderEditError> {
    s.instance_mut(id).ok_or_else(|| ShaderEditError::UnknownInstance(id.to_string()))
}

impl ShaderEdit {
    /// Applies the edit and returns its inverse. On error `s` is unchanged.
    pub fn apply(self, s: &mut ShaderSettings) -> Result<ShaderEdit, ShaderEditError> {
        Ok(match self {
            ShaderEdit::SetValue { instance, param, value } => {
                let old = instance_mut(s, &instance)?.set_value(&param, value);
                ShaderEdit::SetValue { instance, param, value: old }
            }
            ShaderEdit::SetDisplacement { instance, value } => {
                let old = std::mem::replace(&mut instance_mut(s, &instance)?.displacement, value);
                ShaderEdit::SetDisplacement { instance, value: old }
            }
            ShaderEdit::SetTessellation { instance, value } => {
                let old = std::mem::replace(&mut instance_mut(s, &instance)?.tessellation, value);
                ShaderEdit::SetTessellation { instance, value: old }
            }
            ShaderEdit::Rename { instance, name } => {
                let old = std::mem::replace(&mut instance_mut(s, &instance)?.name, name);
                ShaderEdit::Rename { instance, name: old }
            }
            ShaderEdit::Replace { instance: new } => {
                let slot = instance_mut(s, &new.id)?;
                let old = std::mem::replace(slot, *new);
                ShaderEdit::Replace { instance: Box::new(old) }
            }
            ShaderEdit::Insert { at, instance, sets } => {
                if s.instance(&instance.id).is_some() {
                    return Err(ShaderEditError::DuplicateId(instance.id));
                }
                let id = instance.id.clone();
                let at = at.min(s.instances.len());
                s.instances.insert(at, *instance);
                let mut restore = Vec::new();
                for set in &sets {
                    if let Some(old) = s.assign(set, Some(id.clone())) {
                        restore.push(Assignment { set: set.clone(), instance: old });
                    }
                }
                ShaderEdit::Remove { instance: id, restore }
            }
            ShaderEdit::Remove { instance, restore } => {
                let at = s.instances.iter().position(|i| i.id == instance).ok_or_else(|| ShaderEditError::UnknownInstance(instance.clone()))?;
                if let Some(a) = restore.iter().find(|a| a.instance == instance || s.instance(&a.instance).is_none()) {
                    return Err(ShaderEditError::UnknownInstance(a.instance.clone()));
                }
                let sets: Vec<String> = s.sets_using(&instance).into_iter().map(str::to_string).collect();
                s.assignments.retain(|a| a.instance != instance);
                let removed = s.instances.remove(at);
                for a in restore {
                    s.assign(&a.set, Some(a.instance));
                }
                ShaderEdit::Insert { at, instance: Box::new(removed), sets }
            }
            ShaderEdit::Assign { set, instance } => {
                if let Some(id) = &instance
                    && s.instance(id).is_none()
                {
                    return Err(ShaderEditError::UnknownInstance(id.clone()));
                }
                let old = s.assign(&set, instance);
                ShaderEdit::Assign { set, instance: old }
            }
            ShaderEdit::SetMeshUnit(unit) => ShaderEdit::SetMeshUnit(std::mem::replace(&mut s.mesh_unit, unit)),
        })
    }

    /// Folds `later`, an inverse recorded right after `self` in one coalesced step (a slider drag), into `self`:
    /// two inverses of the same field keep the older value. Returns `later` if they concern different things.
    pub fn absorb(&mut self, later: ShaderEdit) -> Option<ShaderEdit> {
        let same = match (&*self, &later) {
            (ShaderEdit::SetValue { instance: a, param: p, .. }, ShaderEdit::SetValue { instance: b, param: q, .. }) => a == b && p == q,
            (ShaderEdit::SetDisplacement { instance: a, .. }, ShaderEdit::SetDisplacement { instance: b, .. })
            | (ShaderEdit::SetTessellation { instance: a, .. }, ShaderEdit::SetTessellation { instance: b, .. })
            | (ShaderEdit::Rename { instance: a, .. }, ShaderEdit::Rename { instance: b, .. }) => a == b,
            (ShaderEdit::SetMeshUnit(_), ShaderEdit::SetMeshUnit(_)) => true,
            _ => false,
        };
        if same { None } else { Some(later) }
    }
}

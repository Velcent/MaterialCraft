//! [`SettingKey`]: a bake setting of one texture set as a log entry's target ([`crate::diag::Target::BakeSetting`]),
//! written `<section>.<field>@<set id>`: `common.max_dist@Body`, `ambient_occlusion.rays@Body`. Sections are
//! `common` and the map names ([`MapKind::name`]); set ids may contain anything, sections and fields never `@`.

use crate::bake::MapKind;
use crate::diag::Target;
use std::fmt;

/// See the module docs.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SettingKey {
    pub set: String,
    /// `<section>.<field>`.
    pub field: String,
}

impl SettingKey {
    pub fn new(set: impl Into<String>, field: impl Into<String>) -> SettingKey {
        SettingKey { set: set.into(), field: field.into() }
    }

    /// A key the baker reports (`max_dist`, or already `<section>.<field>`) for texture set `set`; keys without a
    /// section are common settings.
    pub fn from_baker(set: &str, key: &str) -> SettingKey {
        let field = if key.contains('.') { key.to_string() } else { format!("common.{key}") };
        SettingKey::new(set, field)
    }

    /// Parses `<section>.<field>@<set id>`.
    pub fn parse(text: &str) -> Option<SettingKey> {
        let (field, set) = text.split_once('@')?;
        (field.contains('.') && !set.is_empty()).then(|| SettingKey::new(set, field))
    }

    /// The map the setting belongs to; `None` for common settings.
    pub fn map(&self) -> Option<MapKind> {
        MapKind::from_name(self.field.split('.').next()?)
    }

    pub fn target(&self) -> Target {
        Target::BakeSetting(self.to_string())
    }
}

impl fmt::Display for SettingKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.field, self.set)
    }
}

//! [`SetSink`]: the baker's log entries for one texture set: bake-setting targets become keys of that set (so the
//! log can reveal them in the bakers panel) and the text names the set.

use materialcraft::diag::{Entry, Sink, Target};
use materialcraft::paint::bake_config::SettingKey;
use std::sync::Arc;

pub struct SetSink {
    pub inner: Arc<dyn Sink>,
    pub set: String,
    pub name: String,
}

impl Sink for SetSink {
    fn emit(&self, mut entry: Entry) {
        if let Some(Target::BakeSetting(key)) = &entry.target {
            entry.target = Some(SettingKey::from_baker(&self.set, key).target());
        }
        entry.text = format!("{}: {}", self.name, entry.text);
        self.inner.emit(entry);
    }
}

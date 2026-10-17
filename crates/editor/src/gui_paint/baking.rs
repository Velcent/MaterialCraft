//! Baking on the Paint screen: the bakers panel's requests, the running [`BakeQueue`] and auto-rebakes.
//!
//! Finished maps go into the set's baked maps as one history step each ("Bake <set>") and the set is
//! recomposited, so every layer that reads its baked maps updates (BK2-12). Auto-rebake requests wait
//! [`AUTO_DELAY`] after the last change (a slider drag asks many times) and run once no bake is running.

use super::PaintMode;
use super::bake_editor;
use super::bake_queue::{BakeEvent, BakeJob, BakeQueue};
use super::channel_view::ChannelView;
use super::panels::bakers::BakerEvent;
use materialcraft::bake::MapSelection;
use materialcraft::diag::{Area, SinkExt};
use materialcraft::paint::bake_config::{ops, to_baked};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// How long auto-rebake waits after the last settings change.
pub const AUTO_DELAY: Duration = Duration::from_millis(700);

/// The bake state of the screen.
#[derive(Default)]
pub struct Baking {
    /// The running queue and the document it bakes for.
    queue: Option<(BakeQueue, PathBuf)>,
    /// Maps waiting to be rebaked, per set id, and when the last request came.
    auto: BTreeMap<String, MapSelection>,
    auto_since: Option<Instant>,
    /// Sets of the running queue that failed, with the reason (said once the queue ends).
    failed: Vec<(String, String)>,
}

impl Baking {
    /// (set name, done, total) of the running bake.
    pub fn progress(&self) -> Option<(&str, usize, usize)> {
        self.queue.as_ref().and_then(|(q, _)| q.progress())
    }

    pub fn is_busy(&self) -> bool {
        self.queue.is_some() || !self.auto.is_empty()
    }

    /// Stops everything (the document is closing).
    pub fn stop(&mut self) {
        *self = Baking::default();
    }

    fn request_auto(&mut self, maps: BTreeMap<String, MapSelection>) {
        if maps.is_empty() {
            return;
        }
        for (set, m) in maps {
            let entry = self.auto.entry(set).or_insert_with(MapSelection::none);
            *entry = m.kinds().fold(*entry, MapSelection::with);
        }
        self.auto_since = Some(Instant::now());
    }
}

impl PaintMode {
    /// A request from the bakers panel.
    pub(super) fn bakers_event(&mut self, ev: BakerEvent) {
        match ev {
            BakerEvent::Bake { sets, maps } => self.start_bake(&sets, maps),
            BakerEvent::Cancel => {
                if let Some((q, _)) = &self.baking.queue {
                    q.cancel();
                }
                self.baking.auto.clear();
            }
            ev => {
                let Some(doc) = &mut self.document else { return };
                match bake_editor::apply(doc, ev) {
                    Ok(applied) => {
                        if let Some(show) = applied.show {
                            self.show_channel(show.map_or(ChannelView::Material, ChannelView::Baked));
                        }
                        self.baking.request_auto(applied.rebake);
                    }
                    Err(e) => self.diag.warn(Area::Bake, format!("That bake setting was not changed: {e}.")),
                }
            }
        }
    }

    fn start_bake(&mut self, sets: &[String], maps: Option<MapSelection>) {
        let Some(doc) = &self.document else { return };
        if self.baking.queue.is_some() {
            self.diag.info(Area::Bake, "A bake is running; it has to finish or be cancelled first.");
            return;
        }
        let jobs = BakeJob::for_sets(doc.doc(), sets, maps);
        if jobs.is_empty() {
            self.diag.info(Area::Bake, "Nothing to bake: no map is checked.");
            return;
        }
        self.diag.info(Area::Bake, format!("Baking {} texture set(s).", jobs.len()));
        let dir = doc.source().parent().map(PathBuf::from);
        let queue = BakeQueue::start(doc.bake_mesh(), dir, jobs, self.diag.clone());
        self.baking.queue = Some((queue, doc.source().to_path_buf()));
    }

    /// Takes finished bakes into the document and starts due auto-rebakes. Call once per frame.
    pub(super) fn poll_bake(&mut self) {
        let source = self.document.as_ref().map(|d| d.source().to_path_buf());
        if let Some((queue, for_source)) = &mut self.baking.queue {
            let events = queue.poll();
            let same = source.as_ref() == Some(for_source);
            let finished = queue.is_finished();
            for ev in events {
                match ev {
                    BakeEvent::Baked { set, maps } if same => self.store(&set, &maps),
                    BakeEvent::Failed { set, error } => self.baking.failed.push((set, error)),
                    BakeEvent::Finished { cancelled } => self.bake_finished(cancelled),
                    _ => {}
                }
            }
            if finished || !same {
                self.baking.queue = None;
            }
        }
        let due = self.baking.auto_since.is_some_and(|t| t.elapsed() >= AUTO_DELAY);
        if due && self.baking.queue.is_none() {
            // One set per queue keeps each set's map selection; the next starts when this one is done.
            if let Some((set, maps)) = self.baking.auto.pop_first() {
                self.start_bake(&[set], Some(maps));
            }
            if self.baking.auto.is_empty() {
                self.baking.auto_since = None;
            }
        }
    }

    fn bake_finished(&mut self, cancelled: bool) {
        let failed = std::mem::take(&mut self.baking.failed);
        let doc = self.document.as_ref().map(|d| d.doc());
        let name = |id: &str| doc.and_then(|d| d.texture_set(id)).map_or_else(|| id.to_string(), |s| s.name.clone());
        let what = if cancelled { "Bake cancelled" } else { "Bake finished" };
        if failed.is_empty() {
            self.diag.info(Area::Bake, format!("{what}."));
        } else {
            let list: Vec<String> = failed.iter().map(|(s, e)| format!("{} ({e})", name(s))).collect();
            self.diag.warn(Area::Bake, format!("{what}; not baked: {}.", list.join(", ")));
        }
    }

    /// Puts a bake's maps into the set (one history step) and recomposites it.
    fn store(&mut self, set: &str, maps: &materialcraft::bake::BakedMaps) {
        let Some(doc) = &mut self.document else { return };
        let name = doc.doc().texture_set(set).map_or_else(|| set.to_string(), |s| s.name.clone());
        let result = to_baked(maps)
            .map_err(|e| e.to_string())
            .and_then(|baked| doc.edit(&format!("Bake {name}"), None, |tx| ops::store_baked(tx, set, baked)).map_err(|e| e.to_string()));
        match (result, doc.set_index(set)) {
            (Ok(()), Some(i)) => self.scene.refresh(doc, self.display, vec![i]),
            (Ok(()), None) => {}
            (Err(e), _) => self.diag.error(Area::Bake, format!("{name}: the baked maps could not be stored: {e}.")),
        }
    }
}

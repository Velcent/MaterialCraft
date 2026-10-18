//! [`MapsQueue`]: the order maps runs happen in. One [`MapsWorker`] runs at a time; what is asked for meanwhile is
//! merged into the next run. A request for whole sets cancels a running run with whole sets (a slider drag makes
//! its earlier results outdated before they arrive); runs of tiles only (a brush stroke) are short and always
//! finish, so their patches reach the GPU in the order they were painted.

use super::channel_view::{ChannelView, Shown};
use super::maps_worker::{MapsWorker, SetMaps, Want};
use std::collections::BTreeMap;

/// Jobs to start a worker for, with the display mode they are for.
pub type Start = (ChannelView, Vec<(usize, Want)>);

struct Running {
    worker: MapsWorker,
    shown: Shown,
    /// Its results are outdated and dropped.
    cancelled: bool,
}

#[derive(Default)]
pub struct MapsQueue {
    running: Option<Running>,
    pending: Option<(ChannelView, BTreeMap<usize, Want>)>,
}

impl MapsQueue {
    /// Asks for `wants` in display mode `view`; returns the jobs to start now when no worker runs (the caller starts
    /// one and hands it to [`Self::started`]), else merges them into the next run.
    pub fn ask(&mut self, view: ChannelView, wants: Vec<(usize, Want)>) -> Option<Start> {
        let Some(r) = self.running.as_mut().filter(|r| !r.worker.is_finished()) else {
            self.queue(view, wants);
            return self.pending.take().map(|(v, p)| (v, p.into_iter().collect()));
        };
        let whole = |w: &[(usize, Want)]| w.iter().any(|(_, w)| *w == Want::Full);
        if !r.cancelled && whole(&wants) && whole(r.worker.jobs()) {
            r.worker.cancel();
            r.cancelled = true;
            let again = (r.shown.view, r.worker.jobs().to_vec());
            self.queue(again.0, again.1);
        }
        self.queue(view, wants);
        None
    }

    /// Merges into the next run; a new display mode makes every queued tile want a whole set.
    fn queue(&mut self, view: ChannelView, wants: Vec<(usize, Want)>) {
        let (v, pending) = self.pending.get_or_insert_with(|| (view, BTreeMap::new()));
        if *v != view {
            pending.values_mut().for_each(|w| *w = Want::Full);
            *v = view;
        }
        merge(pending, wants);
    }

    /// The worker started for what [`Self::ask`] or [`Self::poll`] returned.
    pub fn started(&mut self, worker: MapsWorker, shown: Shown) {
        self.running = Some(Running { worker, shown, cancelled: false });
    }

    /// The results that arrived (with the display mode they were made for; cancelled runs' are dropped), and the jobs
    /// to start next when the running worker has ended and more was asked for.
    pub fn poll(&mut self) -> (Vec<(SetMaps, Shown)>, Option<Start>) {
        let Some(r) = &mut self.running else { return (Vec::new(), None) };
        let arrived = r.worker.poll();
        let out = if r.cancelled { Vec::new() } else { arrived.into_iter().map(|m| (m, r.shown)).collect() };
        if !r.worker.is_finished() {
            return (out, None);
        }
        self.running = None;
        let next = self.pending.take().map(|(view, p)| (view, p.into_iter().collect()));
        (out, next)
    }

    /// True if the whole of `set` in `shown` is on its way (running uncancelled, or queued): tiles asked for meanwhile
    /// can be patched onto it.
    pub fn brings_whole(&self, set: usize, shown: &Shown) -> bool {
        let running = self.running.as_ref().is_some_and(|r| !r.cancelled && r.shown == *shown && r.worker.jobs().contains(&(set, Want::Full)));
        let queued = self.pending.as_ref().is_some_and(|(v, p)| *v == shown.view && p.get(&set) == Some(&Want::Full));
        running || queued
    }

    pub fn is_busy(&self) -> bool {
        self.running.is_some() || self.pending.is_some()
    }

    /// (sets done, sets total) of the running worker.
    pub fn progress(&self) -> Option<(usize, usize)> {
        self.running.as_ref().map(|r| r.worker.progress())
    }
}

fn merge(into: &mut BTreeMap<usize, Want>, wants: Vec<(usize, Want)>) {
    for (set, want) in wants {
        let merged = match into.remove(&set) {
            Some(old) => old.merged(want),
            None => want,
        };
        into.insert(set, merged);
    }
}

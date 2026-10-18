//! [`SceneStats`]: what keeping the GPU scene in step has cost so far: sets composited, the worker's time, the bytes
//! handed to the GPU. Read by the performance benchmarks (`perf`); cheap enough to keep always.

use super::maps_worker::SetTiming;
use materialcraft::gpu3d::GpuMaps;
use std::time::Duration;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SceneStats {
    /// Set results handed to the GPU.
    pub sets: usize,
    /// The maps worker's compositing time, summed.
    pub composite: Duration,
    /// The maps worker's GPU map preparation time, summed.
    pub prepare: Duration,
    /// Bytes queued for upload (every part's copy counts).
    pub upload_bytes: usize,
    /// Time spent on the UI thread handing maps to the GPU queue.
    pub handoff: Duration,
}

impl SceneStats {
    /// Counts one set's arrival.
    pub fn arrived(&mut self, timing: SetTiming) {
        self.sets += 1;
        self.composite += timing.composite;
        self.prepare += timing.prepare;
    }

    /// Counts `copies` uploads of `bytes` each, handed over in `took`.
    pub fn handed(&mut self, bytes: usize, copies: usize, took: Duration) {
        self.upload_bytes += bytes * copies;
        self.handoff += took;
    }
}

/// The bytes of one set of GPU maps.
pub fn bytes_of(m: &GpuMaps) -> usize {
    m.base.len() + m.normal.len() + m.rma.len() + m.emissive.as_ref().map_or(0, Vec::len) + m.height.as_ref().map_or(0, |h| h.2.len() * 4)
}

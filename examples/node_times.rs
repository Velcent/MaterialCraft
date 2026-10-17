// SPDX-License-Identifier: MIT OR Apache-2.0
use materialcraft::graph::{Graph, SPECS, demo_graph_for};
use std::time::Instant;
fn main() {
    let size: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(2048);
    let mut v = Vec::new();
    for sp in SPECS {
        let Some(t) = demo_graph_for(sp.kind) else { continue };
        let g = Graph::parse(&t).unwrap();
        let t0 = Instant::now();
        let _ = g.evaluate(Some(size));
        v.push((t0.elapsed().as_secs_f64(), sp.kind));
    }
    v.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    for (t, k) in v.iter().take(25) {
        println!("{t:7.2}s  {k}");
    }
}

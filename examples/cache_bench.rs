//! Editor-style benchmark: first evaluation, then small edits, with and without the node cache.
//! usage: cache_bench graph.mcg size node param v1 v2   (the edit alternates the parameter between v1 and v2)
use materialcraft::graph::{Graph, NodeCache};
use std::sync::Arc;
use std::time::Instant;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut g = Graph::parse(&std::fs::read_to_string(&a[1]).unwrap()).unwrap();
    let size: usize = a[2].parse().unwrap();
    let (node, param): (u32, String) = (a[3].parse().unwrap(), a[4].clone());
    let (v1, v2): (f32, f32) = (a[5].parse().unwrap(), a[6].parse().unwrap());
    let keep: Vec<u32> = g.output_nodes().iter().map(|o| o.2).collect();
    let cache = Arc::new(NodeCache::with_default_budget());
    let t = Instant::now();
    g.evaluate_all_cached(size, &keep, 0, &cache, None, &mut |_| {});
    println!("first evaluation (cold cache): {:.0} ms", t.elapsed().as_secs_f64() * 1000.0);
    for i in 0..4 {
        let v = if i % 2 == 0 { v1 } else { v2 } + 0.01 * (i + 1) as f32;
        g.nodes.get_mut(&node).unwrap().nums.insert(param.clone(), v);
        let t = Instant::now();
        let mut first_ms = 0.0;
        g.evaluate_all_cached(size, &keep, 0, &cache, Some(keep[0]), &mut |_| first_ms = t.elapsed().as_secs_f64() * 1000.0);
        let cached = t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        g.evaluate_all(size, &keep);
        let fresh = t.elapsed().as_secs_f64() * 1000.0;
        println!("edit {i}: cached {cached:.0} ms (selected node shown after {first_ms:.0} ms) vs no cache {fresh:.0} ms");
    }
    println!("cache holds {} nodes, {} MB", cache.len(), cache.bytes() >> 20);
}

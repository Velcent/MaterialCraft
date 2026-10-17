// SPDX-License-Identifier: MIT OR Apache-2.0
use materialcraft::graph::Graph;
use std::time::Instant;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let text = std::fs::read_to_string(&a[1]).unwrap();
    let g = Graph::parse(&text).unwrap();
    for s in a[2..].iter().map(|s| s.parse::<usize>().unwrap()) {
        let t = Instant::now();
        let r = g.evaluate(Some(s));
        println!("{s}: {:?} ok={}", t.elapsed(), r.is_ok());
    }
}

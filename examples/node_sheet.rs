//! Dev helper: renders every node type's demo graph to out_dir/<kind>.png (for a visual contact sheet).
use materialcraft::graph::{Graph, SPECS, demo_graph_for};
fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "node_sheet".into());
    std::fs::create_dir_all(&dir).unwrap();
    for sp in SPECS {
        let Some(text) = demo_graph_for(sp.kind) else { continue };
        let text = text.replacen("size 64", "size 128", 1);
        let g = Graph::parse(&text).unwrap();
        let out = g.evaluate(Some(128)).unwrap();
        let t = &out["o"];
        std::fs::write(format!("{dir}/{}.png", sp.kind), materialcraft::png::encode(t.w, t.h, &t.px, 8)).unwrap();
    }
}

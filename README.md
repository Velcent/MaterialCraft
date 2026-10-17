
### Donate For Claude Max Subscription to support development (all funds go to claude max subscription)
Donate Here: [Donation Button](https://www.paypal.com/donate/?hosted_button_id=DYD94FKUZ9H7G).

our discord is here: if you want to submit features or got any cool ideas! https://discord.gg/9KVCNatNvX

# MaterialCraft

<img width="1495" height="949" alt="image" src="https://github.com/user-attachments/assets/dc27324c-f63c-4569-beb7-be4766bd839c" />

<img width="2556" height="1388" alt="image" src="https://github.com/user-attachments/assets/75dba5c4-50ac-4f79-8395-8d3c20697d63" />

Procedural, node-based material authoring in pure Rust (Windows / Linux / macOS).

## Build & run
    cargo run --release --features gui --bin materialcraft-gui     # visual editor
    cargo run --release -- render examples/brick.mcg out --size 1024 --render3d   # CLI

Requires a current stable Rust toolchain (https://rustup.rs). On Windows the MSVC build tools are needed.
The core library has zero dependencies; the editor uses egui/eframe + rfd (native file dialogs).

## Editor
Start screen (recent files, New graph dialog with templates) -> editor with Explorer, Library (thumbnail grid), Graph,
3D View, 2D View and Properties panels. Panels can be closed/reopened from the Windows menu.

Tab = add node at cursor · drag a link into empty space = add + auto-connect · Ctrl+Z/Y undo/redo ·
Ctrl+C/X/V/D · rubber-band & Shift/Ctrl multi-select · C = comment frame · double-click a subgraph to edit it ·
F = frame all · Alt/Space/Middle-drag = pan · wheel = zoom.

## CLI
    materialcraft render <graph.mcg|.mca> <outdir> [--sizes 512,1024] [--depth 16] [--param "Label=0.5"] [--variations 8] [--render3d]
    materialcraft publish <graph.mcg> <out.mca>      # embeds bitmaps, flattens editor data
    materialcraft bake <low.obj> <high.obj> <outdir> [--size 1024]
    materialcraft nodes | check | params | testmesh | example

## Platforms
Windows, Linux and macOS use the same code: `eframe` (glow / OpenGL 3.3 core) for the window and the 3D viewport, `rfd` for
file dialogs, and only the Rust standard library elsewhere (no platform-specific code besides where the settings folder is:
`%APPDATA%`, `~/.config` or `~/Library/Application Support`). GitHub Actions (`.github/workflows/ci.yml`) builds and tests all three.
Linux needs the usual X11/Wayland/GL development packages (`libxkbcommon-dev libwayland-dev libx11-dev libgl1-mesa-dev`).

## Not implemented (honest list)
FX-Maps, splines/paths, function graphs, SVG/font/AxF import, OCIO/ACE colour management, scripting API,
EXR / HDR output, path-traced 3D rendering, custom mesh import in the 3D viewer, third-party file-format compatibility.
See ROADMAP.md for the complete list.


## Nodes (124)
Generators: shape, tile sampler, tile generator, splatter, splatter circular, perlin, fractal noise, clouds, gaussian noise,
BnW spots, crystal, directional noise, grunge, grunge maps, plasma, voronoi, voronoi fractal, stripes, weave, weave generator, bricks, checker, gradients, star, SDF shapes.
Filters: blend, RGBA merge, switch, flood fill (+ random grayscale / color / gradient), transform 2D, directional / vector /
slope warps, blur and blur HQ, levels, histogram scan / range, bevel, distance, gradient map, uniform color, HSL, color match,
make-it-tile photo / patch and more. Material: normal, ambient occlusion, curvature (smooth), height blend, edge wear, dirt, dust.
Graph items: output (drives the 3D view), input color / grayscale (subgraphs), subgraph instances, bitmap, comment, frame, dot.


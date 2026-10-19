
### Donate For Claude Max Subscription to support development (all funds go to claude max subscription)
Donate Here: [Donation Button](https://www.paypal.com/donate/?hosted_button_id=DYD94FKUZ9H7G).

our discord is here: if you want to submit features or got any cool ideas! https://discord.gg/9KVCNatNvX

# MaterialCraft

<img width="1495" height="949" alt="image" src="https://github.com/user-attachments/assets/dc27324c-f63c-4569-beb7-be4766bd839c" />

<img width="2556" height="1388" alt="image" src="https://github.com/user-attachments/assets/75dba5c4-50ac-4f79-8395-8d3c20697d63" />

Material authoring and texture painting in pure Rust (Windows / Linux / macOS). MaterialCraft has two halves that share one
engine, one asset library and one file of settings:

| | **Material graph** | **Paint mode** |
|---|---|---|
| What you make | Procedural, tileable materials | Textures for a specific mesh |
| How | Node graphs (texture, function and pattern graphs) | Layer stacks with fills, masks, effects and brush strokes, painted on the model |
| Works on | Tileable 2D textures, previewed on test shapes | Your mesh's texture sets (UV space), with baked mesh maps |
| Files | `.mcg` graphs, `.mca` published materials | `.mcpaint` documents, `.mcfrag` smart materials / smart masks |
| Comparable to | node-based material designers | layer-based 3D texture painters |

The two meet in the **asset tray**: a material made in the graph editor is dropped onto a mesh in Paint mode as a fill
layer; grunge and noise looks from the graph side drive Paint mode's mask generators.

## Build & run
    cargo run --release -p materialcraft-editor                     # the editor (binary materialcraft-gui): start screen with both modes
    cargo run --release -p materialcraft-editor -- --paint <file>   # open a .mcpaint document (or import a .spp) straight into Paint mode
    cargo run --release -p materialcraft-editor -- --paint-demo     # Paint mode with a built-in demo model
    cargo run --release -- render examples/brick.mcg out --size 1024 --render3d   # CLI
    cargo run --release --features gpu -- render examples/brick.mcg out --size 2048   # CLI with GPU compute (falls back to the CPU automatically)

Requires a current stable Rust toolchain (https://rustup.rs), Rust 1.95 or newer. On Windows the MSVC build tools are needed.
The core library has zero dependencies; the editor uses egui/eframe + rfd (native file dialogs).

## Material graph (the node editor)
Start screen (recent files, New graph dialog with templates) -> editor with Explorer, Library (thumbnail grid), Graph,
3D View, 2D View and Properties panels. Panels can be closed/reopened from the Windows menu.

Tab = add node at cursor · drag a link into empty space = add + auto-connect · Ctrl+Z/Y undo/redo ·
Ctrl+C/X/V/D · rubber-band & Shift/Ctrl multi-select · C = comment frame · double-click a subgraph to edit it ·
F = frame all · Alt/Space/Middle-drag = pan · wheel = zoom.

### Graph language
Besides texture graphs, a package can hold **function graphs** (`meta kind "function"`, about 85 value nodes: math, vectors,
logic, variables, random, image sampling, calls) and **pattern programs** (`meta kind "fxmap"`: quadrant, iterate, switch and
marker nodes, run by the Pattern Program Map node). Any parameter can be driven by a function, and the pixel processor and
value processor run function graphs. Also: curve node, SVG and text nodes (own rasteriser and TrueType reader).

### Nodes (136 texture nodes, 84 function nodes, 5 pattern-program nodes)
Generators: shape, tile sampler, tile generator, splatter, splatter circular, perlin, fractal noise, clouds, gaussian noise,
BnW spots, crystal, directional noise, grunge, grunge maps, plasma, voronoi, voronoi fractal, stripes, weave, weave generator, bricks, checker, gradients, star, SDF shapes.
Filters: blend, RGBA merge, switch, flood fill (+ random grayscale / color / gradient), transform 2D, directional / vector /
slope warps, blur and blur HQ, levels, histogram scan / range, bevel, distance, gradient map, uniform color, HSL, color match,
make-it-tile photo / patch and more. Material: normal, ambient occlusion, curvature (smooth), height blend, edge wear, dirt, dust.
Structure: output (drives the 3D view), input color / grayscale (subgraphs), subgraph instances, bitmap, SVG, text, curve, pattern program map, pixel / value processor, input value, comment, frame, dot.

## Paint mode (texturing a mesh)
Start a project from a mesh and paint its textures in a layer stack, PBR channels at a time (base colour, roughness,
metallic, height, normal, AO, …), one stack per texture set.

- **New project:** File > New project (Ctrl+N) or the start screen: pick an OBJ or glTF mesh and get one texture set per
  material, several objects per project, a default resolution up to 4K, the normal map format, physical size, cameras from
  glTF and baked maps assigned by name (`TextureSetName_MeshMapName`); project templates, built in or saved by you.

- **Layers and masks:** paint, fill and folder layers; masks with their own effect stacks; blend modes and opacity per
  channel; levels, colour selection on the ID map, anchors/references, polygon fill, effect layers; thumbnails, drag and
  drop, duplicate, group, layer instances, geometry masks, see-through layers. Every edit is one undo step, with a
  history timeline.
- **Painting:** brush and eraser on the 3D model (strokes pulled into texture space per texel, so they cross UV seams),
  pressure (on Linux Wayland through Xwayland), a brush cursor on the surface, lazy mouse, straight lines with angle snap,
  mirror and radial symmetry, quick mask, polygon fill by triangle / face / mesh piece / UV island, smudge, clone,
  stencils and projection painting, the material picker, painting in the 2D UV view; the brush engine is PhotoCraft's
  (pinned, behind one adapter). Strokes are stored next to the pixels, so they can be replayed.
- **Procedural layers:** fills with tiling / UV / tri-planar / planar / spherical / cylindrical / warp projection (with an
  editable warp grid); materials from the graph editor as fills; procedural textures per channel; MaterialCraft's own mask generators driven by baked maps (dirt, edge wear, ambient occlusion,
  curvature, mask builder, UV border, height combination), noise / grunge / pattern textures and island-aware filters (blur,
  sharpen, bevel, warp, slope blur, HSL, …), all written from scratch. Every effect states how far it reads around a
  texel (its reach, recorded in a tested ledger), so a change recomputes only what it can affect.
- **Speed:** a brush stroke recomposites only the tiles it painted (through generators, filters and anchors), a stack
  cache keeps expensive masks and the composite below the edited layer, and undo touches only the texture sets it
  changed; `mcperf` measures it all against a history.
- **Asset tray:** browse, filter, search (`steel*`, `!exact`, `"phrase"`, `tag:`, `-exclude`) and saved searches over your
  libraries; drag materials, smart materials, smart masks, brushes and alphas onto the stack or the brush; rename, edit,
  move and delete assets from a context menu (one asset store does every write). A starter library is generated on first
  run (`mcstarter`): materials from MaterialCraft's own graphs, 116 reference materials from the CC0 values of
  physicallybased.info, brushes, alphas, paper / canvas / noise and relief textures from PhotoCraft's generators, smart
  masks and a studio environment. Documents embed the assets they use, so they open anywhere.
- **Baking:** per texture set from a high-poly mesh or the model itself: normal, world normal, AO, curvature, position,
  thickness, ID; per-set bake settings, a bake queue with progress and cancel, and a Baking layout (F8).
- **Workspace:** dockable panels and saved layouts, rebindable shortcuts (Alt-based navigation in the viewport), colour
  picker and eyedroppers, session log, File > Save / Save As.
- **Import:** Substance 3D Painter projects (`.spp`) are converted into `.mcpaint`: mesh, texture sets, baked maps, the
  layer structure with blend modes, fills, masks, native effects and brush strokes. Procedurals compiled into the project
  can be replaced by MaterialCraft's own equivalents in an import review (or kept as the project's saved result). The importer
  only reads your own files; nothing vendor-made is shipped. The review can also add the project's brushes, alphas,
  stencils and textures to your own library.

## Command-line tools
    materialcraft render <graph.mcg|.mca> <outdir> [--sizes 512,1024] [--depth 16] [--param "Label=0.5"] [--variations 8] [--render3d]
    materialcraft publish <graph.mcg> <out.mca>      # embeds bitmaps, flattens editor data
    materialcraft bake <low.obj> <high.obj> <outdir> [--size 1024]
    materialcraft nodes | check | params | testmesh | example

    mcsub convert <in.spp> <out.mcpaint> [--procedurals keep|replace]   # import a project
    mcsub convert --all <folder> <outdir> [--discard] [--jobs N]        # many projects at once (memory-aware)
    mcsub analyze <in.spp>                                              # procedurals and available replacements (JSON)
    mcsub inspect | tree | doc | model | report | maps | compare | verify-layers …
    mcparity run <subject|all> --projects <list> [--jobs N]             # measure our results against the project's saved ones
    mcperf list | run <id|all> | compare                                # performance scenarios with a history
    mcstarter <folder>                                                  # build the starter asset library (deterministic)

## Workspace layout
`src/` the core library (graph engine and nodes, `paint` document model and compositor, `mesh`, `bake`, `projection`,
`stamp`, `generators`, `filters`, `proc_textures`, `assets`, `history`, …) · `crates/editor` the editor ·
`crates/substance` the `.spp` importer (`mcsub`) · `crates/parity` result measurements (`mcparity`) ·
`crates/brush` the brush adapter · `crates/starter` the starter library (`mcstarter`) · `crates/perf` performance
scenarios (`mcperf`). Contributor rules: [AGENTS.md](AGENTS.md).

## Platforms
Windows, Linux and macOS use the same code: `eframe` (glow / OpenGL 3.3 core) for the window and the 3D viewport, `rfd` for
file dialogs, plus the shared Craft libraries (`craft-ui` docking, `craft-settings`, `craft-atomic`, `craft-cms`, `craft-quit`, from
https://github.com/storytold/craft-libs, MIT OR Apache-2.0). The core library and the command-line tool still use only the Rust standard
library unless built with `--features cms` or `--features gui`. Settings live in `config.toml` under `%APPDATA%\materialcraft`, `~/.config/materialcraft` or
`~/Library/Application Support/materialcraft`. GitHub Actions (`.github/workflows/ci.yml`) builds and tests all three.
Linux needs the usual X11/Wayland/GL development packages (`libxkbcommon-dev libwayland-dev libx11-dev libgl1-mesa-dev`).

## Not implemented (honest list)
**Material graph:** splines/paths, AxF import, OCIO/ACE colour management, scripting API, SVG gradients, OpenType (CFF)
fonts, path-traced 3D rendering and custom mesh import in the graph editor's 3D view.
**Paint mode:** texture export, UDIMs, USD / FBX / Alembic meshes, automatic UV unwrapping, tablet tilt, native Wayland
pen input (Xwayland is used instead), effect groups with their own blending; replacements for the compiled procedurals of imported projects cover the common mask generators, textures and
filters only, and their looks are still being tuned.
See ROADMAP.md for the complete list.

## Licence
Dual licensed under [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT), at your option. See [NOTICE](NOTICE),
[THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md), [ATTRIBUTION.md](ATTRIBUTION.md) and
[INDEPENDENT_IMPLEMENTATION.md](INDEPENDENT_IMPLEMENTATION.md) (how we keep the code original). MaterialCraft is not
affiliated with any other software vendor.

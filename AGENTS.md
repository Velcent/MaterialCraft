# AGENTS.md: guide for AI agents and contributors

**MaterialCraft** is a procedural, node-based material editor written in Rust, with a Paint mode for layered texture
painting on meshes that can import Substance 3D Painter projects. It is a member of the Craft app suite (PhotoCraft
and siblings) and follows the suite's conventions. Write the product name as **MaterialCraft** in user-facing text;
machine names stay lowercase (`materialcraft`, `materialcraft-substance`, `mcsub`).

Internal research, design docs, decisions and the roadmap live **outside this repository** (in `MaterialCraft-docs`
next to it). Never copy them into the repo; refer to them by path in comments when needed.

## Workspace map

```text
src/                      materialcraft: std-only core library + the `materialcraft` CLI (src/main.rs)
  graph nodes2 ops ...    node graph and procedural nodes
  paint/                  Paint mode document model: tiled images, channels, layers, compositing, .mcpaint format
  mesh/ bake render3d     meshes (OBJ/glTF), baking, CPU 3D preview
  gpu3d/                  OpenGL viewport through glow (feature "gui" only)
  gui.rs                  editor binary `materialcraft-gui` (feature "gui"); include!s gui_params/gui_ui/gui_home.rs
  gui_paint/ gui_theme.rs Paint mode screen and the editor theme
crates/substance          materialcraft-substance: reads .spp projects; `mcsub` (inspect, convert, maps, compare)
crates/parity             materialcraft-parity: `mcparity`, measures our results against Painter's on local projects
```

## Build and test

```sh
cargo build --workspace --all-features
cargo test --workspace                       # add --features gui for the editor's tests
cargo clippy --workspace --all-targets --all-features
cargo fmt --all
cargo run --release --features gui --bin materialcraft-gui
cargo run --release -p materialcraft-substance --bin mcsub -- convert <in.spp> <out.mcpaint>
```

Toolchain: Rust 1.95 or newer, edition 2024. Formatting follows `rustfmt.toml` (wide lines, as in the other Craft apps).

## Rules

1. **Clean room.** Nothing proprietary goes into the repository: no Adobe/Substance code, data, shaders, presets,
   resources or documentation text. Everything we need is written from scratch from observed behaviour and open
   standards. Substance files are only *read* by the importer (`crates/substance`) at run time; nothing read from a
   user's project (images, numbers, corpus files) is ever committed.
2. **Module boundaries.**
   - The paint model (`src/paint`) knows no concrete procedural; procedurals plug in behind its interfaces.
   - Mapping tables from Painter's concepts to ours live only in the importer's translate stage
     (`crates/substance/src/translate`); the layers below it know nothing about MaterialCraft's model.
   - The editor is thin: it shows state and turns input into calls on the model; logic lives in the libraries.
3. **Object-oriented and decoupled.** Small types with one job each, wired together explicitly. No god objects or god
   files: split a file before it grows into one.
4. **No drive-by refactors.** A change does what it says. Refactors, renames and formatting go in their own commits.
5. **Never crash (new code).** Non-test code in new crates doesn't panic on any input: no `unwrap()`, `expect()`,
   `panic!`, `todo!`, `unimplemented!`, `unreachable!` or unchecked indexing. Return an error and propagate it with
   `?`. New crates start their root with
   `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::unimplemented, clippy::todo, clippy::unreachable)]`
   (`clippy.toml` allows them in tests). Existing code isn't converted wholesale; tighten it when you touch it, in
   its own commit.
6. **No `unsafe`.** The workspace sets `unsafe_code = "forbid"`. The root package uses `deny` instead because of two
   documented exceptions, each with a module- or function-level `allow` and `SAFETY:` notes:
   - `src/gpu3d`: glow exposes every OpenGL call as `unsafe fn`; the module's doc states the invariant (calls only
     inside egui_glow paint callbacks, objects freed by their owner).
   - `src/ops.rs` `box_cols1`: threads write disjoint column bands of one buffer through a raw pointer.

   Don't add `unsafe` anywhere else.
7. **Attribution.** Code copied from another Craft app (PhotoCraft is MIT OR Apache-2.0) keeps its licence notice in
   the file and gets a row in `ATTRIBUTION.md` in the same commit. Don't change the licence or add licence files: the
   licence is decided when Paint mode merges to main.
8. **Commits** explain why, build on their own, and keep mechanical changes (formatting, renames) apart from logic.

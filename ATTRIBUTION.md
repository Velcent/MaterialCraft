# Attribution

Code and assets in this repository that come from somewhere else, with their author, source and licence.

MaterialCraft reuses code from its sibling apps in the Craft suite where that saves work. PhotoCraft is licensed
MIT OR Apache-2.0; code copied from it (or from another Craft app or crate) keeps its copyright and licence notice at
the top of the copied file and gets a row below **in the same commit**, naming the source file and the revision it was
copied from. Third-party material must be permissively licensed and keep its licence file next to it.

Nothing proprietary is ever listed here, because nothing proprietary may be copied in (see the clean-room rule in
`AGENTS.md`).

This file lists only code and assets copied into the repository. MaterialCraft's own licence is in `LICENSE-MIT`,
`LICENSE-APACHE` and `NOTICE`; libraries the build links (crates, including craft-libs) are listed in
`THIRD_PARTY_LICENSES.md`, not here.

## Copied code

| Path | What | Author | Source (repository, file, revision) | Licence |
|---|---|---|---|---|
| `src/diag/time.rs` | UTC timestamp formatting (`timestamp`, `civil_from_days`) and its test | ArtCraft Team and the PhotoCraft contributors | PhotoCraft, `apps/photocraft/src/logging.rs`, revision 57437ea | MIT OR Apache-2.0 |
| `crates/editor/src/logging/filter.rs` | `RUST_LOG`-style level filter (`Filter`) and its tests | ArtCraft Team and the PhotoCraft contributors | PhotoCraft, `apps/photocraft/src/logging.rs`, revision 57437ea | MIT OR Apache-2.0 |
| `crates/editor/src/logging/file.rs` | Log rotation, line format and the size-capped log file writer, with tests | ArtCraft Team and the PhotoCraft contributors | PhotoCraft, `apps/photocraft/src/logging.rs`, revision 57437ea | MIT OR Apache-2.0 |
| `crates/editor/src/logging/logger.rs` | The `log` backend (`AppLogger`, `install`) and its tests, extended with the diagnostics bridge | ArtCraft Team and the PhotoCraft contributors | PhotoCraft, `apps/photocraft/src/logging.rs`, revision 57437ea | MIT OR Apache-2.0 |
| `crates/editor/src/gui_widgets/slider/row.rs` | Slider row layout (label and value above a thin full-width track, round knob), the gradient track (colour sliders) and the double-click reset, rewritten for MaterialCraft's theme and modifier rules | ArtCraft Team and the PhotoCraft contributors | PhotoCraft, `crates/ui-egui/src/widgets.rs` (`slider_row_prec`, `slider_with`, `RowGestures`), revision 5f65579 | MIT OR Apache-2.0 |
| `crates/editor/src/gui_paint/input/stylus.rs` | Pen sample feed and its merge with egui's pointer (`PenSample`, `StylusFeed`, `Stylus`: touch force tracking, per-move interpolation of the frame's pen samples, pressure curve, tap pressure) and their tests, without PhotoCraft's app parts | ArtCraft Team and the PhotoCraft contributors | PhotoCraft, `crates/ui-egui/src/stylus.rs`, revision 5f65579 | MIT OR Apache-2.0 |
| `crates/editor/src/gui_color/painters.rs` | Colour field painter (grid mesh with a colour per vertex) and the transparency checker | ArtCraft Team and the PhotoCraft contributors | PhotoCraft, `crates/ui-egui/src/color_picker_ui.rs` (`grid_mesh`) and `crates/ui-egui/src/widgets.rs` (`checker`), revision 5f65579 | MIT OR Apache-2.0 |
| `src/projection/warp/bezier.rs` | Bicubic Bézier patch grid (`BezierMesh` layout, Bernstein weights, patch location, evaluation, the coarse-search-plus-Newton parameter search), generalised to 3D control points, with derivatives added | ArtCraft Team and the PhotoCraft contributors | PhotoCraft, `crates/geom/src/warp.rs` (`BezierMesh`), revision 5f65579 | MIT OR Apache-2.0 |

## Assets

| Path | What | Author | Source | Licence |
|---|---|---|---|---|

None yet.

# Attribution

Code and assets in this repository that come from somewhere else, with their author, source and licence.

MaterialCraft reuses code from its sibling apps in the Craft suite where that saves work. PhotoCraft is licensed
MIT OR Apache-2.0; code copied from it (or from another Craft app or crate) keeps its copyright and licence notice at
the top of the copied file and gets a row below **in the same commit**, naming the source file and the revision it was
copied from. Third-party material must be permissively licensed and keep its licence file next to it.

Nothing proprietary is ever listed here, because nothing proprietary may be copied in (see the clean-room rule in
`AGENTS.md`).

This file doesn't change MaterialCraft's own licence (the `license` field in `Cargo.toml`).

## Copied code

| Path | What | Author | Source (repository, file, revision) | Licence |
|---|---|---|---|---|
| `src/diag/time.rs` | UTC timestamp formatting (`timestamp`, `civil_from_days`) and its test | ArtCraft Team and the PhotoCraft contributors | PhotoCraft, `apps/photocraft/src/logging.rs`, revision 57437ea | MIT OR Apache-2.0 |
| `crates/editor/src/logging/filter.rs` | `RUST_LOG`-style level filter (`Filter`) and its tests | ArtCraft Team and the PhotoCraft contributors | PhotoCraft, `apps/photocraft/src/logging.rs`, revision 57437ea | MIT OR Apache-2.0 |
| `crates/editor/src/logging/file.rs` | Log rotation, line format and the size-capped log file writer, with tests | ArtCraft Team and the PhotoCraft contributors | PhotoCraft, `apps/photocraft/src/logging.rs`, revision 57437ea | MIT OR Apache-2.0 |
| `crates/editor/src/logging/logger.rs` | The `log` backend (`AppLogger`, `install`) and its tests, extended with the diagnostics bridge | ArtCraft Team and the PhotoCraft contributors | PhotoCraft, `apps/photocraft/src/logging.rs`, revision 57437ea | MIT OR Apache-2.0 |

## Assets

| Path | What | Author | Source | Licence |
|---|---|---|---|---|

None yet.

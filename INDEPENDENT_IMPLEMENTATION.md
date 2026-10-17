# Independent implementation policy

MaterialCraft is an original program. Everything in this repository is written for this project and is licensed under Apache-2.0 OR MIT (see `LICENSE-APACHE`, `LICENSE-MIT`). This page says how we keep it that way. It is a working policy for contributors, not legal advice.

## What goes in the repository

- Code, shaders, algorithms, icons, fonts, sample graphs, images and text that the contributor wrote, or that come from a source whose licence allows redistribution under Apache-2.0 OR MIT, with the source named in `THIRD_PARTY_LICENSES.md` or `NOTICE`.
- Algorithms taken from published literature or from first principles. Procedural looks (noises, grunges, patterns) are our own recipes.
- Parameter names and value ranges are chosen for this program. Where a name is a plain technical term (blur radius, threshold, seed) it is used as such.

## What stays out

- No source code, files, graphs, presets, bitmaps, icons, fonts, screenshots, documentation text or sample content from any commercial or proprietary product.
- No decompiling, disassembling, traffic or memory inspection of other software, and no use of material obtained under a licence that forbids it.
- No reading or writing of another tool's file formats unless the format is publicly documented, the documentation's licence allows implementing it, and the maintainers have agreed.
- No third-party trademarks in product, node, file-format or command names, and no wording that suggests affiliation or endorsement.
- No copying of another product's window layout, icon set, colour scheme or start screen. The editor's interface is designed for this project.
- Nothing from a vendor's tools, renders or files is committed, attached to issues, or kept in the repository tree. If a contributor compares output against other software privately, the result stays on their machine and the code is still written independently from public descriptions of the technique.

## For contributors

By opening a pull request you confirm that you wrote the change yourself (or have the right to contribute it), that it is not copied or translated from proprietary code or content, and that you license it under Apache-2.0 OR MIT. Do not contribute code you have seen under an NDA or a licence that restricts using what you learned.

If you are unsure whether something is safe to add, ask first in an issue.

## Names

Node and feature names are generic descriptions of what they do ("Blur", "Levels", "Voronoi", "Flood Fill"). If a name turns out to be a brand, it gets renamed. Reports are welcome and handled quickly.

# physicallybased.info material values (vendored)

`materials.json` holds the material values of the Physically Based database, from which `mcstarter` builds the
reference materials (`materials/reference/` in the starter library).

- Source: <https://github.com/AntonPalmqvist/physically-based-api>, file `deploy/v2/materials.json`, commit
  `c1194e780219a80f75f23512887abf633818c42f` (2026-10-11; data schema 2.2, header `updated` 202610110901),
  downloaded 2026-10-18. Also served at <https://api.physicallybased.info/v2/materials>.
- Licence: **CC0 1.0 Universal** (the repository's LICENSE and the file's own header). No attribution is required;
  MaterialCraft credits the source in `ATTRIBUTION.md` and in each reference material's sidecar.
- Changed: values only. Each entry's `description`, `references` and `images` were removed; everything else (names,
  categories, tags, colours, metalness, roughness, IOR, density and the other measured values) is as published.
- Not vendored: the site's preview renders. They show a shader ball under CC BY 4.0, so MaterialCraft renders its own
  thumbnails.

To update, download the file again, drop the same three keys from every entry, update the commit and date above and
bump `LIBRARY_VERSION` in `crates/starter/src/manifest.rs`.

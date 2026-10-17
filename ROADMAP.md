# MaterialCraft Roadmap

MaterialCraft is a node-based procedural material editor written in pure Rust (Windows, Linux, macOS). The core library has zero dependencies; only the GUI binary uses eframe + rfd. File formats: `.mcg` (editable graph), `.mca` (published package).

Status note: everything has been built and tested on Linux only (CI for Windows, Linux and macOS is set up in `.github/workflows/ci.yml` but has not run yet). The GPU viewport has been verified on Mesa llvmpipe only, not on real GPUs.

---

## 1. Implemented (129 node types)

### Generators: noise
- White Noise
- Gaussian Noise
- Perlin Noise
- Fractal Noise (fbm, ridged, turbulence modes)
- Clouds
- Plasma
- Voronoi (distance, cell ID, edge, F2 modes)
- Crystal
- BnW Spots
- Directional Noise (seamless at any angle)
- Grunge

### Generators: shapes and gradients
- Circle, Rectangle, Polygon, Star, Shape
- Gradient (linear, radial), Axial Gradient, Circular Gradient
- Checker
- Uniform Color

### Generators: patterns
- Bricks
- Stripes (seamless at any angle)
- Weave
- Tile Sampler, Tile Generator
- Splatter, Circular Splatter

### Generators: custom
- Pixel Processor (per-pixel expression language)
- Bitmap import (PNG)

### Filters: blur and sharpen
- Blur, Blur HQ, Directional Blur, Slope Blur
- Sharpen, High Pass

### Filters: color and values
- Levels, Adjust, Invert, Threshold
- Histogram Scan, Histogram Range, Quantize, Clamp
- HSL, Tint, Gradient Map, Color Match
- To Gray, Channel, RGBA Merge

### Filters: geometry
- Transform 2D, Mirror, Swirl
- Warp, Directional Warp, Vector Warp
- Make It Tile Photo, Make It Tile Patch

### Filters: compositing and analysis
- Blend (with blend modes), Switch
- Flood Fill, Flood Fill Random, Flood Fill Color, Flood Fill Gradient
- Bevel, Distance, Edge Detect, Emboss

### Material and surface
- Normal From Height, Normal Blend
- Ambient Occlusion, Curvature, Curvature Smooth
- Height Blend
- Edge Wear, Dirt, Dust

### SDF
- SDF Shape, SDF Combine, SDF Mask, SDF 3D Form

### Graph structure
- Output (11 usages: basecolor, normal, roughness, metallic, height, ambient_occlusion, opacity, emissive, diffuse, specular, glossiness)
- Input (for subgraphs), Subgraph, Dot (link re-route)
- Comment, Frame

### Editor and workflow
- Packages workspace: any number of packages open at once, shown in the Explorer as roots (icon, italic bold name, `*` when modified) with their graphs underneath; each graph expands to list its outputs. File > New > Material Graph (Ctrl+N) adds a graph to the selected package; File > New > Empty Package creates a new unsaved package (preset chooser with graph name); File > Open adds packages without closing others. Right-click empty Explorer space: New Package / New Material Graph / Open Package(s); right-click a package: New > Material Graph, Save Package, Close Package (asks when unsaved). Graphs from any open package can be dragged onto the canvas to become nodes
- Graph nodes have one labelled output socket per output (Base Color, Normal, Roughness, ...); links remember which output they read (`in=@3.height` in the file format), sockets with links are filled orange
- Graphs as nodes: any saved graph (`.mcg`/`.mca`) can be dragged from the Explorer onto the canvas, dropped from the OS onto the window; it becomes a subgraph node (input sockets only appear if the graph defines inputs) whose exposed parameters and chosen output work like any node, so complex materials can be built from simpler ones
- Node graph with zoom, pan, rubber-band select, drag-to-connect from either end, cycle protection
- Tab search with auto-connect, undo/redo (100 steps), copy/paste (also via OS clipboard), duplicate, auto-layout
- Subgraph creation and editing, multiple graphs, exposed parameters
- Per-node property panels with color wheel and gradient editor
- Home screen with recent files, New Graph dialog, node library with thumbnails
- Dockable panels (shared `craft-ui` docking): drag a tab to another group, to a side or out into a floating panel; Windows > Reset layout; the layout is remembered between runs; the graph canvas stays fixed
- Settings in `config.toml` (shared `craft-settings`): atomic saves, backup file, damaged files moved aside; old `settings.txt` / `recent.txt` are imported on first run
- Forced dark theme (does not follow the operating system's light theme)

### 3D viewport (GPU, OpenGL 3.3 via egui_glow)
- 13 preview objects: Cube, Cylinder, Hollow box, Inner box (seen from inside), Plane, Plane (hi-res), Sphere, Cloth, Mat ball, Rounded cube, Rounded cylinder, Sphere 2 tiles, Torus (mesh resolution up to 768 x 768; all closed shapes stay crack-free when displaced)
- True outward displacement, plus parallax relief and bump modes
- GGX PBR, mipmaps with anisotropic filtering, 4x MSAA (falls back to none), contact shadows, hue-preserving filmic tone mapping (bright oranges no longer wash out to pink), horizon-occluded reflections
- Light, camera, exposure and ambient controls, UV tiling, wireframe, normal strength; the key light follows the camera by default (Lights > Light follows the camera), a soft fill light keeps shadowed sides readable
- Studio reflection environment (softboxes, rim strip, horizon) that blurs with roughness, so roughness and metallic changes read clearly
- CPU renderer as automatic fallback (Renderer menu)
- Headless test: `cargo run --release --features gui --example gpu_shot -- out.png [shape] [relief] [w h] [nomsaa] [wire]`

### Export, files and tools
- Export to PNG, JPEG, TIFF at 8 or 16 bit, EXR, HDR
- Crash-safe writes for graphs, packages and exports (`craft-atomic`: staged file, flush, atomic replace)
- Output colour space on export for colour maps: sRGB, P3, wide-gamut RGB, ROMM RGB, Rec. 2020, linear sRGB (`craft-cms` ICC engine); CLI `--colorspace`
- Several resolutions at once (256 to 8192 px) and per-output selection
- `.mcg` editable graphs, `.mca` published packages with exposed parameters
- Mesh baking from OBJ (BVH): normal, world normal, AO, curvature, thickness, position, ID
- CLI: render, publish, params, bake

### Quality and resolution
- Graph size 128 to 8192 px; the editor previews at the graph's own size (2048 px by default for a 2K graph, capped at 4096 for the preview), shows a quick 256 px pass first and refines in the background. All parameters are resolution independent (checked by a test)
- Every node tiles seamlessly: centred shapes and gradients wrap around the tile border, checker/weave use even counts, flood fill regions that cross the border keep one continuous gradient, Transform 2D cross-fades only when the matrix cannot tile by itself. `tests/seam.rs` renders every node and 100+ parameter variants and fails on any visible seam
- Anti-aliased hard edges (shapes, checker, bricks), bicubic resampling in Transform/Warp, Scharr-filter normals, fast gaussian blur (cost independent of radius, multi-threaded), 4096-entry gradient table for noise, rotated fractal octaves (no visible grid), spots as a union of discs (no cell-edge cuts)
- 2D view shows true texels: crisp squares when zoomed in, mip-mapped when zoomed out. The shipped example graph is 2048 px. Maps above 4096 px are box-averaged (not point-sampled) when they go to the 3D viewport
- Per-node Output Size: every node has Relative to Input (default), Relative to Parent or Absolute (16-8192 px), plus a -12..+12 power-of-two modifier. Inputs of another size are resampled automatically, so heavy nodes (AO, blurs, noises) can run at half or quarter resolution. Nodes with their own size show it under the card
- Node result cache: the editor keeps every node's result between evaluations, found by a hash of its settings, the evaluation size and the hashes of its inputs. An edit recomputes only the edited node and what is downstream of it; thumbnails and unchanged outputs come straight from the cache. Least-recently-used entries are dropped above a memory budget (3 GB, set `MC_CACHE_MB` to change), and grayscale results are stored with one channel. On the 2048 brick example (2 cores) editing the normal strength went from about 4.5 s to about 0.35 s; `cargo run --release --example cache_bench -- graph.mcg 2048 <node> <param> <v1> <v2>` measures it
- The node shown in the 2D view is evaluated first and put on screen as soon as it is done, before the outputs, the 3D maps and the other thumbnails
- Faster kernels (cold run of the 2048 brick example: 3.9 s to about 1.4 s on 2 cores): Fractal Noise builds its gradient lattices once (about 1.8x faster, identical values); grayscale blurs run on one channel and the vertical pass needs no transpose (Ambient Occlusion about 4x faster); Normal From Height, Levels and Blend run on all cores (Normal about 6x, Levels about 4x faster); Slope Blur uses grayscale planes; Directional Noise samples the streak less densely (2x faster, mean difference below 0.01); bilinear and bicubic sampling skip the wrap arithmetic inside the image; the editor shares the kept output textures instead of copying them
- Responsive previews: editing cancels the running evaluation (it stops within a few rows, even inside a heavy node) and starts the new one immediately; previews are progressive (256, 512, 1024 ... up to the preview size) so a change shows in a fraction of a second; the 2D image and GPU textures are prepared on the worker thread, so the editor never stalls; switching or entering a graph clears the old previews at once
- Memory: intermediate textures are freed as soon as every node that reads them has finished, so large graphs at 4K fit

### Quality checks
- 69 automated tests, including: a wrap-around check that rolls the input of 30 filters and requires the output to roll identically (proves every filter wraps at the borders), seam checks on every node, every node at four resolutions (finite, sane range, not blank), every documented parameter fuzzed with extreme values (no panics/NaN), and whole-graph import and per-output links
- `cargo run --release --example node_sheet -- out_dir` renders a PNG for every node type for visual review

---

## 2. Not implemented yet

The full plan (milestones, settings, order, risks) is in `RELEASE_PLAN.md`.

### Graph logic
- [ ] Package context menu: Rename, Save As, Duplicate, Reveal in file explorer, Delete; graph context menu: Rename, Duplicate, Delete, Close Graph
- [ ] Several graph tabs open at once (one graph is shown at a time today)
- [ ] Imported graphs are embedded copies; no live link or "update from source" when the package file changes
- [ ] Size/format caption (e.g. 2048x2048 - C8) on graph nodes
- [x] Function graphs: about 85 `f_*` value nodes (variables, math, vectors, comparisons, logic, if / sequence, random, image sampling and statistics, inputs, casts, calls), a type checker, function-driven parameters (`fn.<param>`), pixel processor and value processor driven by a function
- [x] Pattern programs (our FX-Map): `fx_map` node running a graph of quadrant / iterate / switch / marker nodes with random seeds, depth limit, blend modes and tileable output
- [x] Pin (`f_pin`) and Input Value (`input_value`) nodes
- [x] Curve node (per-channel tone curves) with an interactive editor
- [x] Non-square graphs (width/height, `size W H`, tests), New Graph dialog with separate sizes, per-node `square_cells` (keeps cells square)
- [x] Live-linked packages, instances through shared links, format versioning, package tab bar, per-graph tabs, link picker for multi-graph packages, dependency manager (relink, embed copy, remove, search folders)
- [ ] Graph templates, parameter presets, randomized variations

### Nodes
- [ ] Spline and path tools (shapes along curves, tracks, seams, borders)
- [ ] Several hundred more nodes, for example:
  - more weaves and tile patterns
  - Non-Uniform Blur
  - Multi-Directional Warp
  - Cells and fur patterns
  - Emboss with Gradient
  - Histogram Equalize
  - Gradient Dynamic
  - Auto Levels
  - Cube Map tools
- [ ] Built-in libraries of ready-made materials, filters and generators

### 3D and rendering
- Custom light rigs (several lights, HDR environment images), a true seamless-edge check in the 3D view
- [ ] HDRI and image-based lighting (only an analytic studio light today)
- [ ] Load custom meshes and 3D scenes into the viewport
- [ ] Path-traced rendering and post-processing (bloom, DOF, SSAO)
- [ ] GPU tessellation (displacement is per vertex on a pre-built mesh today)
- [ ] Camera and light presets, turntable rendering

### Import and color
- [x] SVG import (own rasteriser: shapes, paths, transforms, solid fill and stroke), TrueType font import and text nodes (own reader and a built-in face). Not supported: SVG gradients / clipping / filters, CFF (.otf) fonts, kerning and complex-script shaping
- [ ] Bitmap import for JPEG, TIFF, EXR (only PNG today)
- [ ] AxF material import
- [x] ICC-based output colour spaces for colour maps on export (built-in profiles, `craft-cms`)
- [ ] OCIO config support, per-bitmap input colour space, ICC profile import, colour-managed 2D / 3D views
- [x] 32-bit float output: EXR, HDR and float TIFF export (graph clamps still to be reviewed)

### Publishing and automation
- [ ] Unreal / Unity plugins and a small runtime for published packages
- [ ] Scripting API and batch automation
- [ ] Engine-specific output templates (Unreal ORM packing and similar)
- [ ] Texture atlases for the shape/splatter workflow

### Polish and verification
- [ ] Library thumbnails are only 64 px and generated once at startup
- [ ] `.ico` resource for the Windows exe file (the window and taskbar icon already work on all platforms; the release build hides the console window on Windows)
- [ ] Visual check of the 2D/3D splitter and the export window
- [ ] Run and verify on Windows and macOS
- [ ] Test the GPU viewport on real GPUs (NVIDIA, AMD, Intel, Apple)
- [ ] CPU fallback renderer still uses the old plain environment (roughness/metallic look flatter than on the GPU viewport)
- [ ] GPU viewport: wireframe at mesh resolution 256+ renders as a solid blob; map preparation runs on the main thread

---

## 2b. Feature gaps by area

We have 129 node types. Missing, by area (described by function; `NODES.md` lists what exists):

**Property coverage.** Done so far: the shared noise block (disorder, disorder speed, anisotropy, tile offset), rebuilt scatter nodes (built-in patterns, six inputs, offsets, masks, blending, map inputs, rings), radial blur, edge detect, mosaic and curvature, Voronoi / Cells / Grunge properties, a jittered triangle mesh style, the weave generator, the 15 grunge maps, gradient-map addressing and alpha, colour swapping, horizon AO with world units, colour quantising, multi-output nodes (including Voronoi, Cells, Splatter and the Flood Fill mappers), float export, graph-wide node precision, non-square graphs, typed pins and value pin types, per-node output format, live links, optional GPU compute. Still open: some node properties that need engine features (extra image inputs, function graphs, baked mesh inputs).

### Noises and generators
- Fur-like, fibre-like, fluid-like, liquid, caustic, moisture, microscope-style, anisotropic and blue noise; more cloud and directional-noise variants; dirt gradients; more grunge recipes
- 3D noises: perlin, fractal perlin, ridged, simplex, voronoi, worley

### Patterns and shapes
- Pavement and mesh patterns, more brick and checker variants, fibres, gaussian and gradient variants, scratch generators, extruded and mapped shapes, panorama shapes, random tile patterns
- Shape splatter with blending, data extraction and mask output
- 3D gradients and volume masks

### Adjustments, blending, blurs, channels
- Contrast / luminosity, linear and sRGB conversion, colour lookup tables, HDR range viewer, frequency mapping, non-uniform and selecting histogram tools, lighting-cancel filters, luminance highpass, min / max
- A multi-way switch; pre-multiplied <-> straight alpha conversion

### Effects and warps
- Cross sections, curvature via Sobel, diffusion (colour, grayscale, UV), gloss-aware embossing, extend shape, FXAA, reaction diffusion, summed area tables, vector morph
- Nearest sampling in warps
- Flood-fill mapping outputs (bounding box size, index, position)
- Ray-traced nodes: AO, shadows, irradiance, bent normal; 3D texture nodes

### Normal map tools
- Bent and facing normals, height-normal blending, world-unit height to normal, normal sobel and transform

### Transforms
- Auto crop, polar conversion, clone, noise upscaling, safe transform, non-square transform, non-uniform rotation, symmetry slice, trapezoid and quad transforms

### Material filters
- Material blending (colour, adjustment, height, multi-material), snow / season / water effects
- PBR utilities: workflow converters, validators, dielectric and metal reflectance, PBR preview
- Scan processing: AO cancellation, atlas tools, patch cloning, colour equalising, cropping, multi-angle capture to albedo / normal, smart auto tiling
- One-click bitmap to material

### Mesh-based generators (driven by baked mesh maps)
- Wear, dirt, dust, edge damage, leaks, grease, bleaching, brush and similar masks; weathering looks (cracks, fabric, leather, metal, moss, rock, rust)
- Utilities: tri-planar and planar projection, mesh data combining, material selection
- Our Dirt / Dust / Edge Wear nodes work from a height map only; baked maps are not yet usable as graph inputs

### Splines and paths
- Create (cubic, quadratic, circle, point list, bridge), assemble (merge, append, select), modify (transform, warp, sample thickness / height)
- Render (scatter on spline, fill, mappers, render), path tools (mask to paths, paths to splines, transform, warp, polygon, select, preview, vertex processing)

### Graph system
- Function graph extras: loops, sampler border modes, bitwise operators
- Portal nodes, parameter presets, randomized variations, graph templates
- Tiling modes (none / horizontal / vertical)

### 3D view and rendering
- HDRI / image-based lighting and HDRI tool nodes (blackbody, exposure, HDR merge, physical sun / sky, area lights, panorama tools)
- Path-traced renderer, post-processing (bloom, DOF, SSAO, tone-mapping choices), camera and light presets, turntable
- Custom mesh and 3D scene loading, several meshes at once

### Import, export, colour, automation
- JPEG, TIFF, EXR, HDR bitmap import; SVG gradients; OpenType (CFF) fonts
- OCIO and ICC colour management
- Scripting API and batch automation, engine output templates (Unreal / Unity packing), UDIM, texture atlases, a built-in asset library
- Editor extras: dependency / resource manager, user library folders, navigator minimap, customizable shortcuts, language packs

### Engine
- Evaluation is multi-threaded CPU, with an optional GPU compute path (`--features gpu`, Tools > GPU acceleration) for Perlin / fractal noise, blur, directional blur, levels, blend and normal-from-height at 512 x 512 and up; results match the CPU within rounding (tests/gpu.rs). Each GPU node still reads its result back, so the next steps (Milestone 10) are more kernels and keeping chains resident on the GPU. Not yet checked on real GPUs

---

## 3. Suggested order

1. Function graphs + pattern programs (done, Milestone 1); next the node library by area (Milestone 2), which they unlock
2. Spline / path tools
3. HDRI image-based lighting and custom mesh loading
4. More nodes and a built-in material library
5. Color management, EXR/JPEG/TIFF import, HDR export
6. Scripting/batch API and engine output templates

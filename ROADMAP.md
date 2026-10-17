# MaterialCraft Roadmap

MaterialCraft is a node-based procedural material editor written in pure Rust (Windows, Linux, macOS). The core library has zero dependencies; only the GUI binary uses eframe + rfd. File formats: `.mcg` (editable graph), `.mca` (published package).

Status note: everything has been built and tested on Linux only (CI for Windows, Linux and macOS is set up in `.github/workflows/ci.yml` but has not run yet). The GPU viewport has been verified on Mesa llvmpipe only, not on real GPUs.

---

## 1. Implemented (124 node types)

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
- Resizable 2D and 3D views

### 3D viewport (GPU, OpenGL 3.3 via egui_glow)
- 13 preview objects: Cube, Cylinder, Hollow box, Inner box (seen from inside), Plane, Plane (hi-res), Sphere, Cloth, Mat ball, Rounded cube, Rounded cylinder, Sphere 2 tiles, Torus (mesh resolution up to 768 x 768; all closed shapes stay crack-free when displaced)
- True outward displacement, plus parallax relief and bump modes
- GGX PBR, mipmaps with anisotropic filtering, 4x MSAA (falls back to none), contact shadows, hue-preserving filmic tone mapping (bright oranges no longer wash out to pink), horizon-occluded reflections
- Light, camera, exposure and ambient controls, UV tiling, wireframe, normal strength; the key light follows the camera by default (Lights > Light follows the camera), a soft fill light keeps shadowed sides readable
- Studio reflection environment (softboxes, rim strip, horizon) that blurs with roughness, so roughness and metallic changes read clearly
- CPU renderer as automatic fallback (Renderer menu)
- Headless test: `cargo run --release --features gui --example gpu_shot -- out.png [shape] [relief] [w h] [nomsaa] [wire]`

### Export, files and tools
- Export to PNG, JPEG, TIFF at 8 or 16 bit
- Several resolutions at once (256 to 8192 px) and per-output selection
- `.mcg` editable graphs, `.mca` published packages with exposed parameters
- Mesh baking from OBJ (BVH): normal, world normal, AO, curvature, thickness, position, ID
- CLI: render, publish, params, bake

### Quality and resolution
- Graph size 128 to 8192 px; the editor previews at the graph's own size (2048 px by default for a 2K graph, capped at 4096 for the preview), shows a quick 256 px pass first and refines in the background. All parameters are resolution independent (checked by a test)
- Every node tiles seamlessly: centred shapes and gradients wrap around the tile border, checker/weave use even counts, flood fill regions that cross the border keep one continuous gradient, Transform 2D cross-fades only when the matrix cannot tile by itself. `tests/seam.rs` renders every node and 100+ parameter variants and fails on any visible seam
- Anti-aliased hard edges (shapes, checker, bricks), bicubic resampling in Transform/Warp, Scharr-filter normals, fast gaussian blur (cost independent of radius, multi-threaded), 4096-entry gradient table for noise, rotated fractal octaves (no visible grid), spots as a union of discs (no cell-edge cuts)
- 2D view shows true texels like Designer: crisp squares when zoomed in, mip-mapped when zoomed out. The shipped example graph is 2048 px. Maps above 4096 px are box-averaged (not point-sampled) when they go to the 3D viewport
- Per-node Output Size like Designer: every node has Relative to Input (default), Relative to Parent or Absolute (16-8192 px), plus a -12..+12 power-of-two modifier. Inputs of another size are resampled automatically, so heavy nodes (AO, blurs, noises) can run at half or quarter resolution. Nodes with their own size show it under the card
- Node result cache (Designer-style): the editor keeps every node's result between evaluations, found by a hash of its settings, the evaluation size and the hashes of its inputs. An edit recomputes only the edited node and what is downstream of it; thumbnails and unchanged outputs come straight from the cache. Least-recently-used entries are dropped above a memory budget (3 GB, set `MC_CACHE_MB` to change), and grayscale results are stored with one channel. On the 2048 brick example (2 cores) editing the normal strength went from about 4.5 s to about 0.35 s; `cargo run --release --example cache_bench -- graph.mcg 2048 <node> <param> <v1> <v2>` measures it
- The node shown in the 2D view is evaluated first and put on screen as soon as it is done, before the outputs, the 3D maps and the other thumbnails
- Faster kernels (cold run of the 2048 brick example: 3.9 s to about 1.4 s on 2 cores): Fractal Noise builds its gradient lattices once (about 1.8x faster, identical values); grayscale blurs run on one channel and the vertical pass needs no transpose (Ambient Occlusion about 4x faster); Normal From Height, Levels and Blend run on all cores (Normal about 6x, Levels about 4x faster); Slope Blur uses grayscale planes; Directional Noise samples the streak less densely (2x faster, mean difference below 0.01); bilinear and bicubic sampling skip the wrap arithmetic inside the image; the editor shares the kept output textures instead of copying them
- Responsive previews: editing cancels the running evaluation (it stops within a few rows, even inside a heavy node) and starts the new one immediately; previews are progressive (256, 512, 1024 ... up to the preview size) so a change shows in a fraction of a second; the 2D image and GPU textures are prepared on the worker thread, so the editor never stalls; switching or entering a graph clears the old previews at once
- Memory: intermediate textures are freed as soon as every node that reads them has finished, so large graphs at 4K fit

### Quality checks
- 60 automated tests, including: a wrap-around check that rolls the input of 30 filters and requires the output to roll identically (proves every filter wraps at the borders), seam checks on every node, every node at four resolutions (finite, sane range, not blank), every documented parameter fuzzed with extreme values (no panics/NaN), and whole-graph import and per-output links
- `cargo run --release --example node_sheet -- out_dir` renders a PNG for every node type for visual review

---

## 2. Not implemented yet

### Graph logic
- [ ] Package context menu: Rename, Save As, Duplicate, Reveal in file explorer, Delete; graph context menu: Rename, Duplicate, Delete, Close Graph
- [ ] Several graph tabs open at once (one graph is shown at a time today)
- [ ] Imported graphs are embedded copies; no live link or "update from source" when the package file changes
- [ ] Size/format caption (e.g. 2048x2048 - C8) on graph nodes
- [ ] Function graphs (variables, math, conditionals, function node family)
- [ ] FX-Map (recursive, rule-based pattern generation)
- [ ] Pin node, Input Value node
- [ ] Non-square graphs
- [ ] Graph instances and dependency management between packages
- [ ] Graph templates, parameter presets, randomized variations

### Nodes
- [ ] Spline and path tools (shapes along curves, tracks, seams, borders)
- [ ] More of Designer's several hundred nodes, for example:
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
- [ ] SVG import, font import, text nodes
- [ ] Bitmap import for JPEG, TIFF, EXR (only PNG today)
- [ ] AxF material import
- [ ] OCIO and Adobe Color Engine color management (sRGB only today)
- [ ] Linear/HDR 32-bit float output and EXR export

### Publishing and automation
- [ ] `.sbs` / `.sbsar` compatibility and Unreal / Unity plugins
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

## 2b. Designer feature gap (checked against Adobe's node library and release notes, Oct 2026)

We have 124 node types; Designer's library has roughly 500. Missing, by area:

**Property parity:** `NODES.md` lists, for each of our 122 nodes, the properties Designer documents, what our Properties panel has and what is missing. Done so far: the cheap one-property gaps (Phase 1) the shared noise block (Disorder speed / anisotropy, Tile offset; Phase 2) and the Tile Sampler / Tile Generator / Splatter / Splatter Circular rebuilds (Phase 3: built-in patterns, six inputs, offsets, masks, blending, map inputs, rings) and the four nodes that behaved differently (Phase 4: Radial Blur spin, Edge Detect lines, Mosaic map, Curvature from normal maps) and Voronoi / Voronoi Fractal / Cells / Grunge properties (Phase 5) and Triangle Grid mesh style plus the new Weave Generator (Phase 6) and the Grunge Map family (Phase 6b) and Phase 7 (crack look, Gradient Map addressing/alpha, Color Match colour swap) and Phase 8 (horizon AO with world units, Quantize Color), see its status section. Still open: the other large rebuilds (Grunge Map 001-015 images, Triangle Grid, Gradient Map editor, Color Match, Quantize Color, RTAO / world-unit AO), and the properties that need engine features (multi-output nodes, extra image inputs, function graphs, baked mesh inputs).

### Noises and generators
- (done in batch 1: Cells, Fractal Sum, Waveform, Gaussian Spots) Fur 1-3, Fluid, Liquid, Caustics, Moisture Noise, Messy Fibers 1-3, Microscope View, Anisotropic Noise, Blue Noise, Voronoi Fractal, Clouds 2-3, Directional Noise 2-4, Directional Scratches, Dirt 1-5, Dirt Gradient, White Noise Fast
- 3D noises: 3D Perlin, Perlin Fractal, Ridged Fractal, Simplex, Voronoi, Voronoi Fractal, Worley
- Grunge: Grunge Map 001-015 and the named grunges (Concrete, Damas, Galvanic, Leaks, Leaky Paint, Rough Dirty, Rust Fine, Scratches x3, Shavings, Splashes Dusty, Spots Dirty); we only have one 4-variant Grunge node

### Patterns and shapes
- Alveolus, Arc Pavement, Brick 2, Brick Generator, Checker 2, Fibers 1-2, Gaussian 1-2, Gradient Linear 1-3, Height Extrude, Mesh 1-2, Panorama Shape, Polygon 2, Scratches Generator, Shape Extrude, Shape Mapper, Tile Random 1-2, Weave 2, Weave Generator
- Shape Splatter family: Shape Splatter, Splatter Blend, Splatter Data Extract, Splatter to Mask
- 3D Linear Gradient, 3D Volume Mask, Cube 3D (and GBuffers)

### Adjustments, blending, blurs, channels
- (done in batch 1: Auto Levels, Channel Mixer, Color to Mask, Grayscale Conversion, Histogram Shift, Pow, Replace Color) Contrast/Luminosity, Convert to Linear / sRGB, Hald CLUT, HDR Range Viewer, Height Map Frequencies Mapper, Histogram Scan Non-Uniform, Histogram Select, Lighting Cancel High/Low Frequencies, Luminance Highpass, Min Max
- Blend modes: Multi Switch (we have 20 modes incl. Color Dodge/Burn, Linear Burn/Light, Hard Light, Exclusion, Divide, Color, Luminosity, Dissolve)
- Blurs: Anisotropic, Non Uniform and Radial Blur are done (batch 1)
- Channels: Alpha Merge done; RGBA Split, Alpha Split (need multi-output nodes), Pre-multiplied <-> Straight

### Effects and warps
- Cross Section, Curvature Sobel, Diffusion (Color, Grayscale, UV), Emboss With Gloss, Uber Emboss, Extend Shape, FXAA, Reaction Diffusion, Summed Area Table, Vector Morph
- Nearest sampling option in warps (Multi and Non Uniform Directional Warp are done)
- Shadows (Glow, Mosaic, Shape Drop Shadow, Shape Glow and Shape Stroke are done)
- Flood Fill family: Flood Fill Mapper, to BBox Size, to Index, to Position
- Ray-traced nodes: AO (RTAO), Shadows, Irradiance, Bent Normal; HBAO as a separate AO node; 3D texture nodes (Position, SDF, Surface/Volume Render, Offset)

### Normal map tools
- Bent Normal, Facing Normal, Height Normal Blender, Height to Normal (World Units), Normal Sobel, Normal Transform (done: Normal Invert, Normalize, Vector Rotation, Normal to Height via FFT, Sobel)

### Transforms
- Auto Crop, Cartesian <-> Polar, Clone, Noise Upscale 1-3, Safe Transform, Non-Square Transform, Non-Uniform Rotation, Symmetry Slice (Skew and Symmetry are done), Trapezoid Transform, Quad Transform

### Material filters
- Material blending: Material Blend, Color Blend, Adjustment Blend, Switch, Multi-Material Blend, Material Height Blend, Material Transform
- Effects: Snow Cover, Season Filter, Water Level
- PBR utilities: BaseColor/Metallic/Roughness converter, PBR validators (albedo safe color, BaseColor/Metallic validate), Dielectric F0, Metal Reflectance, PBR Render
- Scan processing: AO Cancellation, Atlas Scatter / Splitter, Clone Patch, Color Equalizer, Crop, Multi-Angle to Albedo/Normal, Smart Auto Tile
- 1-click Bitmap to Material

### Mesh-based generators (driven by baked mesh maps)
- Mask generators: Bottom to Top, Cloth Wear, Dirt, Dripping Rust, Dust, Edge Blur, Edge Damages, Edge Dirt, Edge Notch, Edge Select, Edge Speckle, Edge Wear, Fiber Glass Edge Wear, Grease, Ground Dirt, Leaks, Leather Wear, Light, Mask Builder, Metal Edge Wear, Paint Wear, Selective Dirt, Sun Bleach, Surface Brush
- Weathering: Cracks, Fabric, Leather, Metal, Moss, Rock, Rust
- Utilities: Tri-Planar, 3D Planar Projection, Mesh Data Combiner, Material Selector
- Our Dirt / Dust / Edge Wear nodes work from a height map only, and baked maps are not yet usable as graph inputs

### Splines and paths (Designer 13)
- Create: Spline Cubic, Poly Quadratic, Circle, Point List, Spline Bridge; Assemble: Merge List, Append, Select; Modify: 2D Transform, Warp, Sample Thickness, Sample Height
- Render: Scatter on Spline, Spline Fill, Spline Mapper, Bridge Mapper, Flow Mapper, UV Mapper, Spline Render
- Paths: Mask to Paths, Paths to Splines, Path 2D Transform, Paths Warp, Paths Polygon, Paths Select, Preview Paths, Vertex Processor, Quad Transform on Path

### Graph system
- Function graphs: variables, math, vector, conditionals, loops, Get/Set nodes, sampler nodes with border modes, function-driven parameters
- FX-Map: Quadrant, Iterate and Switch nodes, rule-based recursion (and the Hemisphere morphlet)
- Pixel Processor driven by function graphs (ours uses an expression language)
- Portal node, Pin node, Input Value nodes, non-square graphs, graph instances and dependency management, parameter presets, randomized variations, graph templates
- Tiling modes (none / horizontal only / vertical only) and 16f / 32f float output formats (we have 8 and 16-bit integer)

### 3D view and rendering
- HDRI / image-based lighting and the HDRI tool nodes (Blackbody, Exposure, HDR Merge, Physical Sun/Sky, area lights, Panorama tools)
- Path-traced renderer, post-processing (bloom, DOF, SSAO, tone-mapping choices), camera and light presets, turntable
- Custom mesh and 3D scene loading, several meshes at once

### Import, export, color, automation
- SVG, font and text nodes; JPEG, TIFF, EXR, HDR bitmap import; AxF import
- OCIO / Adobe Color Engine color management; EXR export
- `.sbs` / `.sbsar` compatibility, Substance Source / Assets library, Python scripting API and batch automation, engine output templates (Unreal / Unity packing), UDIM, texture atlases
- Editor extras: dependency / resource manager, user library folders, navigator minimap, customizable shortcuts, language packs

### Engine
- Designer's engine cooks on the GPU; ours is multi-threaded CPU. Biggest long-term speed gap

---

## 3. Suggested order

1. Function graphs + FX-Map (unlocks the most missing nodes)
2. Spline / path tools
3. HDRI image-based lighting and custom mesh loading
4. More nodes and a built-in material library
5. Color management, EXR/JPEG/TIFF import, HDR export
6. Scripting/batch API and engine output templates

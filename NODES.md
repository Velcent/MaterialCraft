# NODES.md: Substance 3D Designer node properties vs MaterialCraft

Research only, no code changed. For every MaterialCraft node type this lists the properties Adobe documents for the matching Substance 3D Designer node, what our Properties panel offers today, and what is missing.

## How this was made, and its limits
- Source: Adobe's official node reference (experienceleague.adobe.com, formerly helpx.adobe.com), one page per node, fetched through a summarising reader. Names, ranges and option lists are reliable. Wording is paraphrased.
- **Adobe's pages almost never state defaults or data types.** Where a range is given it is shown, otherwise `n/s` (not stated). Defaults still have to be checked in Designer itself.
- Screenshots could not be read, so anything only visible in the UI (property order, grouping, slider soft limits) is unconfirmed.
- Some pages were missing or ambiguous. Those cases are listed per node and in the "Not in Designer / not found" section.
- "Ours" lists the labels in our Properties panel today (`src/gui_params.rs`). Every node also has our per-node Output Size.

## Implementation status (updated Oct 2026)
The sections below are the research snapshot. Items marked here are **done in code**; everything else listed below is still open. New properties default to the old behaviour, so existing `.mcg` graphs render the same.

**Phase 1 (done):** Invert toggle · Threshold Mode (greater / greater or equal / lower / lower or equal) · Clamp Apply to alpha · Blend: Add sub and Switch modes, Alpha blending (4 modes), Cropping area · Mirror: Mode (axis X / Y / both / corner), axis offsets, invert axis, corner type · Symmetry: diagonal right, eight-fold, Transfer mode · Skew Align · Normal: Fill alpha with input · Normal Blend: Opacity + Mask input · Normal Combine Technique · Normal Invert blue/alpha · Normal Vector Rotation: Normal format + Rotation map input · Normal to Height: Relief balance, Global opacity · Vector Warp format · Multi Directional Warp: Directions 1/2/4 and Mode (average / max / min / chain) · Directional Warp: Intensity map offset + Input filtering · Warp: Input filtering · Glow: Clear amount + colour · Shape Glow / Shape Stroke: Opacity, Spread (glow), colour · Shape Drop Shadow: Spread · Bevel: Corner type + Normal format · Make It Tile Photo: separate H/V mask size.

**Phase 2 (done):** the shared noise block (Disorder speed, Disorder anisotropy + angle, Tile offset X/Y) on Perlin, Clouds, Fractal Noise, Gaussian Noise, BnW Spots, Gaussian Spots, Directional Noise and Cells (Fractal Sum and Waveform: Disorder + speed); White Noise: Noise distribution.

**Phase 3 (done):** Tile Sampler, Tile Generator, Splatter and Splatter Circular rebuilt in `src/scatter.rs`. All of Designer's 15 built-in patterns plus Pattern Specific (and random), up to six pattern inputs with Random / By pattern number / By ring number distribution, Pattern rotation 0/90/180/270, Symmetry random, Size mode (normal / keep ratio / absolute), Size Y/X ratio, Scale (+random, seed), Offset type (odd / even rows or columns), Offset random (+seed), Global offset, Position random X/Y, Luminance by number / by scale, Mask map invert, Random mask, Checker / Horizontal / Vertical mask, Invert mask, Blending (Max / Add / Alpha blend / Add-Sub), Global opacity, Background colour or input, Reverse rendering order, map inputs (Scale, Rotation, Displacement, Color, Distribution, Background, Mask) with multipliers. Splatter: Grid number + Disorder, Rotation, Pan X/Y, Gain, Output min/max, pattern aspect, blending, background. Splatter Circular: ring amount, ring radius multiplier, amount random (+min), radius random, angle random, spiral factor, spread, scale by pattern / ring number (+invert), centre orientation, ring rotation (+random, offset), colour, luminance random / by number / by ring, random mask, blending, background. Not done: Non-square expansion, Pixel size mode, Vector map inputs, Quincunx flip, Pattern Input Filtering, Pivot options, Splatter Zoom / Disorder angle, Connect patterns, Colour (RGB) versions of the nodes.

**Phase 4 (done):** the four nodes that behaved differently now follow Designer, and the old behaviour stays selectable. Radial Blur: spin blur with Samples (1..128) and Angle (0..0.5), centre; the blur fades out towards the image border so the result tiles (legacy "Zoom" mode kept). Edge Detect: Edge Width (1..16, pixels at 512), Edge Roundness (0..16), Invert, Tolerance (legacy Sobel mode kept). Mosaic: Mosaic Map input (each region of equal map value takes the input's mean colour) and Intensity; without a map it is still the pixel grid. Curvature: reads a normal map with Intensity and Normal Format (legacy height-map mode kept). Not done: Mosaic "Samples" (Designer does not document what it samples), Curvature Sobel variant.

**Phase 5 (done):** Voronoi: Style (F1, random colour, edge, F2, F2-F1, F1*F2, F1/F2), Distance mode (Euclidean, Manhattan, Chebyshev, Minkowski + number), Distance scale, Edge thickness, Rounded curve, Distortion (intensity, scale), Size Y/X, Offset X/Y, Random colour seed (global / manual), Invert. New node **Voronoi Fractal** (Min/Max level, Roughness, Lacunarity, Global opacity, Blend Add / Max / Min). Cells: Luminance random, Edge width, Hardness, Color source (random / pseudorandom + seed / image input) as a new `color` input. Grunge: Balance on every type, Concrete (Large variation, Base noisiness, Dirt specks opacity, Invert dirt, Scratches opacity, Sharpen), Scratches (Quantity, Width, Length), Spots (Details, Coverage, Coverage contrast). Not done: the 15 Grunge Map 001-015 images, Scratches Rough / Fine / Dirty and Spots Dirty as separate nodes, Cells Pattern / Pattern size / Angle, Voronoi per-axis Disorder and 3D size.

**Honest limits.** Adobe documents what each property is called and its range, but not the formulas. Where a formula is undocumented the behaviour is our interpretation, not a numerical match: Relief balance, Normal Combine "channel mixer" and "whiteout", Spread (glow/shadow), Pattern Specific, Horizontal / Vertical mask and the shapes of the built-in patterns (Thorn, Waves, Ridged Bell, Crescent, Capsule), Disorder / anisotropy (a smooth displacement field), Tile offset (whole part = another tile of the plane, fraction = pan), Directions for Multi Directional Warp. Matching Designer pixel for pixel needs side-by-side renders from Designer. **Not done on purpose:** Non-square expansion (we have no non-square graphs yet), Sharpen "Punchthrough alpha" (our Sharpen already leaves alpha untouched), Output Format / Pixel ratio / Tiling mode base parameters.

## Local Designer reference material (found Oct 2026)
The installed Designer (`C:\Program Files\Adobe\Adobe Substance 3D Designer`) ships material that settles most of the "undocumented" points above. It is Adobe's software and content: use it locally as a reference, never copy it into this repository.
- `resources\packages\*.sbs` (about 330 files): every library node as readable XML. Each lists its exact parameters (identifier, type, default, min/max, widget) and, for non-atomic nodes, the full graph of atomic nodes that implements it (Edge Detect = levels, gradient, distance, blend, blur; Grunge Map 001 = blends, levels, warps, transformations). Defaults found so far: Edge Detect width 2, roundness 4, tolerance 0; Splatter has 20 parameters including Zoom, Gain, Pan, Grid Number, Disorder Angle/Random and Output Min/Max.
- `resources\documentation\sbs_reference.md`: the .sbs format reference (needed for opening Designer files). `resources\documentation\pythonapi\html`: the Python API docs. `resources\python`: the `sd` API and samples (can dump every atomic node's properties from inside Designer).
- `sbscooker.exe` and `sbsrender.exe`: command-line cook and render. With them every library node can be rendered to PNG for pixel comparisons against our output.
- `resources\templates`: output templates for the export presets.

## 0. Common to (almost) every Designer node: Base parameters
| Designer | Ours |
|---|---|
| Output Size (Absolute / Relative to Parent / Relative to Input, power-of-two modifier -12..+12, width/height lock) | Yes (Relative to Input / Relative to Parent / Absolute, -12..+12). No width/height lock, square only |
| Output Format (8 bit, 16 bit float, 32 bit float; per node or inherited) | **Missing** (engine is f32, only graph-level export depth) |
| Pixel Size, Pixel Ratio | **Missing** |
| Tiling Mode (None / Horizontal / Vertical / Both; matters for Transformation 2D etc.) | **Missing** (we always tile) |
| Random Seed (inheritable, "new random value" button) | Partly: each random node has its own `seed` |
| Inheritance dropdown per base parameter | Output Size only |

## 0b. Common "noise" block (most Designer noises and spots)
Scale (int) | Disorder (float) | Disorder speed | Disorder anisotropy | Disorder anisotropy angle | Tile offset (float2) | Non-square expansion (bool).
Ours has Scale, Disorder (named `Disorder` on a few nodes), Seed, Contrast, Invert. **Missing everywhere:** Disorder speed, Disorder anisotropy (+ angle), Tile offset, Non-square expansion. Designer noises have no Contrast/Invert/Seed in their panel (they use the Random Seed base parameter); ours are extras that can stay.

---
## 1. Generators: noises

### white_noise: White Noise
Designer: Noise distribution (Uniform / Gaussian / Triangle), Disorder, Disorder speed.
Ours: seed. **Missing:** Noise distribution, Disorder, Disorder speed.

### gaussian_noise: Gaussian Noise
Designer: common noise block (Scale, Disorder, Disorder speed, anisotropy, anisotropy angle, Tile offset, Non-square expansion).
Ours: Scale (grain), seed, contrast, invert. **Missing:** Disorder (+speed, anisotropy, angle), Tile offset, Non-square expansion.

### perlin: Perlin Noise
Designer: Scale, Disorder, Disorder speed, Tile offset, Non-square expansion (no anisotropy).
Ours: Scale, disorder, seed, contrast, invert. **Missing:** Disorder speed, Tile offset, Non-square expansion.

### fbm: Fractal Noise
Designer: **no documentation page found** (not in the node library index; closest are Fractal Sum Base, 3D Perlin Noise Fractal, 3D Ridged Noise Fractal). Keep ours as is. Compare with fractal_sum below.
Ours: Scale, Octaves, Gain, disorder, seed, Mode (fBm / Ridged / Turbulence), contrast, invert.

### clouds: Clouds 1 / 2 / 3 (three separate nodes)
Designer: common noise block (all three identical in the panel).
Ours: one node: Scale, Octaves, Gain, disorder, seed, contrast, invert. **Missing:** Disorder speed/anisotropy/angle, Tile offset, Non-square expansion, the three variants (1, 2, 3).

### plasma: Plasma
Designer: Scale (1..128), Disorder (0..1), Non Square Expansion. (Page is old-format; summary partly unreliable.)
Ours: Scale (1..8), disorder, seed, contrast, invert. **Missing:** Non-square expansion; Scale range is 1..8 vs Designer 1..128.

### voronoi: Voronoi (and Voronoi Fractal)
Designer (documented as the 3D Voronoi, GPU engine): Invert, Scale (float), Size (float3), Offset (float3), Disorder (float3), Distortion Intensity, Distortion Scale Multiplier, Rounded Curve, Distance Scale, Distance Mode (Euclidean / Manhattan / Chebyshev / Minkowski), Minkowski Number, Style (F1, F2, F2-F1, F1*F2, F1/F2, Edge, Random color), Edge Thickness, Random Color Seed Mode (Global / Manual), Random Color Seed, Non Square Expansion.
Voronoi Fractal adds: Min Level, Max Level, Roughness (Add mode only), Lacunarity, Global Opacity, Blend Mode (Add / Max / Min), Enable Tiling.
Ours: Scale, Jitter, seed, Output (Distance / Cell ID / Edges / Second distance).
**Missing:** nearly all of it: Invert, Size/Offset/Disorder per axis, Distortion (intensity, scale), Rounded Curve, Distance Scale, Distance Mode + Minkowski Number, Style (F1*F2, F1/F2, F2-F1, Random color), Edge Thickness, Random Color seed options, Non-square expansion, whole Voronoi Fractal node.

### crystal: Crystal 1 / 2
Designer: Scale (Crystal 1: 1..256, Crystal 2: 1..16, mapping unverified), Disorder (0..1), Non Square Expansion.
Ours: Type (Crystal 1/2), Scale, disorder, seed, contrast, invert. **Missing:** Non-square expansion; Scale range (ours up to 64).

### bnw_spots: BnW Spots 1 / 2 / 3
Designer: common noise block. BnW Spots 1 also has **Roughness** (balances octaves).
Ours: Type (1/2/3), Scale, disorder, seed, contrast, invert. **Missing:** Disorder speed/anisotropy/angle, Tile offset, Non-square expansion, Roughness (variant 1).

### gaussian_spots: Gaussian Spots 1 / 2
Designer: common noise block.
Ours: Scale, Size, Size variation, seed, contrast, invert. **Missing:** Disorder (+speed, anisotropy, angle), Tile offset, Non-square expansion, the 1/2 variants. (Our Size/Size variation are extras.)

### cells: Cells 1 / 2 / 3 / 4 (page-to-variant mapping unverified)
- Cells 1: Scale, Disorder (+speed, anisotropy, angle), Pattern (enum, options n/s), Pattern size (float2), Pattern scale, Luminance random, Angle, Angle random, Tile offset, Non-square expansion.
- Cells 2: Scale, Edge width, Invert, Disorder, Disorder speed, Non-square expansion.
- Cells 3: Scale, Hardness, Invert, Disorder (+speed, anisotropy, angle), Pattern size (float2), Pattern scale, Angle, Angle random, Tile offset, Non-square expansion.
- Cells 4: input Grayscale; Scale, Disorder, Disorder speed, Color source (Random / Pseudorandom / Image input), Pseudorandom seed, Non-square expansion.
Ours: Type (cells1-4), Scale, Randomness, seed, contrast, invert. **Missing:** all the per-variant parameters above (Edge width, Hardness, Pattern/Pattern size/scale, Luminance random, Angle, Color source and the image input for Cells 4, disorder block).

### fractal_sum: Fractal Sum Base / Fractal Sum 1-4
Designer Base: Roughness, Min level (int), Max level (int), Disorder, Disorder speed, Contrast, Global opacity, Non-square expansion. Fractal Sum 1-4: only Disorder, Disorder speed, Non-square expansion each.
Ours: Type (base/turbulence/creased), Scale, Min level, Max level, Roughness, seed, contrast, invert. **Missing:** Disorder + speed, Global opacity, Non-square expansion, Fractal Sum 1-4 variants.

### directional_noise: Directional Noise 1 / 2
Designer: Scale, Disorder (+speed, anisotropy, angle), Angle, Angle random, Tile offset, Non-square expansion.
Ours: Scale, Stretch, Angle, seed, contrast, invert. **Missing:** Angle random, Disorder block, Tile offset, Non-square expansion, variants 1/2. (Our Stretch has no Designer equivalent.)

### grunge: Grunge Map 001-015, Grunge Concrete, Grunge Scratches (Rough / Fine / Dirty), Grunge Spots (/ Dirty) (all separate nodes)
Designer Grunge Map 001-015 (each): Balance, Contrast, Invert, Brush Pattern, Non Square Expansion. 005 adds Disorder; 010 and 011 add Cracks.
Advanced nodes: Concrete (Balance, Contrast, Invert, Non-square + Base Noisiness, Dirt Specks Opacity, Invert Dirt, Scratches Opacity, Sharpen, Large Variation Intensity); Scratches Rough (+ Scratch Quantity, Tiling, Blur, Width, Length, Masking, Dirtiness, Double Scratch, Spots Intensity/Tiling, Dust Intensity/Tiling, Sharpen); Scratches Fine (+ Scratches Amount, Sharpen Intensity, Scratch Value Bias); Scratches Dirty (+ Base Grunge Intensity, Scratches Intensity); Spots (+ Details, Coverage, Coverage Contrast); Spots Dirty (+ Coverage, Scale).
Ours: Type (Concrete / Scratches / Spots / Smudges), disorder, seed, contrast, invert.
**Missing:** Balance, Brush Pattern, Cracks, Non-square expansion, all 15 Grunge Map images, and every Advanced parameter of Concrete / Scratches / Spots.

### waveform: Waveform 1 (2 and 3 not checked)
Designer: Samples (int), Function (Value noise / Cosine / Custom function), Custom function (function graph), Roughness, Scale, Amplitude min, Amplitude max, Noise, Position (Centered / Bottom), Pattern (enum), Pattern variation, Disorder, Disorder speed.
Ours: Waveform (sine/triangle/saw/square), Count, Angle, Warp, Warp scale, seed, contrast, invert. **Missing:** Samples, Function options, Custom function (needs function graphs), Roughness, Amplitude min/max, Noise, Position, Pattern + variation, Disorder (+speed). (Our Angle/Warp have no Designer equivalent.)

---
## 2. Generators: shapes, patterns, gradients

### color: Uniform Color
Designer: Color mode (grayscale / color), Output color (float / float4, alpha = opacity in color mode).
Ours: (r g b a in the file; panel has a colour picker). **Missing:** Color mode switch (grayscale output) is not exposed as a toggle.

### shape / circle / rect (polygon, star): Shape
Designer Shape: Tiling (1..16), Pattern (Square, Disc, Paraboloid, Bell, Gaussian, Thorn, Pyramid, Brick, Gradation, Waves, Half Bell, Ridged Bell, Crescent, Capsule, Cone, Hemisphere), Pattern Specific (0..1), Scale (0..1), Size (X/Y), Angle, Rotation 45°, Non Square Expansion, Non Square Tiling.
Ours `shape`: Shape (Square, Disc, Paraboloid, Gaussian, Diamond, Pyramid, Half sphere, Cone), Width, Height, Rotation, Position X, Position Y, Tiling, Edge softness.
**Missing:** patterns Bell, Thorn, Brick, Gradation, Waves, Ridged Bell, Crescent, Capsule; Pattern Specific; Scale; Rotation 45°; Non-square expansion/tiling. (Position X/Y and Edge softness are ours.)
Ours `circle` (Radius, Edge softness) and `rect` (Width, Height, Edge softness) correspond to Shape's Disc/Square patterns; Designer has no separate Circle/Rectangle node.

### polygon: Polygon 1 / Polygon 2
- Polygon 1: Sides (3..32), Explode, Triangle Size, Scale, Auto Scale, Rotation, Gradient (bool), Gradient Invert, Tiling (1..16), Non Square Expansion, Non Square Tiling.
- Polygon 2: Sides (3..32), Scale, Rotation, Curve (-1..1), Gradient (0..1), Invert Gradient, Auto Scale, Non Square Expansion.
Ours: Sides (3..12), Radius, Edge softness, Rotation. **Missing:** both variants' extras (Explode, Triangle Size, Gradient, Curve, Auto Scale, Tiling, Non-square), Sides up to 32.

### star: Star 1 / Star 2
Designer: **no documentation page found** (patterns/star-1, star-2 failed). Parameters unknown; to be checked in Designer.
Ours: Points, Inner radius, Size, Edge softness, Rotation.

### checker: Checker 1 / Checker 2
Checker 1: Tiling (1..16), Rotate 45 Degrees, Non Square Expansion. Checker 2: **no page found**.
Ours: Tiles (1..64). **Missing:** Rotate 45°, Non-square expansion, Checker 2.

### bricks: Brick 1 / Brick 2 (plus Brick Generator and Tile Brick, not documented here)
Designer: Tiling (1..16), Edge Smoothness (0..1), Interstice Width (0..1), Non Square Expansion. Both pages point to the Brick Generator / Tile Generator for more options.
Ours: Rows, Columns, Mortar, Bevel, Row offset, Brick variation, seed. **Missing:** Edge Smoothness (we have Bevel), Non-square expansion, the separate Brick 1/2 look. Ours already exceeds Brick 1/2 in options.

### stripes: Stripes
Designer: Stripes (1..100), Width (0..1), Softness (0..1), Shift (0..20), Align (Edges / Center), Filtering (bool), Non Square Expansion. ("Lines" page not found.)
Ours: Count, Angle, Edge softness, Stripe width, Angle mode. **Missing:** Shift, Align, Filtering, Non-square expansion. (Our free Angle goes beyond Designer.)

### weave: Weave 1 / Weave 2 / Weave Generator
- Weave 1: Tiling (1..16), Rotate 45°, Non Square Expansion.
- Weave 2: Tiling (1..16), Disorder (0..100), Rotate 45°, Non Square Expansion.
- Weave Generator: Tile X (1..20), Tile Y (1..20), Shape (0..1), Weave (1..10 stitches), Gap (0..1), Non Square Expansion.
Ours: Count, Thread width. **Missing:** Disorder, Rotate 45°, separate X/Y tiling, Shape, Weave count (stitches per block), Gap, Non-square expansion.

### triangle_grid: Triangle Grid
Designer (inputs: Height, Vector map, Color input): Color Output (Per Vertex, Per Triangle, Thin Line, Distance to Edge, Center), Triangulation (Auto, 45°, -45°, Quincux horizontal, Quincux vertical), X Amount, Y Amount, Random Position Multiplier, Random Position (float2), Vector Map Displacement, Quincux Offset X/Y, Rotation, Rotation Disorder, Height Input Multiplier, Height Random, Blend Mode (Min, Max, Depth Test, Alpha Blend), Color Source (Height, Random, Color Input), Color Source Opacity, Distance to Edge Thickness, Line Color, Background Color, Random Color Seed Mode (Global / Manual), Random Color Seed, Non Square Expansion.
Ours: Count, Border, Variation, seed, contrast, invert. **Missing:** essentially everything (we render a simple grid): outputs, triangulation, X/Y amounts, positions, height/vector inputs, colours, blend, seeds.

### starburst: Starburst
Designer: Branches (3..512), Blur Amount (0..32), Randomness (0..1), Size Min (0..1), Size Max (0..2), Size Variation (0..1), Tiling (1..16).
Ours: Count, Rays, Sharpness, Size, Rotation, Rotation random, seed, contrast, invert. **Missing:** Blur Amount, Size Min/Max/Variation, Tiling. (Different parameterisation; branches ≈ our Rays.)

### tile_sampler: Tile Sampler (Gray / Color)
Designer has about 60 properties plus 9 map inputs (Pattern Input 1-6, Scale Map, Displacement Map, Rotation Map, Vector Map, Color Map, Mask Map, Pattern Distribution Map, Background):
- Ungrouped: X Amount (0..64), Y Amount (0..64), Non Square Expansion.
- Pattern: Pattern (Pattern Input + the 15 shapes), Pattern Input Number (1..6), Pattern Input Distribution (Random / Pattern Number / Distribution Map), Pattern Input Filtering, Pattern Specific, Pattern Specific Random, Rotation (0/90/180/270), Rotation Random, Symmetry Random, Symmetry Random Mode.
- Size: Size Mode (Normal / Keep Ratio / Absolute / Pixel), Size (x2), Size Random, Scale (0..10), Scale Random, Scale Map Multiplier, Scale Vector Map Multiplier, Scale Parametrization Affect.
- Position: Position Random (x2, 0..10), Offset, Offset Type (Horizontal/Vertical quincux or global), Global Offset (x2), Displacement Map Intensity, Displacement Angle, Vector Map Displacement.
- Rotation: Rotation, Rotation Random, Rotation Map Multiplier, Vector Map Multiplier.
- Color: Mask Map Threshold, Mask Map Invert, Mask Map Sampling Technique, Mask Random, Invert Mask, Blending Mode (Add/Sub, Max, Alpha Blend), Color, Color/Luminance Random, Color Parametrization Mode, Multiplier, Affect, Global Opacity, Background Color, Reverse Rendering Order.
Ours: X amount, Y amount, Size, Size random, Rotation, Rotation random, Position random, Color, Color random, Mask map threshold, seed; inputs: pattern, mask.
**Missing:** about 45 properties and 7 of 9 map inputs: built-in Pattern choice, multi-pattern inputs, Pattern Specific, Symmetry Random, Size Mode, separate X/Y size, Scale vs Size, Scale/Displacement/Rotation/Vector/Color/Distribution maps, Offset + Offset Type, Global Offset, Mask Random/Invert/Sampling, Blending Mode, Color Parametrization, Global Opacity, Background Color, Reverse Rendering Order, Non-square expansion.

### tile_generator: Tile Generator
Designer: inputs Pattern Input 1-6, Background. Properties: X/Y Amount (1..64), Non Square Expansion; Pattern (Image Input + shapes), Pattern Input Number/Distribution (Random / By Pattern Number), Pattern Specific, Image Input Filtering, Rotation (0/90/180/270), Rotation Random, Quincunx Flip, Symmetry Random (+Mode); Size Mode (Normal-Interstice, Normal-Size, Keep Ratio, Absolute, Pixel), Middle Size, Interstice X/Y, Size (x2), Scale (0..2), Scale Random, Scale Random Seed; Offset, Offset Random, Offset Random Seed, Vertical Offset, Position Random (x2), Global Offset (x2); Rotation, Rotation Random; Color, Luminance/Color Random, Luminance By Number, Luminance By Scale, Checker/Horizontal/Vertical/Random Mask, Invert Mask, Blending Mode (Add, Max, Add Sub), Background Color, Global Opacity, Reverse Rendering Order.
Ours: X amount, Y amount, Size, Row offset, Rotation, Rotation random, Size random, Position random, Color, Color random, Mask map threshold, seed.
**Missing:** about 35 properties: Pattern choices, Interstice/Size Mode, Middle Size, Offset Random, Vertical Offset, Global Offset, Quincunx Flip, Symmetry Random, the mask toggles, Luminance By Number/Scale, Blending Mode, Background Color, Global Opacity, Reverse Rendering Order, Scale seeds.

### splatter: Splatter
Designer (one map input): Pattern Size Width/Height (0..1000), Rotation (-360..360), Rotation Variation, Zoom (100..10000), Gain, Pan X/Y, Disorder (0..100), Grid Number (0..8), Disorder Angle, Disorder Random, Pattern Size (5..12), Size Variation, Image Input Filtering, Output Level Min/Max, Background Color, Luminance Variation (gray) / Color Variation (color).
Ours: Count, Size, Size random, Rotation random, Color random, seed. **Missing:** Pattern Size W/H, Rotation (global), Zoom, Gain, Pan X/Y, Disorder + Angle + Random, Grid Number, Output Level Min/Max, Background Color, Filtering.

### splatter_circular: Splatter Circular
Designer: Pattern Amount (1..64), Pattern Amount Random, Pattern Amount Random Min, Ring Amount (1..10), Non Square Expansion; Pattern (Image Input + shapes), Pattern Input Number/Distribution (Random / By Pattern Number / By Ring Number), Image Input Filtering, Pattern Specific, Symmetry Random (+Mode); Radius, Radius Random, Ring Radius Multiplier, Angle Random, Spiral Factor, Spread, Offset along Direction, Global Offset; Connect Patterns, Size (Connected), Size Random, Scale, Scale Random, Scale by Pattern Number, Invert Pattern Number, Scale by Ring Number, Invert Ring Number; Pattern Rotation (+Random), Pattern Rotation Pivot (Center, Min X, Max X, Min Y, Max Y), Center Orientation, Ring Rotation (+Random, Offset); Color, Luminance Random, Luminance By Scale / Pattern Number / Ring Number (+Invert), Random Mask, Background Color, Blending Mode, Global Opacity.
Ours: Number, Radius, Size, Size random, Rotation, Rotation random, Position random, seed. **Missing:** about 40 properties: rings, spiral, spread, pattern choices, by-number/ring scaling and luminance, ring rotation, pivot, colour group, blending.

### gradient: Gradient Linear 1 / 2 / 3
Designer: Tiling (1..16), Rotation (0/90/180/270; 2 and 3: 0/90), Position (0..1, Linear 3 only).
Ours: Type (Linear / Radial), Angle (free, 0..1 turn). **Missing:** Tiling, Position (Linear 3).

### gradient_axial: Gradient Axial
Designer: Point 1, Point 2 (2D points), Non Square Expansion. Ours: Angle. **Missing:** Point 1/2 (draggable in the 2D view in Designer), Non-square expansion.

### gradient_circular: Gradient Radial / Gradient Circular
Gradient Radial: Shape (Cone / Hemisphere), Point 1 (centre), Point 2 (radius point), Non Square Expansion. Gradient Circular (angular sweep): Point 1, Point 2, Non Square Expansion. (Gradient Angular: no page.)
Ours: Offset. **Missing:** Shape (Cone/Hemisphere), Point 1/2, Non-square expansion; radial mode currently lives under `gradient`.

### gradient_reflected: no Designer node of that name
Closest: Gradient Linear 3 (Position gives a peak). Keep ours: Count, Angle, contrast, invert.

### pixel_processor: Pixel Processor
Designer: Color mode (bool), Per pixel function (function graph; `$pos`, input sampling by index). Inputs: Input image #.
Ours: expr / expr_g / expr_b / expr_a strings. **Missing:** a function-graph editor (roadmap item), per-input sampling nodes.

### bitmap: Bitmap
Designer: Color mode, PKG resource path, Resize method (Smooth stretch / Nearest stretch). Output Size is Absolute by default.
Ours: file. **Missing:** Color/grayscale toggle, Resize method, SVG/other formats.

---
## 3. Filters: blur, sharpen, effects

### blur: Blur (atomic "Box Blur")
Designer: Intensity (unbounded). Ours: Intensity. **Complete.**

### blur_hq: Blur HQ Grayscale / Color
Designer: Intensity (0..16), Quality (0..1). Ours: Intensity, Quality. **Complete** (grayscale and colour share one node).

### directional_blur: Directional Blur
Designer: Intensity (pixels), Angle (turns). Ours: Angle, Length. **Complete** (our "Length" = Intensity).

### non_uniform_blur: Non-Uniform Blur Grayscale / Color
Designer: Blur Map input; Intensity (0..50), Anisotropy (0..1), Asymmetry (0..1), Angle (0..1), Samples (1..16), Blades (1..9).
Ours: Intensity. **Missing:** Anisotropy, Asymmetry, Angle, Samples, Blades.

### radial_blur: Radial Blur
Designer: Samples (1..128), Angle (0..0.5, spin amount), Center Position.
Ours: Amount, Radius, Center X, Center Y. **Missing:** Samples. (Ours is a zoom blur with Radius; Designer's is a spin blur with Angle, so the behaviour differs and should be reworked to match.)

### anisotropic_blur: Anisotropic Blur (Color / Grayscale)
Designer: Intensity (0..16), Anisotropy (0..1), Angle (0..1), Quality (0..1).
Ours: Intensity, Anisotropy, Angle. **Missing:** Quality.

### slope_blur: Slope Blur (Color / Grayscale)
Designer: Slope input; Samples (0..32), Intensity (0..16), Mode (Blur / Min / Max).
Ours: Samples, Intensity, Mode. **Complete.**

### sharpen: Sharpen
Designer: Intensity, Punchthrough Alpha (colour inputs). Ours: Radius, Amount. **Missing:** Punchthrough Alpha (Designer has no Radius; ours is an extra).

### highpass: High Pass (Color / Grayscale)
Designer: Radius (0..64). Ours: Radius. **Complete.** (Luminance Highpass is a separate Designer node, not implemented.)

### glow: Glow (Color / Grayscale)
Designer: Glow Amount (0..1), Clear Amount (0..1), Glow Size (0..20), Glow Color (colour version).
Ours: Width, Intensity. **Missing:** Clear Amount, Glow Color, colour variant.

### shape_glow: Shape Glow
Designer: Mode (Soft / Precise), Width (-1..1), Spread (0..1), Opacity (0..1), (Shadow) Color, Mask Color (gray), Input Is Pre-Multiplied, Pre-Multiply Output (colour).
Ours: Width, Softness, Intensity, Shape threshold. **Missing:** Mode, Spread, Colour, Mask Color, pre-multiply options; Width should accept negatives.

### shape_stroke: Shape Stroke
Designer: Width (-1..1), Opacity, (Outline) Color, Mask Color, Input Is Pre-Multiplied, Pre-Multiply Output.
Ours: Width, Offset, Softness, Shape threshold. **Missing:** Opacity, Colour, Mask Color, pre-multiply, negative width.

### shape_drop_shadow: Shape Drop Shadow
Designer: Angle (0..1), Distance (-0.5..0.5), Size (0..1), Spread (0..1), Opacity, Shadow Color, Mask Color, pre-multiply options.
Ours: Angle, Distance, Blur, Opacity. **Missing:** Spread, Shadow Color, Mask Color, pre-multiply; Distance range.

### bevel: Bevel
Designer: inputs input + Custom Curve; outputs **Heightmap and Normalmap**. Distance (-1..1), Corner Type (Round / Angular), Smoothing (0..5), Use Non-Uniform Blur, Use Custom Curve, Normal Intensity (0..50), Normal Format (DirectX / OpenGL).
Ours: Distance, Smoothing, Shape threshold, Output as normal, Normal intensity. **Missing:** Corner Type, Use Non-Uniform Blur, Custom Curve input, Normal Format, a second output (needs multi-output nodes), negative Distance.

### distance: Distance
Designer: inputs Mask + Source; Color mode, Maximum distance, Combine source/distance (bool), Distance mode (Euclidean / Manhattan / Chebyshev).
Ours: Distance range, Shape threshold, Mode. **Missing:** Source input (colour copied from nearest border), Combine source/distance, Color mode, and Distance mode (Euclidean / Manhattan / Chebyshev). Our Mode (Inside / Outside / Both) has no Designer equivalent.

### edge_detect: Edge Detect
Designer: Edge Width (1..16), Edge Roundness (0..16), Invert, Tolerance (0..1). Ours: Intensity. **Missing:** all four Designer parameters (ours is a simple gradient filter).

### sobel: no node of this name in Designer
Closest: Normal Sobel (Intensity 0..3, Normal Format) and Curvature Sobel (Intensity 0..1, Normal type). Ours: Intensity. Decide whether to keep as extra or replace by the two real nodes.

### emboss: Emboss (atomic)
Designer: inputs Input + Intensity (height); Intensity, Light angle, Highlight color, Shadow color. Ours: Angle, Intensity. **Missing:** Highlight color, Shadow color, separate height input. (Uber Emboss is a separate node, not implemented.)

### mosaic: Mosaic
Designer: inputs Color + Mosaic Map; Samples (0..16), Intensity (0..1). Ours: Cells per side. **Missing:** all of Designer's (it is a map-driven warp mosaic); ours is a pixelate. Behaviour differs.

---
## 4. Filters: colour and values

### levels: Levels
Designer: Level in low / high / mid, Level out low / high (float per channel for colour), Intermediary clamp; header actions Invert, Auto level, histogram/slider UI.
Ours: Auto levels, Input low, Input high, Gamma, Output low, Output high. **Missing:** per-channel (RGBA) values, Intermediary clamp, histogram UI.

### auto_levels: Auto Levels. No parameters in Designer. Ours: Per channel (extra).

### adjust: Contrast/Luminosity (no node named Brightness/Contrast)
Designer: Contrast (-1..1), Luminosity (-1..1). Ours: Brightness, Contrast. Equivalent; consider renaming Brightness to Luminosity and matching the ranges.

### invert: Invert / Invert Grayscale
Designer: Invert (bool toggle). Ours: no parameter. **Missing:** the toggle (trivial).

### threshold: Threshold
Designer: Threshold (0..1), Mode (Greater, Greater or equal, Lower, Lower or equal). Ours: Threshold, Edge softness. **Missing:** Mode.

### histogram_scan: Histogram Scan
Designer: Position, Contrast, Invert Position. Ours: Position, Contrast, Invert. **Complete.** (Designer also has Histogram Select: Position, Range, Contrast; see histogram_range.)

### histogram_range: Histogram Range
Designer: Range, Position. Ours: Position, Range, Contrast, Invert. **Complete** (extras are ours).

### histogram_shift: Histogram Shift. Designer: Position (wraps). Ours: Position. **Complete.**

### quantize: Quantize Color
Designer: outputs Output, ID, Palette, Palette color amount; Max. color amount, Contour smoothing, Dithering, Dithering pattern (Blue noise / Bayer), Ignore alpha, Distance color space (Lab / RGB), ID sort mode (Z-curve / Hue / Representativity), Downscale filtering. (No grayscale Quantize page found.)
Ours: Steps. **Missing:** everything except the idea; multi-output (ID, Palette) not possible yet.

### clamp: Clamp
Designer: Min, Max (0..1), Apply to Alpha (colour). Ours: Low, High. **Missing:** Apply to Alpha.

### pow: Pow. Designer: exponent (0..10). Ours: Exponent. **Complete.**

### hsl: HSL
Designer: Hue, Saturation, Lightness (0.5 neutral). Ours: Hue, Saturation, Lightness. **Complete** (check neutral = 0.5).

### tint: no Designer node
Closest: Blend (Multiply) with a Uniform Color. Ours: Red, Green, Blue. Keep as an extra.

### colorize: Gradient Map
Designer: Color mode, Gradient addressing (Repeat / Clamp), Gradient (keys, Gradient Editor). Gradient Editor: pins with position + colour, add/move/delete, invert positions, clear, invert colours, desaturate, interpolation (Linear, Flat Tangents + Midpoint, Smooth + Smoothness, No Interpolation), colour picker (RGB/HSV/alpha/hex, sRGB toggle), eyedropper from an image line with Precision.
Ours: Interpolation + gradient editor (8 keys: p, r, g, b). **Missing:** Gradient addressing, Color mode, Midpoint/Smoothness, more than 8 keys, alpha per key, eyedropper, copy/paste.

### color_match: Color Match
Designer: inputs Input, Source Color, Target Color; outputs Mask. Source Color Mode (Average / Parameter / Input), Source Color, Target Color Mode (Parameter / Input), Target Color, Custom Color Variation (Hue, Chrominance, Luminance), Use Mask, Mask Mode, Mask smoothing.
Ours: Strength (+ reference input). **Missing:** nearly all.

### channel_mixer: Channel Mixer
Designer: Red / Green / Blue Channel (RGB triple each, -200..200), Monochrome. Ours: 9 coefficients + Monochrome. **Complete** (check ranges ±200).

### color_to_mask: Color to Mask
Designer: Use color input, Color, Threshold, Selection fade, Distance color space (RGB, Linear sRGB, Luminance, Lab, Angle), Distance weights (3 floats). Inputs Color, Color Input.
Ours: Red, Green, Blue, Range, Softness. **Missing:** Use color input (second input), Distance color space, Distance weights.

### replace_color: Replace Color
Designer: Source Color, Target Color. Ours: Source/Target RGB + Range + Softness. **Complete** (extras are ours; Designer's "Replace Color Range" has range).

### chrominance_extract: Chrominance Extract. No parameters. **Complete.**

### grayscale_conversion / to_gray: Grayscale Conversion
Designer: Channel weights (RGBA), Flatten alpha, Background value. Advanced variant: Grayscale Type (Desaturation, Luma, Average, Max, Min).
Ours: grayscale_conversion: Red/Green/Blue weight; to_gray: none. **Missing:** Alpha weight, Flatten alpha, Background value, Advanced variant types.

### channel: Channels Shuffle
Designer: two inputs; Red / Green / Blue / Alpha channel (each picks a source channel of Input 1 or 2; option list undocumented). Ours: Channel (one channel out). **Missing:** two-input shuffle with per-channel source.

### merge: RGBA Merge. No parameters. Ours: none. **Complete.**
### alpha_merge: Alpha Merge. No parameters. Ours: none. **Complete.**

### blend: Blend
Designer: Opacity, Blending mode (12 in the atomic node: Copy, Add (Linear dodge), Subtract, Multiply, Add sub, Max, Min, Switch, Divide, Overlay, Screen, Soft light), Alpha blending (Use source alpha / Ignore alpha / Straight / Premultiplied), Cropping area (float4). Inputs Foreground, Background, Opacity mask.
Ours: Blending mode (20 modes, superset), Opacity. **Missing:** Add sub, Switch, Alpha blending modes, Cropping area.

### switch: Switch
Designer: Switch (bool, two inputs; Multi Switch for many). Ours: Index (4 inputs). Equivalent; add Multi Switch naming if desired.

### flood_fill: Flood Fill
Designer: Safety/Speed trade-off (Simple / Complex / No failure), Advanced options, Override trade-off (-1..100). Ours: Threshold. **Missing:** Safety/Speed, Advanced. (Designer has no threshold: input is already binary.)

### flood_fill_random: Flood Fill to Random Grayscale
Designer page lists no parameters (incomplete). Ours: Minimum, Maximum, seed. Keep.

### flood_fill_color: Flood Fill to Random Color. No parameters. Ours: seed (extra).

### flood_fill_gradient: Flood Fill to Gradient
Designer: Angle, Angle Variation, Multiply by Bounding Box Size, Angle Image Input Multiplier, Slope Image Input Multiplier, Multiply by Slope Intensity, Flat Slope Color; inputs Angle Input, Slope Input.
Ours: Angle, Angle variation, seed. **Missing:** Multiply by Bounding Box Size, the two input multipliers, Multiply by Slope Intensity, Flat Slope Color, Angle/Slope inputs.

---
## 5. Filters: geometry and warps

### transform: Transformation 2D
Designer: Transformation matrix (float4, with 2D-view gizmo), Offset (float2), Mipmap mode (Automatic / Manual), Mipmap level, Matte color, Filtering (enum); Tiling mode comes from the base parameters (None to disable). Rotation lives inside the matrix.
Ours: Offset X/Y, Scale X/Y, Rotation, Tiling mode. **Missing:** matrix editing (shear/symmetry), Mipmap mode + level, Matte color, Filtering, on-image gizmo. Also Safe Transform below.

### safe_transform (not in our list, found incidentally): Tile (1..16), Offset Mode (Manual / Random), Offset, Rotation, Tile Safe Rotation, Symmetry (none, X, Y, X+Y), Background Color, Mipmap Mode, Mipmap Level (0..10).

### mirror: Mirror
Designer: Mode (Mirror Axis X, Mirror Axis Y, Mirror Corner), Axis X Offset, Axis Y Offset, Invert Axis X, Invert Axis Y, Corner Type (Top Left, Top Right, Bottom Left, Bottom Right).
Ours: Axis (x, y, both). **Missing:** Offsets, Invert axis, Corner mode and Corner Type.

### symmetry: Symmetry
Designer: Symmetry Mode (Mirror Y, Mirror X, Diagonal Left, Diagonal Right, Mirror X/Y, Mirror X / Mirror Y, Diagonal Left / Diagonal Right, Diagonal Right / Diagonal Left, 8), Transfer Mode (Copy, Add, Subtract, Multiply, Add Sub, Max, Min).
Ours: Axis (x, y, both, diagonal), Offset, Flip side. **Missing:** most Symmetry Modes (the 4 combined and the 8-fold), Transfer Mode.

### skew: Skew
Designer: Axis (Horizontal / Vertical), Amount (-1..1), Align (Center, Top Left, Bottom Right).
Ours: Skew X, Skew Y (whole numbers). **Missing:** Align; our parameters differ (Designer uses one Axis + Amount).

### swirl: Swirl
Designer: Matrix (position/size via 2D gizmo), Offset (0..1), Amount (-16..16). Ours: Amount, Radius. **Missing:** Matrix, Offset; Amount range.

### warp: Warp (atomic)
Designer: inputs Input + Gradient; Intensity, Input filtering mode (Nearest / Bilinear). Ours: Intensity. **Missing:** Input filtering mode.

### directional_warp: Directional Warp
Designer: Intensity, Warp angle (turns), Input filtering mode, Intensity map offset. Ours: Angle, Intensity. **Missing:** Input filtering mode, Intensity map offset.

### non_uniform_directional_warp: Non-Uniform Directional Warp
Designer: inputs Input, Intensity Input, Warp Angle Input; Intensity (0..20), Warp Angle (0..1), Warp Angle Input Multiplier (0..1), Trail Mode (Min, Max, Average), Trail Length (0..1), Trail Fade (0..1), Trail Curve (-1..1).
Ours: Intensity, Angle. **Missing:** Warp Angle Input Multiplier, Trail Mode/Length/Fade/Curve.

### multi_directional_warp: Multi Directional Warp
Designer: Intensity (0..20), Warp Angle (0..1), Mode (Average, Max, Min, Chain), Directions (1, 2, 4). Ours: Intensity, Directions, Angle. **Missing:** Mode.

### vector_warp: Vector Warp
Designer: Intensity (0..1), Vector Format (DirectX / OpenGL). Ours: Intensity. **Missing:** Vector Format.

### make_it_tile_photo: Make It Tile Photo
Designer: Mask Warping H/V (-100..100), Mask Size H/V (0..1), Mask Precision H/V (0..1). Ours: Blend width. **Missing:** the six H/V parameters (we expose one).

### make_it_tile_patch: Make It Tile Patch
Designer: Mask Size, Mask Precision, Mask Warping, Pattern size width/height (0..1000), Disorder, Size Variation, Octave (0..6), Rotation, Rotation Variation, Background Color, Color Variation (colour) / Luminosity Variation (gray).
Ours: Patches, Softness, seed. **Missing:** about 11 properties.

---
## 6. Material and surface

### normal: Normal (atomic) / Height to Normal World Units / Normal Sobel
Designer Normal: Intensity (values above 100 add little), Normal format (OpenGL / DirectX flip), Alpha channel content (Fill Alpha With Input / Force Alpha To 1).
Height to Normal World Units: Surface Size (cm, 0..1000), Height Depth (cm, 0..100), Normal Format, Sampling (Standard / Sobel). Normal Sobel: Intensity (0..3), Normal Format.
Ours: Intensity, DirectX format. **Missing:** Alpha channel content; the World Units and Sobel-sampling variants.

### normal_blend: Normal Blend
Designer: Opacity (0..1), Use Mask (bool); inputs NormalFG, NormalBG, Mask. Ours: no parameters. **Missing:** Opacity, Use Mask + mask input.

### normal_combine: Normal Combine
Designer: Technique (Whiteout low quality / Channel mixer high quality / Detail oriented high quality). Ours: none (reoriented blend). **Missing:** Technique.

### normal_invert: Normal Invert
Designer: Invert Red, Green, Blue, Alpha. Ours: Invert red (X), Invert green (Y). **Missing:** Invert Blue, Invert Alpha.

### normal_normalize: Normal Normalise. No parameters. **Complete.**

### normal_vector_rotation: Normal Vector Rotation
Designer: Rotation Angle (0..1), Normal Format; inputs Normal, Rotation Map. Ours: Angle. **Missing:** Normal Format, Rotation Map input.

### normal_to_height: Normal to Height (and HQ)
Designer: Relief Balance (0..1), Normal Format, Global Opacity (0..1). HQ: Normal Format, Relief Balance, Height Intensity (0..1), Height Normalize, Quality (Normal / High).
Ours: DirectX format. **Missing:** Relief Balance, Global Opacity, plus HQ's Height Intensity / Height Normalize / Quality.

### ao: Ambient Occlusion (HBAO) / Ambient Occlusion (RTAO)
HBAO: Use World Units, Height Depth (0..1, screen units), Surface Size (0..1000) and Height Scale cm (0..1000) (world units), Radius (0..1), Quality (4 / 8 / 16 samples), GPU Optimization.
RTAO: Use Physical Size, Physical Size (float3), Samples, Height Scale, Distribution, Maximum Distance, Spread Angle.
Ours: Spread, Height depth. **Missing:** Radius, Quality, Use World Units + Surface Size + Height Scale, and the whole RTAO set (Samples, Distribution, Max Distance, Spread Angle).

### curvature: Curvature (Filter Node) / Curvature Sobel
Designer: input is a **normal map**; Intensity (0..10), Normal Format. Sobel: Intensity (0..1), Normal type.
Ours: Radius, Intensity (height-based). **Missing:** Normal Format and normal-map input behaviour (ours reads a height map).

### curvature_smooth: Curvature Smooth
Designer: Normal format; outputs Curvature, Convexity, Concavity. Ours: Smoothing, Intensity. **Missing:** Normal format, 3 outputs (needs multi-output nodes).

### height_blend: Height Blend
Designer: inputs Height Top, Height Bottom, Mask; Height Offset (0..1), Contrast (0..1), Mode (Balanced height / Bottom height priority), Opacity (0..1); outputs blended height + mask.
Ours: Opacity, Height transition, Output. **Missing:** Mode, Contrast, Height Offset naming, second output.

### edge_wear / dirt / dust: mesh-based mask generators in Designer
They need baked mesh maps as inputs.
- Edge Wear: Level, Contrast, Threshold, Edges Width, Disorder (0..1 each); inputs Curvature, Mask. Ours: Amount, Edge width, Contrast.
- Metal Edge Wear: Wear Level, Wear Contrast, Edges Smoothness (0..16), Grunge Amount, Grunge Scale (1..16), Ambient Occlusion Masking, Curvature Weight, Use Custom Grunge, Use Triplanar, Triplanar Blending Contrast.
- Dirt: Dirt Level, Dirt Contrast, Grunge Amount, Edges Masking, Use Custom Grunge, Grunge Scale (1..16), Use Triplanar, Triplanar Blending Contrast (0.001..1); inputs Curvature, AO, Grunge, Mask, World Normal, Position. Ours: Amount, Contrast.
- Dust: Level, Contrast, Occlusion Amount, Noise Opacity, Override Noise; inputs AO, World Space Normal, Noise, Mask. Ours: Amount, Contrast.
- Dirt 1-5 / Dirt Gradient (noise generators): Scale, Disorder (+speed, anisotropy, angle), Tile offset, Non-square expansion (Dirt 2-5 not fetched).
- Weathering family (Cracks, Fabric, Leather, Metal, Moss, Rock, Rust): index page only, parameters undocumented.
**Missing:** all parameters above; Dirt/Dust live in the Mesh-based generators section and need baked inputs (roadmap item).

### Material filters not implemented
Material Height Blend (channel toggles, Height Offset, Contrast, Mode, Opacity, Albedo Matching), Water Level (Channels, Water Level, Water Darkness, Edges Wetness (+Distance), Depth Blur Amount/Opacity, Sludge Color/Depth/Opacity, Frost, Frost Intensity, Frost Cracks, Frost Normal Format), Snow Cover (Channels, Fresh Snow, Melted Snow, Buildup, Smoothness, Flakes Intensity), Height Normal Blender (Normal Intensity 0..16, Normal Format).

---
## 7. SDF nodes
Designer has **no compositing nodes** called SDF Primitive 2D/3D, SDF Boolean, SDF to Mask or Extrude. SDF in Designer is (a) a family of **function-graph** nodes (primitives, operators Union / Intersection / Subtraction, morphs, shell, symmetries, transforms, material set) used by Shape Splatter v2 and the 3D viewer, and (b) the compositing node **3D Texture SDF** (input Mask Input; Threshold; Output: Distance Field / Signed Distance Field). Our sdf_shape, sdf_op, sdf_mask, sdf_form are therefore our own design:
- sdf_shape: Shape, Size, Aspect, Sides, Rotation, Corner rounding, Position X/Y, Distance range
- sdf_op: Operation, Smoothing, Distance range
- sdf_mask: Offset, Edge softness
- sdf_form: Shape, Size, Repeat, Rounding, Size variation, Height, seed
Action: keep, and add function-graph SDF later.

---
## 8. Graph items

### output: Output
Designer attributes: Identifier, Description, Label, User data, Group; integration: Component, Usage (list not documented), Color space. No image parameters; Output Format is a base parameter.
Ours: usage list of 11. **Missing:** Label, Description, Group, User data, Component, Color space, "Set as default output".

### input: Input Color / Input Grayscale / Input Value
Designer: PKG resource path (preview bitmap), Default value (colour / gray / value); attributes Identifier, Description, Label, User data, Group; Usage, Color space. Input Color page could not be fetched; inferred from Input Grayscale and Input Value.
Ours: Input index, Grayscale. **Missing:** Default value, preview resource, Identifier / Label / Description / Group, Input Value (float / int / vectors / strings).

### dot: Dot
Designer: Name (turns it into a portal transmitter), Input Portal (receiver picks a transmitter). Ours: none. **Missing:** portals (named link jumps).

### subgraph: Graph instance
Designer: Input parameters section (exposed graph parameters, with Preview and Presets tabs), base parameters (default Relative to input) with an "Override base parameters" menu; context menu Open reference / Open reference in context / Expose parameters.
Ours: exposed parameters. **Missing:** Presets tab, override base parameters, in-context editing, Expose-parameters dialog.

### comment / frame: not researched here (no image parameters).

---
## 9. Not in Designer / no page found
- No Designer node with the same name: `tint`, `gradient_reflected`, `sobel`, `adjust` (Contrast/Luminosity), `circle`/`rect` (they are Shape patterns), `edge_wear`/`dirt`/`dust` as image filters (they are mesh-based generators), all four SDF nodes.
- No documentation page found: Fractal Noise, Star 1/2, Checker 2, Tile Brick, Lines, Gradient Angular, Waveform 2/3 (not checked), Flood Fill to Random Grayscale (page empty), Input Color (fetch failed).
- Page-to-variant mapping unverified: Cells 1/2/3, Crystal 1/2 ranges.
- Voronoi's page describes the 3D GPU Voronoi; the classic 2D Voronoi may expose fewer options.

---
## 10. Summary and suggested order

**Nodes whose properties are already complete or nearly so (about 20):** blur, blur_hq, directional_blur, slope_blur, highpass, histogram_scan, histogram_range, histogram_shift, pow, hsl, channel_mixer, replace_color, auto_levels, chrominance_extract, normal_normalize, merge, alpha_merge, multi_directional_warp (minus Mode), vector_warp (minus Format), anisotropic_blur (minus Quality).

**Cheap wins (one or two missing properties, no new engine work):** invert toggle, threshold Mode, clamp Apply to Alpha, blend Add Sub + Alpha blending + Cropping area, levels Intermediary clamp, normal_invert Blue/Alpha, normal Alpha channel content, normal_combine Technique, normal_blend Opacity/mask, normal_vector_rotation Normal Format, normal_to_height Relief Balance/Global Opacity, vector_warp Format, multi_directional_warp Mode, directional_warp filtering/offset, warp filtering, sharpen Punchthrough Alpha, mirror offsets/invert/corner, symmetry modes + Transfer Mode, skew Axis/Amount/Align, glow Clear Amount, shape glow/stroke/shadow Spread/Opacity/Colour, bevel Corner Type/Normal Format, make_it_tile_photo six H/V sliders, non-uniform blur extras.

**Common block, touches ~25 noise nodes:** Disorder speed, Disorder anisotropy (+angle), Tile offset, Non-square expansion, plus Non-square expansion on every pattern.

**Larger rebuilds (new behaviour):** Tile Sampler (~45 properties, 7 map inputs), Tile Generator (~35), Splatter / Splatter Circular (~40), Voronoi + Voronoi Fractal, Triangle Grid, Grunge Map 001-015 and the Advanced grunge nodes, Cells 1-4 per-variant parameters, Shape pattern list (Bell, Thorn, Brick, Gradation, Waves, Ridged Bell, Crescent, Capsule), Polygon 1/2, Weave Generator, Edge Detect, Mosaic and Radial Blur (behaviour differs from Designer), Gradient Map editor (addressing, interpolation modes, more keys), Quantize Color, Color Match, Ambient Occlusion (RTAO + world units).

**Needs engine features first:** multi-output nodes (Bevel, Curvature Smooth, Quantize, Height Blend), second image inputs (Distance Source, Color to Mask, Channels Shuffle, Normal Vector Rotation map, Emboss height, Non-uniform blur map exists), function graphs (Pixel Processor, Waveform custom function), baked mesh inputs (Dirt, Dust, Edge Wear, Metal Edge Wear, Weathering), per-node Output Format / Tiling Mode / Pixel Ratio.


## Reference comparison (Oct 2026)
A local comparison of our parameter defaults/ranges against the installed Designer library was generated (kept in the git-ignored `reference/` folder, never committed). Applied so far: Edge Detect `edge_roundness` default 4. Remaining known default/range differences (65 defaults, ~450 missing params, mostly `non_square_expansion`, `disorder`, `enable_tiling`, vector size/offset) are queued; many differ only in unit convention (e.g. blur/glow widths normalised in Designer, pixels here), so they are changed per node together with `compute()` and tests, not in bulk.

**New-node defaults:** nodes created in the editor now start with Designer-style values for polygon sides (6), tile sampler/generator counts (16/10), circular splatter (10 / radius 0.25 / centre orientation on), splatter grid number 4, histogram range 0.5, pow exponent 4 and shape drop shadow (angle 0.25, distance 0.02, opacity 0.5). They are stored in the node, so older graphs render unchanged (`new_node_defaults` in gui_params.rs). Radial Blur defaults are 64 samples / angle 0.125 and polygon sides go up to 32.

**Phase 6 (Oct 2026):** Triangle Grid gained a jittered-mesh style (`style=1`: x/y amount, random position, quincunx offsets, fixed/alternate/random triangulation, smooth/flat/edge-distance output, height random, thin lines) while `style=0` keeps the old square-triangle look; new nodes get style 1. New node Weave Generator (tile x/y, weave count, shape, gap x/y; our own implementation). Still open for these two: vector-map/height inputs, rotation/rotation disorder, colour output, blend modes, weave opacity/mask outputs.

**Phase 6b:** new node Grunge Map (maps 1-15, our own recipes modelled by eye on Designer's fifteen grunge maps: specks, streaks, blobs, thin scratches, dirt bands, blotches, speckle, crazing, cracks, drips, wispy grain). Parameters: map, balance, contrast, brush pattern, cracks (maps 11/13), disorder, invert, seed. Each map's default dirt coverage was calibrated to the mean coverage of the matching reference render. Looks are similar in character, not identical; the old Grunge node is unchanged.

**Phase 7 (Oct 2026):** (a) Grunge Map cracks reworked: jagged hairline cracks with wandering width, gaps and a second branch network (maps 10, 11, 13); coverage re-calibrated. (b) Gradient Map: addressing (clamp / repeat), smoothness for smooth interpolation, a fourth 'smoother' interpolation, per-key alpha (`a0`..`a15`, editable in files) and a 'use gradient alpha' switch; up to 16 keys in the engine. Still open: midpoint per key, alpha editing in the editor, eyedropper. (c) Color Match: new 'Colour swap' method (Designer-style): source colour from the image average, a colour or an input; target colour or input; custom variation (hue / chrominance / luminance); mask by hue, chroma and luma range with smoothness. Works in OKLCh. The old statistics transfer stays as method 0; nodes created in the editor start with the colour swap. Still open: the Mask output (needs multi-output nodes), mask blur and mask input.

**Phase 8 (Oct 2026):** (a) Ambient Occlusion gained a horizon-based method (`mode=1`): radius, intensity, quality (4 / 8 / 16 directions), 'use world units' with surface size (cm) and height depth (cm), or screen-relative height depth; the old blur AO stays as mode 0 and new nodes start with the horizon method. Still open: RTAO (samples, distribution, max distance, spread angle, physical size), GPU optimisation switch. (b) New Quantize Color node (k-means over the colour histogram, deterministic): max colour amount, contour smoothing, dithering with noise or Bayer pattern, ignore alpha, Lab or RGB distance. Still open: the ID, Palette and Palette colour amount outputs (need multi-output nodes), ID sort modes, downscale filtering.

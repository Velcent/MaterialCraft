# MaterialCraft node reference

136 texture node types, 84 function-graph node types and 5 pattern-program node types. This page is generated from the node definitions in `src/graph.rs` (`materialcraft nodes` prints the same data). Parameters are listed after the colon; every parameter has a default, and the editor's Properties panel shows ranges and units.

Common to most nodes: **Output Size** (relative to input, relative to parent or absolute, with a power-of-two modifier), **Output format** (graph default, 8 / 16-bit integer, 16-bit float or 32-bit float; the graph default is set under Node Precision and is 32-bit float unless changed), `seed`, `contrast`, `invert` on generators, and the shared **disorder** group (`disorder`, `disorder_speed`, `anisotropy`, `anisotropy_angle`) and **tile offset** (`tile_x`, `tile_y`) on noises. Every node tiles seamlessly and works on non-square graphs; the cell-based generators (`perlin`, `fbm`, `clouds`, `voronoi`, `cells`, `tile_sampler`, `tile_generator`) also take `square_cells=1` to keep their cells square when the graph is not.

Nodes with several outputs: `color_match` (output, mask); `quantize_color` (output, id, palette); `weave_generator` (height, mask_1, mask_2, opacity); `rgba_split` (r, g, b, a); `alpha_split` (rgb, alpha); `voronoi` (output, cell_id, edge, distance); `cells` (output, cell_id, distance, edge); `splatter` (output, id, center, size, luminance); `flood_fill_to_position` (position, x, y); `flood_fill_to_bbox_size` (size, width, height). Link to a named output with `in=@id.output` in a `.mcg` file, or with `output <name> <id>.<output>` for a graph output.

## Generators

| Node | Inputs | Description and parameters |
|---|---|---|
| `color` |  | constant colour: r g b a |
| `white_noise` |  | per-pixel random: seed distribution(0 uniform,1 gaussian,2 triangle) |
| `gaussian_noise` |  | gaussian-distributed per-pixel noise: seed scale contrast invert disorder disorder_speed anisotropy anisotropy_angle tile_x tile_y |
| `perlin` |  | perlin noise: scale scale_x scale_y seed disorder disorder_speed tile_x tile_y |
| `fbm` |  | fractal noise/clouds: scale octaves gain seed mode(0 fbm,1 ridged,2 turbulence) disorder disorder_speed anisotropy anisotropy_angle tile_x tile_y |
| `voronoi` |  | cellular: scale jitter seed mode(0 f1,1 cell id,2 edge,3 f2,4 f2-f1,5 f1*f2,6 f1/f2) aspect offset_x offset_y distortion distortion_scale distance_mode(0 euclid,1 manhattan,2 chebyshev,3 minkowski) minkowski edge_thickness rounded distance_scale color_seed_mode color_seed invert |
| `voronoi_fractal` |  | layered voronoi: scale jitter seed mode aspect offset_x offset_y distortion distortion_scale distance_mode minkowski edge_thickness rounded distance_scale min_level max_level roughness lacunarity opacity blend(0 add,1 max,2 min) color_seed_mode color_seed invert |
| `gradient` |  | gradient: mode(0 linear,1 radial) angle |
| `circle` |  | radius softness |
| `rect` |  | width height softness |
| `polygon` |  | sides radius softness rotation |
| `checker` |  | tiles |
| `bricks` |  | rows cols mortar bevel offset lum_var seed |
| `pixel_processor` | in, in2, in3, in4 | per-pixel expr / expr_g / expr_b / expr_a (string params), or fn="function" to run a function graph per pixel |
| `shape` |  | procedural shape: square, disc, paraboloid, gaussian, diamond, pyramid, half sphere, cone |
| `tile_sampler` | pattern, pattern2, patte | rn3,pattern4,pattern5,pattern6,mask,scale_map,rotation_map,displacement_map,color_map,distribution_map,background grid of pattern instances: x_amount y_amount size aspect size_mode scale scale_random size_random rotation rotation_random pattern_rotation symmetry_random position_random offset offset_mode offset_random global_offset_x global_offset_y color color_random lum_by_number lum_by_scale mask_threshold mask_invert mask_random mask_checker invert_mask blending opacity background reverse_order pattern_type pattern_specific pattern_distribution |
| `tile_generator` | pattern, pattern2, patte | rn3,pattern4,pattern5,pattern6,mask,scale_map,rotation_map,displacement_map,color_map,distribution_map,background brick-style tiles: x_amount y_amount size aspect size_mode scale scale_random size_random rotation rotation_random pattern_rotation symmetry_random position_random offset offset_mode offset_random global_offset_x global_offset_y color color_random lum_by_number lum_by_scale mask_checker mask_horizontal mask_vertical mask_random invert_mask blending opacity background reverse_order pattern_type pattern_specific pattern_distribution |
| `splatter` | pattern, pattern2, patte | rn3,pattern4,pattern5,pattern6 random scatter of a pattern: count grid_number scale scale_var aspect rotation rot_var pattern_rotation lum_var disorder pan_x pan_y symmetry_random gain output_min output_max background blending opacity pattern_type pattern_specific pattern_distribution |
| `splatter_circular` | pattern, pattern2, patte | rn3,pattern4,pattern5,pattern6 instances on one or more circles: number amount_random rings radius radius_random ring_radius_multiplier angle_random spiral_factor spread size size_random scale_by_number scale_by_ring rotation rotation_random center_orientation ring_rotation ring_rotation_random position_random color luminance_random lum_by_number lum_by_ring mask_random background blending opacity pattern_type pattern_specific pattern_distribution |
| `clouds` |  | soft cloud noise: scale seed contrast invert disorder disorder_speed anisotropy anisotropy_angle tile_x tile_y |
| `bnw_spots` |  | black and white spots: scale seed contrast invert variant disorder disorder_speed anisotropy anisotropy_angle tile_x tile_y |
| `crystal` |  | faceted crystal cells: scale disorder seed contrast invert variant |
| `directional_noise` |  | streak noise, seamless at any angle: scale stretch angle seed contrast invert disorder disorder_speed anisotropy anisotropy_angle tile_x tile_y |
| `grunge` |  | grunge masks: variant(0 concrete,1 scratches,2 spots,3 smudges) balance large_variation base_noisiness dirt_specks_opacity invert_dirt scratches_opacity sharpen quantity scratch_width scratch_length details coverage coverage_contrast disorder disorder_speed seed contrast invert |
| `plasma` |  | smooth plasma waves |
| `stripes` |  | stripes: count angle softness width tile(1 = snap to seamless angle, 0 = exact) |
| `weave` |  | woven threads: count width |
| `star` |  | star shape: points inner size softness rotation |
| `gradient_axial` |  | symmetric gradient: angle |
| `gradient_circular` |  | angular gradient: offset |
| `cells` | color | cellular noise: scale jitter seed variant(0 cells1,1 cells2,2 cells3,3 cells4) luminance_random color_source(0 random,1 pseudorandom,2 image) pseudorandom_seed edge_width hardness contrast invert disorder disorder_speed anisotropy anisotropy_angle tile_x tile_y |
| `fractal_sum` |  | sum of noise octaves: scale min_level max_level roughness seed mode(0 base,1 turbulence,2 creased) contrast invert disorder disorder_speed |
| `waveform` |  | tileable wave pattern: count angle warp warp_scale seed shape(0 sine,1 triangle,2 saw,3 square) contrast invert disorder disorder_speed |
| `gaussian_spots` |  | soft gaussian spots: scale size size_variation seed contrast invert disorder disorder_speed anisotropy anisotropy_angle tile_x tile_y |
| `triangle_grid` |  | grid of triangles: style(0 squares,1 jittered mesh) count border variation x_amount y_amount random_position_multiplier quincunx_offset_x quincunx_offset_y triangulation(0 fixed,1 alternate,2 random) output(0 smooth,1 flat,2 edge distance) height_random thin_lines distance_to_edge_thickness thin_line_color thin_line_background seed contrast invert |
| `grunge_map` |  | grunge map 1-15: map balance contrast brush_pattern cracks disorder coverage seed invert |
| `weave_generator` |  | woven threads with gaps: tile_x tile_y weave shape gap_x gap_y contrast invert |
| `starburst` |  | star-like rays: count rays sharpness size rotation rotation_random seed contrast invert |
| `gradient_reflected` |  | gradient that goes 0-1-0: count angle contrast invert |
| `fx_map` | in, in2, in3, in4 | run a pattern program: program="name" depth seed blend (0 max 1 add 2 over 3 min 4 multiply) color bg bg_r bg_g bg_b |
| `svg` |  | vector image: file="art.svg" fit (0 contain 1 stretch 2 cover) scale x y |
| `text` |  | text: text="..." font="face.ttf" (optional) size x y align (0 left 1 centre 2 right) spacing line_spacing rotation value |
| `value_processor` | in, in2, in3, in4 | run fn="function" once; the result (float, float3 or float4) fills the image. The function can read image statistics |

## SDF

| Node | Inputs | Description and parameters |
|---|---|---|
| `sdf_shape` |  | 2D signed distance shape: shape(0 circle,1 box,2 polygon,3 ring) size aspect sides rotation round x y range |
| `sdf_op` | a, b | combine SDFs: op(0 union,1 subtract,2 intersect) smooth range |
| `sdf_mask` | in | SDF -> mask: offset softness |
| `sdf_form` |  | procedural 3D SDF form as height: shape(0 sphere,1 box,2 torus,3 cylinder) size repeat round scatter seed height |

## Utility

| Node | Inputs | Description and parameters |
|---|---|---|
| `bitmap` |  | flip_x flip_y gray; imported PNG bitmap: file="path.png" |
| `input` |  | subgraph input: idx |
| `subgraph` | in0, in1, in2, in3 | instance of a reusable subgraph: graph="name" |
| `output` | in | graph output: usage identifier |
| `dot` | in | re-route a link |
| `input_value` |  | graph parameter read by functions: name, type (0 float, 1 int, 2 bool, 3 float2, 4 float3, 5 float4, 6 int2, 7 int3, 8 int4, 9 string), v / x y z w, min max, label |

## Filters

| Node | Inputs | Description and parameters |
|---|---|---|
| `switch` | a, b, c, d | pass one of up to four inputs: index |
| `flood_fill` | in | label connected regions of a mask |
| `flood_fill_to_index` | in | region number of each flood-fill region as a gray level |
| `flood_fill_to_position` | in | centre of each flood-fill region (outputs: position, x, y) |
| `flood_fill_to_bbox_size` | in | bounding-box size of each flood-fill region (outputs: size, width, height) |
| `flood_fill_random` | in | random grayscale per flood-fill region |
| `flood_fill_color` | in | random color per flood-fill region |
| `flood_fill_gradient` | in | gradient across each flood-fill region |
| `vector_warp` | in, vector | displace by a vector map: intensity directx |
| `slope_blur` | in, slope | blur along a slope map: samples intensity mode |
| `blur_hq` | in | high quality blur: intensity quality |
| `histogram_range` | in | select a range of values: position range contrast invert |
| `bevel` | in | bevel a shape: distance smoothing threshold use_as_normal normal_intensity corner(0 round,1 angular) directx |
| `distance` | in | distance field of a mask: distance mode |
| `color_match` | in, reference, src_color | ,target_color recolour: method(0 statistics transfer from reference,1 colour swap) strength source_mode(0 average,1 colour,2 input) src_r src_g src_b target_mode(0 colour,1 input) target_r target_g target_b custom_variation variation_hue variation_chroma variation_luma use_mask mask_hue_range mask_chroma_range mask_luma_range mask_smoothness |
| `make_it_tile_photo` | in | make a photo tile: width width_v |
| `make_it_tile_patch` | in | make an image tile with random patches |
| `clamp` | in | clamp values: low high apply_alpha |
| `highpass` | in | high-pass filter: radius |
| `swirl` | in | swirl distortion: amount radius |
| `blur` | in | gaussian blur: radius |
| `directional_blur` | in | angle length |
| `sharpen` | in | radius amount |
| `levels` | in | low high gamma out_low out_high |
| `adjust` | in | brightness contrast |
| `invert` | in | invert colours: invert |
| `threshold` | in | binary threshold: threshold softness mode(0 greater,1 greater or equal,2 lower,3 lower or equal) |
| `histogram_scan` | in | position width |
| `quantize` | in | steps |
| `to_gray` | in | luminance |
| `channel` | in | ch(0 r,1 g,2 b,3 a) |
| `merge` | r, g, b, a | combine channels |
| `tint` | in | multiply by colour: r g b |
| `colorize` | in | gradient map: interp(0 linear,1 smooth,2 step,3 smoother) smoothness addressing(0 clamp,1 repeat) use_alpha  stops p0 r0 g0 b0 a0 ... p15 r15 g15 b15 a15 |
| `hsl` | in | hue sat val |
| `transform` | in | offset_x offset_y scale_x scale_y rotation(turns) |
| `mirror` | in | mirror an image: axis(0 x,1 y,2 both,3 corner) offset_x offset_y invert_x invert_y corner(0 top left,1 top right,2 bottom left,3 bottom right) |
| `warp` | in, warp | slope warp: intensity filtering(0 bilinear,1 nearest,2 bicubic) |
| `directional_warp` | in, warp | warp in one direction: angle intensity offset filtering(0 bilinear,1 nearest,2 bicubic) |
| `blend` | fg, bg, mask | blend two images: mode(0 copy,1 add,2 sub,3 mul,4 screen,5 overlay,6 max,7 min,8 diff,9 softlight,20 add sub,21 switch) opacity alpha_mode(0 source alpha,1 ignore alpha,2 straight,3 premultiplied) crop_x0 crop_y0 crop_x1 crop_y1 |
| `quantize_color` | in | reduce to few colours (k-means): color_amount contour_smoothing dithering dithering_pattern(0 noise,1 bayer) ignore_alpha distance_color_space(0 lab,1 rgb) |
| `edge_detect` | in | edge lines: mode(0 lines,1 sobel) edge_width edge_roundness invert tolerance intensity |
| `emboss` | in | angle intensity |
| `non_uniform_blur` | in, mask | blur by a mask: intensity |
| `radial_blur` | in | spin blur around a centre: mode(0 spin,1 zoom) samples angle x y amount radius |
| `anisotropic_blur` | in | blur stretched in one direction: intensity anisotropy angle |
| `symmetry` | in | mirror the image around a line: axis(0 x,1 y,2 both,3 diagonal,4 anti-diagonal,5 eight-fold) offset flip transfer(0 copy,1 add,2 sub,3 mul,4 add sub,5 max,6 min) |
| `skew` | in | shear that keeps tiling: skew_x skew_y (whole numbers) align(0 centre,1 top left,2 bottom right) |
| `auto_levels` | in | stretch values to the full range: per_channel |
| `channel_mixer` | in | mix colour channels: rr rg rb gr gg gb br bg bb mono |
| `alpha_merge` | rgb, alpha | put a grayscale image into the alpha channel |
| `color_to_mask` | in | mask of pixels close to a colour: r g b range softness |
| `replace_color` | in | replace one colour with another: src_r src_g src_b dst_r dst_g dst_b range softness |
| `pow` | in | raise values to a power: exponent |
| `histogram_shift` | in | shift all values: position |
| `grayscale_conversion` | in | weighted grayscale: r g b |
| `sobel` | in | sobel edge magnitude: intensity |
| `glow` | in | glow around bright areas: width intensity clear glow_r glow_g glow_b |
| `mosaic` | in, mosaic_map | mosaic of the map regions (or a pixel grid without a map): count intensity |
| `non_uniform_directional_warp` | in, intensity, angle | warp along a direction chosen per pixel: intensity angle |
| `multi_directional_warp` | in, warp | warp in several directions at once: intensity directions(0 one,1 two,2 four) angle mode(0 average,1 max,2 min,3 chain) |
| `rgba_split` | in | split into R, G, B and A grayscale outputs |
| `alpha_split` | in | split into an RGB output and an alpha output |
| `shape_stroke` | in | outline of a shape: width offset softness threshold opacity color_r color_g color_b |
| `shape_glow` | in | glow around a shape: width softness intensity threshold opacity spread color_r color_g color_b |
| `chrominance_extract` | in | colour without brightness |
| `shape_drop_shadow` | in | drop shadow of a mask: angle distance blur opacity spread |
| `curve` | in | tone curves: points (master), points_r points_g points_b points_a as "x:y,x:y" lists |
| `channel_shuffle` | in, in2 | rebuild channels: src_r src_g src_b src_a (0-3 channels of in, 4-7 channels of in2, 8 zero, 9 one), inv_r inv_g inv_b inv_a |

## Material

| Node | Inputs | Description and parameters |
|---|---|---|
| `curvature_smooth` | in | smooth curvature from a normal map |
| `height_blend` | bg, fg, mask | blend two height maps: opacity contrast output |
| `edge_wear` | in, noise | edge wear mask from a height map: amount width contrast |
| `dirt` | in, noise | dirt mask from an ambient occlusion map: amount contrast |
| `dust` | in, noise | dust mask from a normal map: amount contrast |
| `normal` | in | height->normal (OpenGL +Y): strength directx alpha_from_input |
| `normal_blend` | a, b, mask | blend normal maps (a = background, b = foreground): opacity |
| `ao` | in | ambient occlusion: mode(0 blur,1 horizon) radius intensity hbao_radius hbao_intensity quality(0 4,1 8,2 16 samples) use_world_units height_depth surface_size height_depth_cm |
| `curvature` | in | curvature of a normal map: mode(0 normal map,1 height map) intensity directx radius |
| `normal_invert` | in | flip normal map channels: invert_x invert_y invert_z invert_a |
| `normal_normalize` | in | renormalise a normal map |
| `normal_vector_rotation` | in, rotation | rotate normal vectors: angle directx |
| `normal_to_height` | in | integrate a normal map into a height map: directx relief_balance opacity |
| `normal_combine` | base, detail | combine two normal maps: technique(0 whiteout,1 channel mixer,2 detail oriented) |

## Function-graph nodes

Used inside graphs with `meta kind "function"`. Links carry values (float, float2-4, integer, integer2-4, boolean, string). Slots `a`, `b`, `c` are operands. Running a function never produces NaN or infinity.

| Node | Inputs | Description and parameters |
|---|---|---|
| `f_out` | in | result of the function |
| `f_pin` | a | re-route a value link |
| `f_float` |  | constant float: v |
| `f_int` |  | constant integer: v |
| `f_bool` |  | constant boolean: v (0 or 1) |
| `f_string` |  | constant string: v |
| `f_vec2` | x, y | float2 from parts (x y, linked parts win over the typed values) |
| `f_vec3` | x, y, z | float3 from parts |
| `f_vec4` | x, y, z, w | float4 from parts |
| `f_ivec2` | x, y | integer2 from parts |
| `f_ivec3` | x, y, z | integer3 from parts |
| `f_ivec4` | x, y, z, w | integer4 from parts |
| `f_swizzle` | a | pick or reorder components: mask (xyzw or rgba, e.g. "zyx") |
| `f_neg` | a | negate |
| `f_abs` | a | absolute value |
| `f_sqrt` | a | square root (0 for negative input) |
| `f_exp` | a | e to the power a |
| `f_log` | a | natural logarithm (0 for input <= 0) |
| `f_log2` | a | base-2 logarithm (0 for input <= 0) |
| `f_sin` | a | sine (radians) |
| `f_cos` | a | cosine (radians) |
| `f_tan` | a | tangent (radians) |
| `f_asin` | a | inverse sine (input clamped to -1..1) |
| `f_acos` | a | inverse cosine (input clamped to -1..1) |
| `f_atan` | a | inverse tangent |
| `f_radians` | a | degrees to radians |
| `f_degrees` | a | radians to degrees |
| `f_floor` | a | round down |
| `f_ceil` | a | round up |
| `f_round` | a | round to nearest |
| `f_fract` | a | fractional part |
| `f_sign` | a | -1, 0 or 1 |
| `f_saturate` | a | clamp to 0..1 |
| `f_not` | a | boolean not |
| `f_length` | a | length of a vector |
| `f_normalize` | a | vector of length 1 (zero stays zero) |
| `f_add` | a, b | a + b |
| `f_sub` | a, b | a - b |
| `f_mul` | a, b | a * b |
| `f_div` | a, b | a / b (0 when b is 0; integer division truncates) |
| `f_mod` | a, b | remainder of a / b with the sign of b (0 when b is 0) |
| `f_pow` | a, b | a to the power b |
| `f_min` | a, b | smaller value |
| `f_max` | a, b | larger value |
| `f_atan2` | a, b | angle of the point (b, a) |
| `f_step` | a, b | 0 when b < a, else 1 |
| `f_dot` | a, b | dot product |
| `f_cross` | a, b | cross product of two float3 |
| `f_distance` | a, b | distance between two points |
| `f_clamp` | a, b, c | clamp a between b (low) and c (high) |
| `f_lerp` | a, b, c | a + (b - a) * c |
| `f_smoothstep` | a, b, c | smooth 0..1 ramp of c between edges a and b |
| `f_eq` | a, b | a == b |
| `f_ne` | a, b | a != b |
| `f_lt` | a, b | a < b |
| `f_le` | a, b | a <= b |
| `f_gt` | a, b | a > b |
| `f_ge` | a, b | a >= b |
| `f_and` | a, b | boolean and |
| `f_or` | a, b | boolean or |
| `f_xor` | a, b | boolean exclusive or |
| `f_if` | cond, a, b | a when cond is true, else b (only the chosen branch runs) |
| `f_sequence` | a, b | run a, then b; returns b |
| `f_set` | a | store a in the variable `name`; returns a |
| `f_get` |  | read the variable `name` (0 until it is set) |
| `f_random` | a, b | random float between a (default 0) and b (default 1); `seed` shifts the sequence |
| `f_sample_gray` | a | brightness of input image `slot` (default in) at the position a (float2, 0..1) |
| `f_sample_color` | a | colour (float4) of input image `slot` at the position a |
| `f_image_avg` |  | average colour (float4) of input image `slot` |
| `f_image_min` |  | smallest channel values (float4) of input image `slot` |
| `f_image_max` |  | largest channel values (float4) of input image `slot` |
| `f_pos` |  | position being evaluated (float2, 0..1) |
| `f_size` |  | output size in pixels (float2) |
| `f_index` |  | instance / iteration number (integer) |
| `f_count` |  | number of instances / iterations (integer) |
| `f_seed` |  | random seed of the caller (integer) |
| `f_input` |  | input of this function: name, type (0 float, 1 int, 2 bool, 3 float2, 4 float3, 5 float4, 6 int2, 7 int3, 8 int4, 9 string), default v / x y z w |
| `f_graph_input` |  | value of an Input Value node of the graph the caller lives in: name, type, default v / x y z w |
| `f_to_float` | a | convert to float |
| `f_to_int` | a | convert to integer (drops the fraction) |
| `f_to_bool` | a | non-zero is true |
| `f_to_string` | a | text of a value |
| `f_concat` | a, b | join two strings |
| `f_call` | a, b, c, d | run another function (`fn`); a b c d fill its inputs in the order of their node numbers |

## Pattern-program nodes

Used inside graphs with `meta kind "fxmap"`, run by the `fx_map` node. A cell (centre, size, rotation, value, colour) is passed down; quadrants and iterations change it, markers draw.

| Node | Inputs | Description and parameters |
|---|---|---|
| `fxm_out` | in | start of a pattern program |
| `fxm_marker` |  | draw a shape: shape (0 circle 1 square 2 diamond 3 image) src x y w h rot value r g b softness rnd_pos rnd_scale rnd_rot rnd_val rnd_color |
| `fxm_quadrant` | q0, q1, q2, q3 | split the cell in four: scale off_x off_y p0..p3 (chance) v0..v3 (value) r0..r3 (rotation) rnd_pos rnd_scale rnd_rot rnd_val |
| `fxm_iterate` | body | repeat body: count dx dy dscale drot dval, or grid_x grid_y stagger; rnd_x rnd_y rnd_scale rnd_rot rnd_val |
| `fxm_switch` | a, b, c, d | run one branch: mode (0 index, 1 random) index w0..w3 |

## Planned node families

Described by what they do; see `RELEASE_PLAN.md` for the order.

- More noises: fur-like, fibre-like, fluid-like, liquid, caustics, anisotropic and blue noise, 3D noises (perlin, simplex, worley, ridged), and more grunge and dirt recipes.
- More patterns: arc and herringbone pavements, brick variants, fibres, meshes, scratches, extruded and mapped shapes, shape splatter with data extraction.
- More adjustments: contrast / luminosity, linear / sRGB conversion, colour lookup tables, histogram select and equalise, frequency tools, pre-multiplied conversion.
- More effects: cross sections, diffusion, uber emboss, anti-aliasing, reaction diffusion, summed area tables, vector morph, flood fill mapping outputs.
- More transforms: auto crop, polar conversion, clone, noise upscaling, safe and non-square transforms, trapezoid / quad transforms.
- Normal tools: bent and facing normals, height-normal blending, world-unit height to normal, normal transform.
- Material tools: PBR converters and validators, material blending, layers, snow / season / water filters, palette tools.
- Mesh-driven generators that use baked curvature, thickness, position and AO maps.
- Splines and paths with shape, scatter and mapping nodes.

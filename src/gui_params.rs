// ======================================================================= Node properties (included into gui.rs)

#[derive(Clone, Copy)]
enum PK {
    F { def: f32, lo: f32, hi: f32 },
    I { def: i32, lo: i32, hi: i32 },
    E { def: i32, opts: &'static [&'static str] },
    B { def: bool },
}

#[derive(Clone, Copy)]
struct PD {
    name: &'static str,
    label: &'static str,
    k: PK,
}

fn pf(name: &'static str, label: &'static str, def: f32, lo: f32, hi: f32) -> PD {
    PD { name, label, k: PK::F { def, lo, hi } }
}
fn pi(name: &'static str, label: &'static str, def: i32, lo: i32, hi: i32) -> PD {
    PD { name, label, k: PK::I { def, lo, hi } }
}
fn pe(name: &'static str, label: &'static str, def: i32, opts: &'static [&'static str]) -> PD {
    PD { name, label, k: PK::E { def, opts } }
}
fn pb(name: &'static str, label: &'static str, def: bool) -> PD {
    PD { name, label, k: PK::B { def } }
}

const BLEND_MODES: &[&str] = &["Copy", "Add", "Subtract", "Multiply", "Screen", "Overlay", "Max", "Min", "Difference", "Soft light", "Color dodge", "Color burn", "Linear burn", "Hard light", "Exclusion", "Divide", "Linear light", "Color", "Luminosity", "Dissolve"];
const SHAPES: &[&str] = &["Square", "Disc", "Paraboloid", "Gaussian", "Diamond", "Pyramid", "Half sphere", "Cone"];

const ROT90: &[&str] = &["0", "90", "180", "270"];
const SYMM: &[&str] = &["None", "Horizontal", "Vertical", "Both"];
const TILE_BLEND: &[&str] = &["Max", "Add", "Alpha blend", "Add / Sub"];
const SIZE_MODES: &[&str] = &["Normal (fill cell)", "Keep ratio", "Absolute"];
const OFFSET_MODES: &[&str] = &["None", "Odd rows", "Even rows", "Odd columns", "Even columns"];
const CHECKER: &[&str] = &["Off", "Checker A", "Checker B"];
const DISTRIB: &[&str] = &["Random", "By pattern number", "By ring number (circular)"];

/// Pattern selection shared by every scatter node.
fn pattern_block() -> Vec<PD> {
    vec![
        pe("pattern_type", "Pattern", 0, materialcraft::scatter::PATTERN_NAMES),
        pf("pattern_specific", "Pattern specific", 0.5, 0.0, 1.0),
        pe("pattern_distribution", "Pattern input distribution", 0, DISTRIB),
    ]
}

/// Properties shared by Tile Sampler and Tile Generator (everything except the X / Y amounts).
fn tile_common(gen: bool) -> Vec<PD> {
    let mut v = vec![
        pf("size", "Size", if gen { 0.92 } else { 0.8 }, 0.05, 3.0),
        pf("aspect", "Size Y / X", 1.0, 0.1, 4.0),
        pe("size_mode", "Size mode", 0, SIZE_MODES),
        pf("scale", "Scale", 1.0, 0.0, 10.0),
        pf("scale_random", "Scale random", 0.0, 0.0, 1.0),
        pi("scale_seed", "Scale random seed", 0, 0, 9999),
        pf("size_random", "Size random", 0.0, 0.0, 1.0),
        pf("rotation", "Rotation", 0.0, 0.0, 1.0),
        pf("rotation_random", "Rotation random", 0.0, 0.0, 1.0),
        pe("pattern_rotation", "Pattern rotation", 0, ROT90),
        pe("symmetry_random", "Symmetry random", 0, SYMM),
        pf("offset", if gen { "Row offset" } else { "Offset" }, 0.5, 0.0, 1.0),
        pe("offset_mode", "Offset type", if gen { 1 } else { 0 }, OFFSET_MODES),
        pf("offset_random", "Offset random", 0.0, 0.0, 1.0),
        pi("offset_seed", "Offset random seed", 0, 0, 9999),
        pf("global_offset_x", "Global offset X", 0.0, -1.0, 1.0),
        pf("global_offset_y", "Global offset Y", 0.0, -1.0, 1.0),
        pf("position_random", "Position random", 0.0, 0.0, 1.0),
        pf("position_random_x", "Position random X", 1.0, 0.0, 10.0),
        pf("position_random_y", "Position random Y", 1.0, 0.0, 10.0),
        pf("color", "Color", 1.0, 0.0, 1.0),
        pf("color_random", "Color random", 0.0, 0.0, 1.0),
        pf("lum_by_number", "Luminance by number", 0.0, 0.0, 1.0),
        pf("lum_by_scale", "Luminance by scale", 0.0, 0.0, 1.0),
        pf("mask_threshold", "Mask map threshold", 0.5, 0.0, 1.0),
        pb("mask_invert", "Mask map invert", false),
        pf("mask_random", "Random mask", 0.0, 0.0, 1.0),
        pe("mask_checker", "Checker mask", 0, CHECKER),
        pb("mask_horizontal", "Horizontal mask (every other column)", false),
        pb("mask_vertical", "Vertical mask (every other row)", false),
        pb("invert_mask", "Invert mask", false),
        pf("scale_map_multiplier", "Scale map multiplier", 1.0, 0.0, 2.0),
        pf("rotation_map_multiplier", "Rotation map multiplier", 1.0, 0.0, 4.0),
        pf("displacement_intensity", "Displacement map intensity", 0.5, 0.0, 4.0),
        pf("displacement_angle", "Displacement angle", 0.0, 0.0, 1.0),
        pf("color_map_multiplier", "Color map multiplier", 1.0, 0.0, 2.0),
        pf("pattern_specific_random", "Pattern specific random", 0.0, 0.0, 1.0),
        pe("blending", "Blending mode", 0, TILE_BLEND),
        pf("opacity", "Global opacity", 1.0, 0.0, 1.0),
        pf("background", "Background color", 0.0, 0.0, 1.0),
        pb("reverse_order", "Reverse rendering order", false),
    ];
    v.extend(pattern_block());
    v.push(p_seed());
    v
}

/// Category and sub category of a parameter in the Properties panel (empty sub = directly under the category).
fn param_group(kind: &str, name: &str) -> (&'static str, &'static str) {
    let scatter = matches!(kind, "tile_sampler" | "tile_generator" | "splatter" | "splatter_circular");
    if scatter {
        return match name {
            "pattern_type" | "pattern_specific" | "pattern_specific_random" | "pattern_distribution" | "pattern_rotation" | "symmetry_random" => ("Pattern", ""),
            "x_amount" | "y_amount" | "count" | "grid_number" | "number" | "rings" => ("Layout", "Amount"),
            "amount_random" | "amount_random_min" | "ring_radius_multiplier" => ("Layout", "Amount"),
            "offset" | "offset_mode" | "offset_random" | "offset_seed" | "global_offset_x" | "global_offset_y" | "pan_x" | "pan_y" => ("Layout", "Offset"),
            "position_random" | "position_random_x" | "position_random_y" | "disorder" | "radius_random" | "angle_random" => ("Layout", "Randomness"),
            "radius" | "spiral_factor" | "spread" => ("Layout", "Circle"),
            "size" | "aspect" | "size_mode" | "scale" | "scale_var" => ("Size", "Size"),
            "scale_random" | "scale_seed" | "size_random" | "scale_by_number" | "invert_number" | "scale_by_ring" | "invert_ring" => ("Size", "Variation"),
            "rotation" | "rotation_random" | "rot_var" | "center_orientation" => ("Rotation", ""),
            "ring_rotation" | "ring_rotation_random" | "ring_rotation_offset" => ("Rotation", "Rings"),
            "color" | "color_random" | "lum_var" | "luminance_random" | "lum_by_number" | "lum_by_scale" | "lum_by_ring" => ("Color", "Luminance"),
            "blending" | "opacity" | "background" | "gain" | "output_min" | "output_max" | "reverse_order" => ("Color", "Blending and output"),
            "mask_threshold" | "mask_invert" => ("Mask", "Mask map"),
            "mask_random" | "mask_checker" | "mask_horizontal" | "mask_vertical" | "invert_mask" => ("Mask", "Procedural mask"),
            "scale_map_multiplier" | "rotation_map_multiplier" | "displacement_intensity" | "displacement_angle" | "color_map_multiplier" => ("Map inputs", ""),
            "seed" => ("Pattern", "Random seed"),
            _ => ("Other", ""),
        };
    }
    let n = name;
    if n.starts_with("seed") || n.ends_with("seed") {
        return ("Random", "");
    }
    if n.starts_with("disorder") || n.starts_with("anisotropy") || n == "speed" {
        return ("Disorder", "");
    }
    if n.starts_with("tile_") {
        return ("Tiling offset", "");
    }
    if n.contains("random") || n.contains("var") || n.contains("jitter") {
        return ("Variation", "");
    }
    if n.contains("opacity") || n.contains("blend") || n.contains("background") || n.contains("contrast") || n.contains("invert") || n.contains("clamp") || n.contains("gain") || n.contains("output") || n.contains("levels") {
        return ("Output", "");
    }
    if n.contains("angle") || n.contains("rotat") || n.contains("offset") || n.starts_with("pan") || n.contains("zoom") || n.contains("symm") || n.contains("mirror") || n.contains("skew") || n.contains("axis") {
        return ("Transform", "");
    }
    ("Main", "")
}

fn p_seed() -> PD {
    pi("seed", "Random seed", 0, 0, 9999)
}
fn p_con() -> PD {
    pf("contrast", "Contrast", 1.0, 0.0, 4.0)
}
fn p_inv() -> PD {
    pb("invert", "Invert", false)
}
fn p_dis() -> PD {
    pf("disorder", "Disorder", 0.0, 0.0, 1.0)
}

/// Editable parameters of each node type (defaults match the engine).
fn pdefs(kind: &str) -> Vec<PD> {
    let mut v = pdefs_base(kind);
    // Designer's shared noise block: Disorder, Disorder speed, Disorder anisotropy (+ angle), Tile offset
    let (aniso, tile, speed, dis) = match kind {
        "gaussian_noise" | "clouds" | "bnw_spots" | "gaussian_spots" | "directional_noise" | "cells" | "fbm" => (true, true, true, true),
        "perlin" => (false, true, true, true),
        "fractal_sum" | "waveform" => (false, false, true, true),
        _ => (false, false, false, false),
    };
    if dis || speed {
        let mut extra: Vec<PD> = Vec::new();
        if !v.iter().any(|p| p.name == "disorder") {
            extra.push(p_dis());
        }
        extra.push(pf("disorder_speed", "Disorder speed", 1.0, 0.0, 4.0));
        if aniso {
            extra.push(pf("anisotropy", "Disorder anisotropy", 0.0, 0.0, 1.0));
            extra.push(pf("anisotropy_angle", "Disorder anisotropy angle", 0.0, 0.0, 1.0));
        }
        if tile {
            extra.push(pf("tile_x", "Tile offset X", 0.0, -16.0, 16.0));
            extra.push(pf("tile_y", "Tile offset Y", 0.0, -16.0, 16.0));
        }
        // after the first of scale/count/etc., before seed/contrast/invert
        let at = v.iter().position(|p| matches!(p.name, "seed" | "contrast" | "invert")).unwrap_or(v.len());
        // keep an existing Disorder where it already is: move the rest in front of the seed
        for (i, e) in extra.into_iter().enumerate() {
            v.insert(at + i, e);
        }
    }
    if kind == "white_noise" {
        v.insert(0, pe("distribution", "Noise distribution", 0, &["Uniform", "Gaussian", "Triangle"]));
    }
    v
}

fn pdefs_base(kind: &str) -> Vec<PD> {
    match kind {
        "white_noise" => vec![p_seed()],
        "gaussian_noise" => vec![pi("scale", "Scale (grain)", 1, 1, 16), p_seed(), p_con(), p_inv()],
        "perlin" => vec![pi("scale", "Scale", 4, 1, 64), p_dis(), p_seed(), p_con(), p_inv()],
        "fbm" => vec![pi("scale", "Scale", 4, 1, 64), pi("octaves", "Octaves", 5, 1, 10), pf("gain", "Gain", 0.5, 0.0, 1.0), p_dis(), p_seed(), pe("mode", "Mode", 0, &["fBm", "Ridged", "Turbulence"]), p_con(), p_inv()],
        "clouds" => vec![pi("scale", "Scale", 3, 1, 64), pi("octaves", "Octaves", 6, 1, 10), pf("gain", "Gain", 0.6, 0.0, 1.0), p_dis(), p_seed(), p_con(), p_inv()],
        "voronoi" | "voronoi_fractal" => {
            let mut v = vec![
                pi("scale", "Scale", 6, 1, 64),
                pf("aspect", "Size Y / X", 1.0, 0.25, 4.0),
                pf("offset_x", "Offset X", 0.0, 0.0, 1.0),
                pf("offset_y", "Offset Y", 0.0, 0.0, 1.0),
                pf("jitter", "Disorder", 1.0, 0.0, 1.0),
                pf("distortion", "Distortion intensity", 0.0, 0.0, 2.0),
                pi("distortion_scale", "Distortion scale", 3, 1, 32),
                pe("mode", "Style", 0, &["F1 (distance)", "Random color", "Edge", "F2", "F2 - F1", "F1 * F2", "F1 / F2"]),
                pf("edge_thickness", "Edge thickness", 0.25, 0.01, 1.0),
                pf("rounded", "Rounded curve", 0.0, 0.0, 1.0),
                pe("distance_mode", "Distance mode", 0, &["Euclidean", "Manhattan", "Chebyshev", "Minkowski"]),
                pf("minkowski", "Minkowski number", 2.0, 0.25, 16.0),
                pf("distance_scale", "Distance scale", 1.0, 0.0, 8.0),
                pe("color_seed_mode", "Random color seed mode", 0, &["Global (node seed)", "Manual"]),
                pi("color_seed", "Random color seed", 0, 0, 9999),
                pb("invert", "Invert", false),
            ];
            if kind == "voronoi_fractal" {
                v.extend([
                    pi("min_level", "Min level", 0, 0, 8),
                    pi("max_level", "Max level", 3, 0, 8),
                    pf("roughness", "Roughness (Add)", 0.5, 0.0, 1.5),
                    pf("lacunarity", "Lacunarity", 2.0, 1.0, 4.0),
                    pf("opacity", "Global opacity", 1.0, 0.0, 1.0),
                    pe("blend", "Blend mode", 0, &["Add", "Max", "Min"]),
                ]);
            }
            v.push(p_seed());
            v
        }
        "bnw_spots" => vec![pe("variant", "Type", 0, &["Spots 1", "Spots 2", "Spots 3"]), pi("scale", "Scale", 8, 1, 64), p_dis(), p_seed(), p_con(), p_inv()],
        "crystal" => vec![pe("variant", "Type", 0, &["Crystal 1", "Crystal 2"]), pi("scale", "Scale", 6, 1, 64), p_dis(), p_seed(), p_con(), p_inv()],
        "directional_noise" => vec![pf("scale", "Scale", 6.0, 1.0, 32.0), pf("stretch", "Stretch", 8.0, 1.0, 32.0), pf("angle", "Angle", 0.0, 0.0, 1.0), p_seed(), p_con(), p_inv()],
        "grunge_map" => vec![pi("map", "Grunge map", 1, 1, 15), pf("balance", "Balance", 0.5, 0.0, 1.0), pf("contrast", "Contrast", 0.5, 0.0, 1.0), pf("brush_pattern", "Brush pattern", 0.0, 0.0, 1.0), pf("cracks", "Cracks (maps 11, 13)", 0.5, 0.0, 1.0), pf("disorder", "Disorder", 0.0, 0.0, 1.0), pb("invert", "Invert", false), p_seed()],
        "grunge" => vec![
            pe("variant", "Type", 0, &["Concrete", "Scratches", "Spots", "Smudges"]),
            pf("balance", "Balance", 0.5, 0.0, 1.0),
            pf("large_variation", "Large variation (Concrete)", 0.55, 0.0, 1.0),
            pf("base_noisiness", "Base noisiness (Concrete)", 0.45, 0.0, 1.0),
            pf("dirt_specks_opacity", "Dirt specks opacity (Concrete)", 0.0, 0.0, 1.0),
            pb("invert_dirt", "Invert dirt (Concrete)", false),
            pf("scratches_opacity", "Scratches opacity (Concrete)", 0.0, 0.0, 1.0),
            pf("sharpen", "Sharpen (Concrete)", 0.0, 0.0, 1.0),
            pi("quantity", "Scratch quantity (Scratches)", 3, 1, 32),
            pf("scratch_width", "Scratch width (Scratches)", 0.4, 0.05, 0.95),
            pf("scratch_length", "Scratch length (Scratches)", 0.45, 0.0, 0.9),
            pi("details", "Details (Spots)", 22, 2, 128),
            pf("coverage", "Coverage (Spots)", 0.35, 0.0, 0.9),
            pf("coverage_contrast", "Coverage contrast (Spots)", 0.25, 0.02, 1.0),
            p_dis(),
            p_seed(),
            p_con(),
            p_inv(),
        ],
        "plasma" => vec![pi("scale", "Scale", 3, 1, 8), p_dis(), p_seed(), p_con(), p_inv()],
        "gradient" => vec![pe("mode", "Type", 0, &["Linear", "Radial"]), pf("angle", "Angle", 0.0, 0.0, 1.0)],
        "gradient_axial" => vec![pf("angle", "Angle", 0.0, 0.0, 1.0)],
        "gradient_circular" => vec![pf("offset", "Offset", 0.0, 0.0, 1.0)],
        "circle" => vec![pf("radius", "Radius", 0.4, 0.0, 0.7), pf("softness", "Edge softness", 0.01, 0.0, 0.5)],
        "rect" => vec![pf("width", "Width", 0.6, 0.0, 1.0), pf("height", "Height", 0.4, 0.0, 1.0), pf("softness", "Edge softness", 0.01, 0.0, 0.5)],
        "polygon" => vec![pi("sides", "Sides", 5, 3, 32), pf("radius", "Radius", 0.4, 0.0, 0.7), pf("softness", "Edge softness", 0.01, 0.0, 0.5), pf("rotation", "Rotation", 0.0, 0.0, 1.0)],
        "shape" => vec![
            pe("shape", "Shape", 1, SHAPES),
            pf("size_x", "Width", 0.6, 0.0, 1.5),
            pf("size_y", "Height", 0.6, 0.0, 1.5),
            pf("rotation", "Rotation", 0.0, 0.0, 1.0),
            pf("pos_x", "Position X", 0.0, -0.5, 0.5),
            pf("pos_y", "Position Y", 0.0, -0.5, 0.5),
            pi("tiling", "Tiling", 1, 1, 16),
            pf("softness", "Edge softness", 0.02, 0.0, 1.0),
        ],
        "star" => vec![pi("points", "Points", 5, 3, 16), pf("inner", "Inner radius", 0.45, 0.0, 1.0), pf("size", "Size", 0.4, 0.0, 0.7), pf("softness", "Edge softness", 0.01, 0.0, 0.5), pf("rotation", "Rotation", 0.0, 0.0, 1.0)],
        "checker" => vec![pi("tiles", "Tiles", 8, 1, 64)],
        "bricks" => vec![
            pi("rows", "Rows", 8, 1, 32),
            pi("cols", "Columns", 4, 1, 32),
            pf("mortar", "Mortar", 0.03, 0.0, 0.2),
            pf("bevel", "Bevel", 0.05, 0.001, 0.5),
            pf("offset", "Row offset", 0.5, 0.0, 1.0),
            pf("lum_var", "Brick variation", 0.3, 0.0, 1.0),
            p_seed(),
        ],
        "stripes" => vec![pi("count", "Count", 8, 1, 64), pf("angle", "Angle", 0.0, 0.0, 1.0), pf("softness", "Edge softness", 0.1, 0.001, 0.5), pf("width", "Stripe width", 0.5, 0.0, 1.0), pe("tile", "Angle mode", 1, &["Exact angle (may not tile)", "Seamless (snap to tileable angle)"])],
        "weave" => vec![pi("count", "Count", 6, 1, 32), pf("width", "Thread width", 0.8, 0.1, 1.0)],
        "tile_sampler" => {
            let mut v = vec![pi("x_amount", "X amount", 8, 1, 64), pi("y_amount", "Y amount", 8, 1, 64)];
            v.extend(tile_common(false));
            v
        }
        "tile_generator" => {
            let mut v = vec![pi("x_amount", "X amount", 6, 1, 64), pi("y_amount", "Y amount", 6, 1, 64)];
            v.extend(tile_common(true));
            v
        }
        "splatter" => {
            let mut v = vec![pi("count", "Count", 40, 0, 500), pi("grid_number", "Grid number (0 = random)", 0, 0, 32), pf("scale", "Size", 0.1, 0.01, 0.5), pf("scale_var", "Size random", 0.3, 0.0, 1.0), pf("aspect", "Pattern height / width", 1.0, 0.1, 4.0)];
            v.extend([
                pf("rotation", "Rotation", 0.0, 0.0, 1.0),
                pf("rot_var", "Rotation random", 1.0, 0.0, 1.0),
                pe("pattern_rotation", "Pattern rotation", 0, ROT90),
                pe("symmetry_random", "Symmetry random", 0, SYMM),
                pf("lum_var", "Color random", 0.0, 0.0, 1.0),
                pf("disorder", "Disorder (grid jitter)", 1.0, 0.0, 1.0),
                pf("pan_x", "Pan X", 0.0, -1.0, 1.0),
                pf("pan_y", "Pan Y", 0.0, -1.0, 1.0),
                pf("gain", "Gain", 1.0, 0.1, 4.0),
                pf("output_min", "Output min", 0.0, 0.0, 1.0),
                pf("output_max", "Output max", 1.0, 0.0, 1.0),
                pf("background", "Background", 0.0, 0.0, 1.0),
                pe("blending", "Blending mode", 0, TILE_BLEND),
                pf("opacity", "Global opacity", 1.0, 0.0, 1.0),
            ]);
            v.extend(pattern_block());
            v.push(p_seed());
            v
        }
        "splatter_circular" => {
            let mut v = vec![
                pi("number", "Pattern amount", 12, 1, 256),
                pf("amount_random", "Amount random", 0.0, 0.0, 1.0),
                pi("amount_random_min", "Amount random min", 1, 1, 256),
                pi("rings", "Ring amount", 1, 1, 10),
                pf("radius", "Radius", 0.3, 0.0, 0.7),
                pf("radius_random", "Radius random", 0.0, 0.0, 1.0),
                pf("ring_radius_multiplier", "Ring radius multiplier", 0.5, 0.0, 2.0),
                pf("angle_random", "Angle random", 0.0, 0.0, 1.0),
                pf("spiral_factor", "Spiral factor", 0.0, -2.0, 2.0),
                pf("spread", "Spread", 1.0, 0.0, 1.0),
                pf("size", "Size", 0.1, 0.005, 0.5),
                pf("size_random", "Size random", 0.0, 0.0, 1.0),
                pf("scale_by_number", "Scale by pattern number", 0.0, 0.0, 1.0),
                pb("invert_number", "Invert pattern number", false),
                pf("scale_by_ring", "Scale by ring number", 0.0, 0.0, 1.0),
                pb("invert_ring", "Invert ring number", false),
                pf("rotation", "Pattern rotation", 0.0, 0.0, 1.0),
                pf("rotation_random", "Pattern rotation random", 0.0, 0.0, 1.0),
                pf("center_orientation", "Center orientation", 0.0, 0.0, 1.0),
                pf("ring_rotation", "Ring rotation", 0.0, -1.0, 1.0),
                pf("ring_rotation_random", "Ring rotation random", 0.0, 0.0, 1.0),
                pf("ring_rotation_offset", "Ring rotation offset", 0.0, 0.0, 1.0),
                pf("position_random", "Position random", 0.0, 0.0, 1.0),
                pe("symmetry_random", "Symmetry random", 0, SYMM),
                pf("color", "Color", 1.0, 0.0, 1.0),
                pf("luminance_random", "Luminance random", 0.0, 0.0, 1.0),
                pf("lum_by_number", "Luminance by pattern number", 0.0, 0.0, 1.0),
                pf("lum_by_ring", "Luminance by ring number", 0.0, 0.0, 1.0),
                pf("mask_random", "Random mask", 0.0, 0.0, 1.0),
                pf("background", "Background", 0.0, 0.0, 1.0),
                pe("blending", "Blending mode", 0, TILE_BLEND),
                pf("opacity", "Global opacity", 1.0, 0.0, 1.0),
            ];
            v.extend(pattern_block());
            v.push(p_seed());
            v
        }
        "sdf_shape" => vec![
            pe("shape", "Shape", 0, &["Circle", "Box", "Polygon", "Ring"]),
            pf("size", "Size", 0.25, 0.0, 0.5),
            pf("aspect", "Aspect", 1.0, 0.1, 3.0),
            pi("sides", "Sides", 5, 3, 12),
            pf("rotation", "Rotation", 0.0, 0.0, 1.0),
            pf("round", "Corner rounding", 0.0, 0.0, 0.3),
            pf("x", "Position X", 0.0, -0.5, 0.5),
            pf("y", "Position Y", 0.0, -0.5, 0.5),
            pf("range", "Distance range", 0.25, 0.02, 1.0),
        ],
        "sdf_op" => vec![pe("op", "Operation", 0, &["Union", "Subtract", "Intersect"]), pf("smooth", "Smoothing", 0.0, 0.0, 0.3), pf("range", "Distance range", 0.25, 0.02, 1.0)],
        "sdf_mask" => vec![pf("offset", "Offset", 0.0, -0.5, 0.5), pf("softness", "Edge softness", 0.02, 0.0, 0.5)],
        "sdf_form" => vec![
            pe("shape", "Shape", 0, &["Sphere", "Box", "Torus", "Cylinder"]),
            pf("size", "Size", 0.3, 0.05, 0.5),
            pi("repeat", "Repeat", 3, 1, 16),
            pf("round", "Rounding", 0.05, 0.0, 0.3),
            pf("scatter", "Size variation", 0.3, 0.0, 1.0),
            pf("height", "Height", 1.5, 0.1, 4.0),
            p_seed(),
        ],
        "input" => vec![pi("idx", "Input index", 0, 0, 3), pb("gray", "Grayscale", false)],
        "switch" => vec![pi("index", "Index", 0, 0, 3)],
        "blur" => vec![pf("radius", "Intensity", 4.0, 0.0, 64.0)],
        "blur_hq" => vec![pf("intensity", "Intensity", 6.0, 0.0, 64.0), pi("quality", "Quality", 1, 1, 4)],
        "directional_blur" => vec![pf("angle", "Angle", 0.0, 0.0, 1.0), pf("length", "Length", 0.05, 0.0, 0.3)],
        "sharpen" => vec![pf("radius", "Radius", 2.0, 0.0, 32.0), pf("amount", "Amount", 1.0, 0.0, 5.0)],
        "levels" => vec![
            pb("auto", "Auto levels", false),
            pf("low", "Input low", 0.0, 0.0, 1.0),
            pf("high", "Input high", 1.0, 0.0, 1.0),
            pf("gamma", "Gamma", 1.0, 0.1, 4.0),
            pf("out_low", "Output low", 0.0, 0.0, 1.0),
            pf("out_high", "Output high", 1.0, 0.0, 1.0),
        ],
        "adjust" => vec![pf("brightness", "Brightness", 0.0, -1.0, 1.0), pf("contrast", "Contrast", 1.0, 0.0, 3.0)],
        "threshold" => vec![pf("threshold", "Threshold", 0.5, 0.0, 1.0), pe("mode", "Mode", 0, &["Greater", "Greater or equal", "Lower", "Lower or equal"]), pf("softness", "Edge softness", 0.0, 0.0, 1.0)],
        "invert" => vec![pb("invert", "Invert", true)],
        "histogram_scan" => vec![pf("position", "Position", 0.5, 0.0, 1.0), pf("contrast", "Contrast", 0.5, 0.0, 1.0), pb("invert", "Invert", false)],
        "histogram_range" => vec![pf("position", "Position", 0.5, 0.0, 1.0), pf("range", "Range", 0.2, 0.0, 1.0), pf("contrast", "Contrast", 0.5, 0.0, 1.0), pb("invert", "Invert", false)],
        "quantize" => vec![pi("steps", "Steps", 4, 2, 32)],
        "channel" => vec![pe("ch", "Channel", 0, &["Red", "Green", "Blue", "Alpha"])],
        "tint" => vec![pf("r", "Red", 1.0, 0.0, 1.0), pf("g", "Green", 1.0, 0.0, 1.0), pf("b", "Blue", 1.0, 0.0, 1.0)],
        "colorize" => vec![pe("interp", "Interpolation", 0, &["Linear", "Smooth", "Step", "Smoother"]), pf("smoothness", "Smoothness", 1.0, 0.0, 1.0), pe("addressing", "Gradient addressing", 0, &["Clamp", "Repeat"]), pb("use_alpha", "Use gradient alpha", false)],
        "hsl" => vec![pf("hue", "Hue", 0.0, -1.0, 1.0), pf("sat", "Saturation", 1.0, 0.0, 2.0), pf("val", "Lightness", 1.0, 0.0, 2.0)],
        "transform" => vec![
            pf("offset_x", "Offset X", 0.0, -1.0, 1.0),
            pf("offset_y", "Offset Y", 0.0, -1.0, 1.0),
            pf("scale_x", "Scale X", 1.0, 0.1, 4.0),
            pf("scale_y", "Scale Y", 1.0, 0.1, 4.0),
            pf("rotation", "Rotation", 0.0, 0.0, 1.0),
            pe("tiling", "Tiling mode", 0, &["H and V tiling", "No tiling"]),
        ],
        "mirror" => vec![
            pe("axis", "Mode", 0, &["Mirror axis X", "Mirror axis Y", "Both axes", "Mirror corner"]),
            pf("offset_x", "Axis X offset", 0.5, 0.0, 1.0),
            pf("offset_y", "Axis Y offset", 0.5, 0.0, 1.0),
            pb("invert_x", "Invert axis X", false),
            pb("invert_y", "Invert axis Y", false),
            pe("corner", "Corner type", 0, &["Top left", "Top right", "Bottom left", "Bottom right"]),
        ],
        "warp" => vec![pf("intensity", "Intensity", 0.05, 0.0, 0.5), pe("filtering", "Input filtering", 2, &["Bilinear", "Nearest", "Bicubic"])],
        "directional_warp" => vec![pf("angle", "Warp angle", 0.0, 0.0, 1.0), pf("intensity", "Intensity", 0.1, 0.0, 0.5), pf("offset", "Intensity map offset", 0.5, 0.0, 1.0), pe("filtering", "Input filtering", 2, &["Bilinear", "Nearest", "Bicubic"])],
        "vector_warp" => vec![pf("intensity", "Intensity", 0.05, 0.0, 0.5), pe("directx", "Vector format", 0, &["OpenGL", "DirectX"])],
        "slope_blur" => vec![pi("samples", "Samples", 16, 1, 64), pf("intensity", "Intensity", 0.1, 0.0, 1.0), pe("mode", "Mode", 0, &["Blur", "Min", "Max"])],
        "swirl" => vec![pf("amount", "Amount", 0.3, -2.0, 2.0), pf("radius", "Radius", 0.5, 0.0, 1.0)],
        "highpass" => vec![pf("radius", "Radius", 6.0, 0.0, 64.0)],
        "clamp" => vec![pf("low", "Min", 0.0, 0.0, 1.0), pf("high", "Max", 1.0, 0.0, 1.0), pb("apply_alpha", "Apply to alpha", false)],
        "blend" => vec![
            pe("mode", "Blending mode", 0, BLEND_MODES),
            pf("opacity", "Opacity", 1.0, 0.0, 1.0),
            pe("alpha_mode", "Alpha blending", 1, &["Use source alpha", "Ignore alpha", "Straight alpha blending", "Premultiplied alpha blending"]),
            pf("crop_x0", "Cropping left", 0.0, 0.0, 1.0),
            pf("crop_y0", "Cropping top", 0.0, 0.0, 1.0),
            pf("crop_x1", "Cropping right", 1.0, 0.0, 1.0),
            pf("crop_y1", "Cropping bottom", 1.0, 0.0, 1.0),
        ],
        "normal" => vec![pf("strength", "Intensity", 2.0, 0.0, 10.0), pb("directx", "DirectX format", false), pb("alpha_from_input", "Fill alpha with input", false)],
        "normal_blend" => vec![pf("opacity", "Opacity", 1.0, 0.0, 1.0)],
        "normal_combine" => vec![pe("technique", "Technique", 2, &["Whiteout", "Channel mixer", "Detail oriented"])],
        "ao" => vec![
            pe("mode", "Method", 0, &["Blur (legacy)", "Horizon (HBAO)"]),
            pf("radius", "Spread (legacy)", 6.0, 0.0, 64.0),
            pf("intensity", "Height depth (legacy)", 2.0, 0.0, 10.0),
            pf("hbao_radius", "Radius", 1.0, 0.0, 1.0),
            pf("hbao_intensity", "Intensity", 1.0, 0.0, 2.0),
            pe("quality", "Quality", 1, &["4 samples", "8 samples", "16 samples"]),
            pb("use_world_units", "Use world units", false),
            pf("height_depth", "Height depth (screen)", 0.1, 0.0, 1.0),
            pf("surface_size", "Surface size (cm)", 300.0, 1.0, 1000.0),
            pf("height_depth_cm", "Height depth (cm)", 30.0, 0.0, 1000.0),
        ],
        "quantize_color" => vec![pi("color_amount", "Max. colour amount", 32, 1, 128), pf("contour_smoothing", "Contour smoothing", 0.0, 0.0, 8.0), pf("dithering", "Dithering", 0.0, 0.0, 1.0), pe("dithering_pattern", "Dithering pattern", 0, &["Noise", "Bayer"]), pb("ignore_alpha", "Ignore alpha", false), pe("distance_color_space", "Distance colour space", 0, &["Lab", "RGB"])],
        "curvature" => vec![pe("mode", "Input", 0, &["Normal map", "Height map (legacy)"]), pe("directx", "Normal format", 0, &["OpenGL", "DirectX"]), pf("intensity", "Intensity", 4.0, 0.0, 10.0), pf("radius", "Radius (height input)", 3.0, 0.0, 64.0)],
        "curvature_smooth" => vec![pf("smoothing", "Smoothing", 2.0, 0.0, 32.0), pf("intensity", "Intensity", 6.0, 0.0, 20.0)],
        "edge_detect" => vec![pe("mode", "Method", 0, &["Edge lines", "Sobel (legacy)"]), pf("edge_width", "Edge width", 2.0, 1.0, 16.0), pf("edge_roundness", "Edge roundness", 4.0, 0.0, 16.0), pb("invert", "Invert", false), pf("tolerance", "Tolerance", 0.0, 0.0, 1.0), pf("intensity", "Intensity (Sobel)", 2.0, 0.0, 10.0)],
        "emboss" => vec![pf("angle", "Angle", 0.125, 0.0, 1.0), pf("intensity", "Intensity", 4.0, 0.0, 10.0)],
        "flood_fill" => vec![pf("threshold", "Threshold", 0.05, 0.0, 1.0)],
        "flood_fill_random" => vec![pf("min", "Minimum", 0.0, 0.0, 1.0), pf("max", "Maximum", 1.0, 0.0, 1.0), p_seed()],
        "flood_fill_color" => vec![p_seed()],
        "flood_fill_gradient" => vec![pf("angle", "Angle", 0.0, 0.0, 1.0), pf("variation", "Angle variation", 0.0, 0.0, 1.0), p_seed()],
        "bevel" => vec![
            pf("distance", "Distance", 0.05, 0.0, 0.5),
            pf("smoothing", "Smoothing", 0.5, 0.0, 1.0),
            pf("threshold", "Shape threshold", 0.5, 0.0, 1.0),
            pb("use_as_normal", "Output as normal", false),
            pf("normal_intensity", "Normal intensity", 4.0, 0.0, 50.0),
            pe("corner", "Corner type", 1, &["Round", "Angular"]),
            pe("directx", "Normal format", 0, &["OpenGL", "DirectX"]),
        ],
        "distance" => vec![pf("distance", "Distance range", 0.1, 0.0, 1.0), pf("threshold", "Shape threshold", 0.5, 0.0, 1.0), pe("mode", "Mode", 0, &["Inside", "Outside", "Both"])],
        "color_match" => vec![
            pe("method", "Method", 0, &["Statistics transfer", "Colour swap"]),
            pf("strength", "Strength", 1.0, 0.0, 1.0),
            pe("source_mode", "Source colour mode", 0, &["Average", "Colour", "Input"]),
            pf("src_r", "Source red", 0.5, 0.0, 1.0), pf("src_g", "Source green", 0.5, 0.0, 1.0), pf("src_b", "Source blue", 0.5, 0.0, 1.0),
            pe("target_mode", "Target colour mode", 0, &["Colour", "Input"]),
            pf("target_r", "Target red", 0.5, 0.0, 1.0), pf("target_g", "Target green", 0.5, 0.0, 1.0), pf("target_b", "Target blue", 0.5, 0.0, 1.0),
            pb("custom_variation", "Custom colour variation", false),
            pf("variation_hue", "Hue", 0.0, 0.0, 360.0), pf("variation_chroma", "Chrominance", 1.0, 0.0, 2.0), pf("variation_luma", "Luminance", 1.0, 0.0, 2.0),
            pb("use_mask", "Use mask", true),
            pf("mask_hue_range", "Mask hue range", 30.0, 0.0, 180.0), pf("mask_chroma_range", "Mask chroma range", 0.5, 0.0, 1.0), pf("mask_luma_range", "Mask luma range", 0.5, 0.0, 1.0),
            pf("mask_smoothness", "Mask smoothness", 0.3, 0.0, 2.0),
        ],
        "height_blend" => vec![pf("opacity", "Opacity", 1.0, 0.0, 1.0), pf("contrast", "Height transition", 0.5, 0.0, 2.0), pe("output", "Output", 0, &["Height", "Blend mask"])],
        "make_it_tile_photo" => vec![pf("width", "Mask size H", 0.6, 0.1, 1.0), pf("width_v", "Mask size V", 0.6, 0.1, 1.0)],
        "make_it_tile_patch" => vec![pi("patches", "Patches", 6, 2, 32), pf("softness", "Softness", 0.5, 0.0, 1.0), p_seed()],
        "edge_wear" => vec![pf("amount", "Amount", 0.5, 0.0, 1.0), pf("width", "Edge width", 2.0, 0.0, 16.0), pf("contrast", "Contrast", 1.0, 0.1, 4.0)],
        "dirt" | "dust" => vec![pf("amount", "Amount", 0.5, 0.0, 1.0), pf("contrast", "Contrast", 1.0, 0.1, 4.0)],
        "non_uniform_blur" => vec![pf("intensity", "Intensity", 8.0, 0.0, 64.0)],
        "radial_blur" => vec![pe("mode", "Blur type", 0, &["Spin", "Zoom (legacy)"]), pi("samples", "Samples", 64, 1, 128), pf("angle", "Angle", 0.125, 0.0, 0.5), pf("x", "Center X", 0.0, -0.5, 0.5), pf("y", "Center Y", 0.0, -0.5, 0.5), pf("amount", "Zoom amount", 0.1, 0.0, 1.0), pf("radius", "Zoom radius", 0.5, 0.01, 0.5)],
        "anisotropic_blur" => vec![pf("intensity", "Intensity", 8.0, 0.0, 64.0), pf("anisotropy", "Anisotropy", 0.8, 0.0, 1.0), pf("angle", "Angle", 0.0, 0.0, 1.0)],
        "symmetry" => vec![
            pe("axis", "Symmetry mode", 0, &["Mirror X", "Mirror Y", "Mirror X / Y", "Diagonal left", "Diagonal right", "Eight-fold"]),
            pe("transfer", "Transfer mode", 0, &["Copy", "Add", "Subtract", "Multiply", "Add sub", "Max", "Min"]),
            pf("offset", "Offset", 0.0, -0.5, 0.5),
            pb("flip", "Flip side", false),
        ],
        "skew" => vec![pi("skew_x", "Skew X", 1, -8, 8), pi("skew_y", "Skew Y", 0, -8, 8), pe("align", "Align", 1, &["Center", "Top left", "Bottom right"])],
        "auto_levels" => vec![pb("per_channel", "Per channel", false)],
        "channel_mixer" => vec![
            pf("rr", "Red from red", 1.0, -2.0, 2.0), pf("rg", "Red from green", 0.0, -2.0, 2.0), pf("rb", "Red from blue", 0.0, -2.0, 2.0),
            pf("gr", "Green from red", 0.0, -2.0, 2.0), pf("gg", "Green from green", 1.0, -2.0, 2.0), pf("gb", "Green from blue", 0.0, -2.0, 2.0),
            pf("br", "Blue from red", 0.0, -2.0, 2.0), pf("bg", "Blue from green", 0.0, -2.0, 2.0), pf("bb", "Blue from blue", 1.0, -2.0, 2.0),
            pb("mono", "Monochrome", false),
        ],
        "color_to_mask" => vec![pf("r", "Red", 1.0, 0.0, 1.0), pf("g", "Green", 0.0, 0.0, 1.0), pf("b", "Blue", 0.0, 0.0, 1.0), pf("range", "Range", 0.2, 0.0, 1.0), pf("softness", "Softness", 0.1, 0.0, 1.0)],
        "replace_color" => vec![
            pf("src_r", "Source red", 1.0, 0.0, 1.0), pf("src_g", "Source green", 0.0, 0.0, 1.0), pf("src_b", "Source blue", 0.0, 0.0, 1.0),
            pf("dst_r", "Target red", 0.0, 0.0, 1.0), pf("dst_g", "Target green", 0.0, 0.0, 1.0), pf("dst_b", "Target blue", 1.0, 0.0, 1.0),
            pf("range", "Range", 0.25, 0.0, 1.0), pf("softness", "Softness", 0.15, 0.0, 1.0),
        ],
        "pow" => vec![pf("exponent", "Exponent", 2.0, 0.0, 8.0)],
        "histogram_shift" => vec![pf("position", "Position", 0.5, 0.0, 1.0)],
        "grayscale_conversion" => vec![pf("r", "Red weight", 0.3, 0.0, 1.0), pf("g", "Green weight", 0.59, 0.0, 1.0), pf("b", "Blue weight", 0.11, 0.0, 1.0)],
        "cells" => vec![pe("variant", "Type", 0, &["Cells 1", "Cells 2", "Cells 3", "Cells 4"]), pi("scale", "Scale", 8, 1, 64), pf("jitter", "Randomness", 1.0, 0.0, 1.0), pf("luminance_random", "Luminance random (Type 1)", 0.0, 0.0, 1.0), pe("color_source", "Color source (Type 2)", 0, &["Random", "Pseudorandom", "Image input"]), pi("pseudorandom_seed", "Pseudorandom seed", 0, 0, 9999), pf("edge_width", "Edge width (Type 3)", 0.25, 0.01, 1.0), pf("hardness", "Hardness (Type 4)", 2.0, 0.1, 16.0), p_seed(), p_con(), p_inv()],
        "fractal_sum" => vec![
            pe("mode", "Type", 0, &["Base", "Turbulence", "Creased"]), pi("scale", "Scale", 2, 1, 32), pi("min_level", "Min level", 0, 0, 8), pi("max_level", "Max level", 6, 0, 8),
            pf("roughness", "Roughness", 0.5, 0.0, 1.5), p_seed(), p_con(), p_inv(),
        ],
        "waveform" => vec![
            pe("shape", "Waveform", 0, &["Sine", "Triangle", "Saw", "Square"]), pi("count", "Count", 8, 1, 64), pf("angle", "Angle", 0.0, 0.0, 1.0),
            pf("warp", "Warp", 0.3, 0.0, 4.0), pi("warp_scale", "Warp scale", 4, 1, 32), p_seed(), p_con(), p_inv(),
        ],
        "gaussian_spots" => vec![pi("scale", "Scale", 8, 1, 64), pf("size", "Size", 0.5, 0.05, 1.0), pf("size_variation", "Size variation", 0.5, 0.0, 1.0), p_seed(), p_con(), p_inv()],
        "normal_invert" => vec![pb("invert_x", "Invert red (X)", false), pb("invert_y", "Invert green (Y)", true), pb("invert_z", "Invert blue (Z)", false), pb("invert_a", "Invert alpha", false)],
        "normal_vector_rotation" => vec![pf("angle", "Rotation angle", 0.0, 0.0, 1.0), pe("directx", "Normal format", 0, &["OpenGL", "DirectX"])],
        "normal_to_height" => vec![pf("relief_balance", "Relief balance", 0.5, 0.0, 1.0), pe("directx", "Normal format", 0, &["OpenGL", "DirectX"]), pf("opacity", "Global opacity", 1.0, 0.0, 1.0)],
        "non_uniform_directional_warp" => vec![pf("intensity", "Intensity", 0.1, -1.0, 1.0), pf("angle", "Angle", 0.0, 0.0, 1.0)],
        "multi_directional_warp" => vec![pf("intensity", "Intensity", 0.1, -1.0, 1.0), pe("directions", "Directions", 2, &["1", "2", "4"]), pf("angle", "Warp angle", 0.0, 0.0, 1.0), pe("mode", "Mode", 0, &["Average", "Max", "Min", "Chain"])],
        "triangle_grid" => vec![
            pe("style", "Style", 0, &["Squares (legacy)", "Jittered mesh"]),
            pi("count", "Count (legacy)", 8, 1, 64),
            pf("border", "Border (legacy)", 0.06, 0.0, 0.5),
            pf("variation", "Variation (legacy)", 1.0, 0.0, 1.0),
            pi("x_amount", "X amount", 16, 1, 64),
            pi("y_amount", "Y amount", 16, 1, 64),
            pf("random_position_multiplier", "Random position", 1.0, 0.0, 2.0),
            pf("quincunx_offset_x", "Quincunx offset X", 0.0, 0.0, 1.0),
            pf("quincunx_offset_y", "Quincunx offset Y", 0.0, 0.0, 1.0),
            pe("triangulation", "Triangulation", 0, &["Fixed", "Alternate", "Random"]),
            pe("output", "Output", 0, &["Smooth heights", "Flat triangles", "Edge distance"]),
            pf("height_random", "Height random", 1.0, 0.0, 1.0),
            pb("thin_lines", "Thin lines", false),
            pf("distance_to_edge_thickness", "Line thickness", 0.025, 0.0, 0.1),
            pf("thin_line_color", "Line colour", 1.0, 0.0, 1.0),
            pf("thin_line_background", "Line background", 0.0, 0.0, 1.0),
            p_seed(),
            p_con(),
            p_inv(),
        ],
        "weave_generator" => vec![pi("tile_x", "Tile X", 1, 1, 20), pi("tile_y", "Tile Y", 1, 1, 20), pi("weave", "Weave", 2, 1, 10), pf("shape", "Shape", 0.5, 0.0, 1.0), pf("gap_x", "Gap X", 0.1, 0.0, 0.9), pf("gap_y", "Gap Y", 0.1, 0.0, 0.9), p_con(), p_inv()],
        "starburst" => vec![
            pi("count", "Count", 3, 1, 32), pi("rays", "Rays", 8, 2, 64), pf("sharpness", "Sharpness", 2.0, 0.1, 16.0), pf("size", "Size", 1.0, 0.05, 2.0),
            pf("rotation", "Rotation", 0.0, 0.0, 1.0), pf("rotation_random", "Rotation random", 0.0, 0.0, 1.0), p_seed(), p_con(), p_inv(),
        ],
        "gradient_reflected" => vec![pi("count", "Count", 1, 1, 16), pf("angle", "Angle", 0.0, 0.0, 1.0), p_con(), p_inv()],
        "shape_stroke" => vec![pf("width", "Width", 8.0, 0.5, 256.0), pf("offset", "Offset", 0.0, -1.0, 1.0), pf("softness", "Softness", 0.5, 0.0, 1.0), pf("threshold", "Shape threshold", 0.5, 0.0, 1.0), pf("opacity", "Opacity", 1.0, 0.0, 1.0), pf("color_r", "Outline color R", 1.0, 0.0, 1.0), pf("color_g", "Outline color G", 1.0, 0.0, 1.0), pf("color_b", "Outline color B", 1.0, 0.0, 1.0)],
        "shape_glow" => vec![pf("width", "Width", 16.0, 0.5, 256.0), pf("softness", "Softness", 0.5, 0.0, 1.0), pf("intensity", "Intensity", 1.0, 0.0, 4.0), pf("threshold", "Shape threshold", 0.5, 0.0, 1.0), pf("spread", "Spread", 0.0, 0.0, 1.0), pf("opacity", "Opacity", 1.0, 0.0, 1.0), pf("color_r", "Glow color R", 1.0, 0.0, 1.0), pf("color_g", "Glow color G", 1.0, 0.0, 1.0), pf("color_b", "Glow color B", 1.0, 0.0, 1.0)],
        "sobel" => vec![pf("intensity", "Intensity", 1.0, 0.0, 16.0)],
        "glow" => vec![pf("width", "Glow size", 8.0, 0.0, 128.0), pf("intensity", "Glow amount", 1.0, 0.0, 8.0), pf("clear", "Clear amount", 0.0, 0.0, 1.0), pf("glow_r", "Glow color R", 1.0, 0.0, 1.0), pf("glow_g", "Glow color G", 1.0, 0.0, 1.0), pf("glow_b", "Glow color B", 1.0, 0.0, 1.0)],
        "mosaic" => vec![pf("intensity", "Intensity (with map)", 1.0, 0.0, 1.0), pi("count", "Cells per side (no map)", 16, 1, 128)],
        "shape_drop_shadow" => vec![pf("angle", "Angle", 0.125, 0.0, 1.0), pf("distance", "Distance", 0.03, 0.0, 0.5), pf("blur", "Blur", 6.0, 0.0, 128.0), pf("opacity", "Opacity", 0.8, 0.0, 1.0), pf("spread", "Spread", 0.0, 0.0, 1.0)],
        _ => vec![],
    }
}

// ---------------------------------------------------------------- colour wheel
fn hsv_color32(h: f32, s: f32, v: f32) -> Color32 {
    let (r, g, b) = ops::hsv_to_rgb(h, s, v);
    Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

/// Hue ring + saturation/value square. Returns true when the colour changed.
fn color_wheel(ui: &mut egui::Ui, id: Id, rgb: &mut [f32; 3]) -> bool {
    let size = 164.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click_and_drag());
    let c = rect.center();
    let (r_out, r_in) = (size * 0.5 - 2.0, size * 0.5 - 18.0);
    let (mut h, mut sv, mut vv) = ops::rgb_to_hsv(rgb[0], rgb[1], rgb[2]);
    let hue_id = id.with("hue");
    if sv < 0.01 || vv < 0.01 {
        if let Some(hs) = ui.data(|d| d.get_temp::<f32>(hue_id)) {
            h = hs;
        }
    }
    let half = r_in * 0.68;
    let sq = Rect::from_center_size(c, vec2(half * 2.0, half * 2.0));
    let mode_id = id.with("mode");
    let mut changed = false;
    if resp.drag_started() || resp.clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            let d = (p - c).length();
            let mode = if d > r_in - 4.0 && d < r_out + 8.0 { 1u8 } else if sq.expand(4.0).contains(p) { 2 } else { 0 };
            ui.data_mut(|dd| dd.insert_temp(mode_id, mode));
        }
    }
    if resp.dragged() || resp.clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            match ui.data(|d| d.get_temp::<u8>(mode_id)).unwrap_or(0) {
                1 => {
                    h = ((p - c).y.atan2((p - c).x) / std::f32::consts::TAU).rem_euclid(1.0);
                    changed = true;
                }
                2 => {
                    sv = ((p.x - sq.min.x) / sq.width()).clamp(0.0, 1.0);
                    vv = 1.0 - ((p.y - sq.min.y) / sq.height()).clamp(0.0, 1.0);
                    changed = true;
                }
                _ => {}
            }
        }
    }
    if changed {
        ui.data_mut(|d| d.insert_temp(hue_id, h));
        let (r, g, b) = ops::hsv_to_rgb(h, sv, vv);
        *rgb = [r, g, b];
    }
    let painter = ui.painter_at(rect.expand(2.0));
    // ring
    let mut mesh = egui::Mesh::default();
    let segs = 120;
    for i in 0..segs {
        let (a0, a1) = (i as f32 / segs as f32 * std::f32::consts::TAU, (i + 1) as f32 / segs as f32 * std::f32::consts::TAU);
        let col = hsv_color32((i as f32 + 0.5) / segs as f32, 1.0, 1.0);
        let base = mesh.vertices.len() as u32;
        for (a, r) in [(a0, r_in), (a0, r_out), (a1, r_out), (a1, r_in)] {
            mesh.colored_vertex(c + vec2(a.cos(), a.sin()) * r, col);
        }
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base, base + 2, base + 3);
    }
    painter.add(egui::Shape::mesh(mesh));
    // saturation / value square
    let mut mesh = egui::Mesh::default();
    let n = 12;
    for yi in 0..=n {
        for xi in 0..=n {
            let (s, v) = (xi as f32 / n as f32, 1.0 - yi as f32 / n as f32);
            mesh.colored_vertex(pos2(sq.min.x + s * sq.width(), sq.min.y + (1.0 - v) * sq.height()), hsv_color32(h, s, v));
        }
    }
    for yi in 0..n {
        for xi in 0..n {
            let a = (yi * (n + 1) + xi) as u32;
            let b = a + 1;
            let c2 = a + (n + 1) as u32;
            mesh.add_triangle(a, b, c2);
            mesh.add_triangle(b, b + (n + 1) as u32, c2);
        }
    }
    painter.add(egui::Shape::mesh(mesh));
    // markers
    let ha = h * std::f32::consts::TAU;
    painter.circle(c + vec2(ha.cos(), ha.sin()) * ((r_in + r_out) * 0.5), 5.5, hsv_color32(h, 1.0, 1.0), Stroke::new(2.0_f32, Color32::WHITE));
    painter.circle(pos2(sq.min.x + sv * sq.width(), sq.min.y + (1.0 - vv) * sq.height()), 5.0, Color32::from_rgb((rgb[0] * 255.0) as u8, (rgb[1] * 255.0) as u8, (rgb[2] * 255.0) as u8), Stroke::new(2.0_f32, if vv > 0.55 { Color32::BLACK } else { Color32::WHITE }));
    changed
}

fn rgb_hex(c: [f32; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", (c[0].clamp(0.0, 1.0) * 255.0 + 0.5) as u8, (c[1].clamp(0.0, 1.0) * 255.0 + 0.5) as u8, (c[2].clamp(0.0, 1.0) * 255.0 + 0.5) as u8)
}

fn parse_hex(s: &str) -> Option<[f32; 3]> {
    let t = s.trim().trim_start_matches('#');
    if t.len() != 6 || !t.is_ascii() {
        return None;
    }
    let p = |i: usize| u8::from_str_radix(&t[i..i + 2], 16).ok().map(|v| v as f32 / 255.0);
    Some([p(0)?, p(2)?, p(4)?])
}

// ---------------------------------------------------------------- gradient editor
fn grad_at(stops: &[(f32, [f32; 3])], t: f32) -> [f32; 3] {
    if stops.is_empty() {
        return [t; 3];
    }
    if t <= stops[0].0 {
        return stops[0].1;
    }
    for i in 1..stops.len() {
        if t <= stops[i].0 {
            let (a, b) = (stops[i - 1], stops[i]);
            let f = (t - a.0) / (b.0 - a.0).max(1e-6);
            return [a.1[0] + (b.1[0] - a.1[0]) * f, a.1[1] + (b.1[1] - a.1[1]) * f, a.1[2] + (b.1[2] - a.1[2]) * f];
        }
    }
    stops[stops.len() - 1].1
}

fn c32(c: [f32; 3]) -> Color32 {
    Color32::from_rgb((c[0].clamp(0.0, 1.0) * 255.0) as u8, (c[1].clamp(0.0, 1.0) * 255.0) as u8, (c[2].clamp(0.0, 1.0) * 255.0) as u8)
}

fn stops_from_node(n: &Node) -> Vec<(f32, [f32; 3])> {
    let mut v: Vec<(f32, [f32; 3])> = (0..16)
        .filter_map(|i| {
            let p = *n.nums.get(&format!("p{i}"))?;
            let g = |c: &str| n.nums.get(&format!("{c}{i}")).copied().unwrap_or(0.0);
            Some((p, [g("r"), g("g"), g("b")]))
        })
        .collect();
    v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    if v.is_empty() {
        v = vec![(0.0, [0.0; 3]), (1.0, [1.0; 3])];
    }
    v
}

fn gradient_editor(ui: &mut egui::Ui, id: Id, stops: &mut Vec<(f32, [f32; 3])>) -> bool {
    let mut changed = false;
    let w = ui.available_width();
    let (area, _) = ui.allocate_exact_size(vec2(w, 46.0), Sense::hover());
    let bar = Rect::from_min_size(area.min + vec2(7.0, 0.0), vec2(area.width() - 14.0, 22.0));
    let painter = ui.painter_at(area);
    let segs = 72;
    for i in 0..segs {
        let t = (i as f32 + 0.5) / segs as f32;
        let r = Rect::from_min_max(pos2(bar.min.x + bar.width() * i as f32 / segs as f32, bar.min.y), pos2(bar.min.x + bar.width() * (i + 1) as f32 / segs as f32 + 1.0, bar.max.y));
        painter.rect_filled(r, 0.0, c32(grad_at(stops, t)));
    }
    painter.rect_stroke(bar, 0.0, Stroke::new(1.0_f32, Color32::from_gray(20)));
    let mut sel = ui.data(|d| d.get_temp::<usize>(id)).unwrap_or(0).min(stops.len() - 1);
    let bar_resp = ui.interact(bar, id.with("bar"), Sense::click());
    let mut grabbed = false;
    for i in 0..stops.len() {
        let x = bar.min.x + stops[i].0 * bar.width();
        let mr = Rect::from_center_size(pos2(x, bar.max.y + 11.0), vec2(14.0, 20.0));
        let r = ui.interact(mr, id.with(("stop", i)), Sense::click_and_drag());
        if r.clicked() || r.drag_started() {
            sel = i;
            grabbed = true;
        }
        if r.dragged() {
            if let Some(p) = r.interact_pointer_pos() {
                let lo = if i > 0 { stops[i - 1].0 + 0.002 } else { 0.0 };
                let hi = if i + 1 < stops.len() { stops[i + 1].0 - 0.002 } else { 1.0 };
                stops[i].0 = ((p.x - bar.min.x) / bar.width()).clamp(lo, hi.max(lo));
                changed = true;
                grabbed = true;
            }
        }
        let edge = if sel == i { Color32::from_rgb(255, 170, 60) } else { Color32::from_gray(200) };
        painter.add(egui::Shape::convex_polygon(vec![pos2(x, bar.max.y + 1.0), pos2(x - 5.0, bar.max.y + 8.0), pos2(x + 5.0, bar.max.y + 8.0)], edge, Stroke::NONE));
        let sq = Rect::from_center_size(pos2(x, bar.max.y + 15.0), vec2(11.0, 9.0));
        painter.rect_filled(sq, 1.0, c32(stops[i].1));
        painter.rect_stroke(sq, 1.0, Stroke::new(1.5_f32, edge));
    }
    if bar_resp.clicked() && !grabbed && stops.len() < 16 {
        if let Some(p) = bar_resp.interact_pointer_pos() {
            let t = ((p.x - bar.min.x) / bar.width()).clamp(0.0, 1.0);
            let col = grad_at(stops, t);
            let at = stops.iter().position(|s| s.0 > t).unwrap_or(stops.len());
            stops.insert(at, (t, col));
            sel = at;
            changed = true;
        }
    }
    sel = sel.min(stops.len() - 1);
    ui.data_mut(|d| d.insert_temp(id, sel));
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        ui.label(format!("Stop {}", sel + 1));
        let mut pos = stops[sel].0;
        if ui.add(egui::DragValue::new(&mut pos).speed(0.005).range(0.0..=1.0).fixed_decimals(3)).changed() {
            let lo = if sel > 0 { stops[sel - 1].0 + 0.002 } else { 0.0 };
            let hi = if sel + 1 < stops.len() { stops[sel + 1].0 - 0.002 } else { 1.0 };
            stops[sel].0 = pos.clamp(lo, hi.max(lo));
            changed = true;
        }
        if ui.add_enabled(stops.len() > 2, egui::Button::new("Remove")).clicked() {
            stops.remove(sel);
            changed = true;
        }
    });
    let mut col = stops[sel].1;
    if color_wheel(ui, id.with("wheel"), &mut col) {
        stops[sel].1 = col;
        changed = true;
    }
    changed
}

// ---------------------------------------------------------------- property panels
fn row_label(ui: &mut egui::Ui, text: &str) {
    ui.allocate_ui_with_layout(vec2(112.0, 18.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.label(text);
    });
}

impl App {
    fn inspector(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().slider_width = (ui.available_width() - 190.0).clamp(70.0, 220.0);
        if let Some(ci) = self.sel_comment.filter(|c| *c < self.g.comments.len()) {
            ui.strong("Comment frame");
            let mut c = self.g.comments[ci].clone();
            let mut changed = ui.text_edit_singleline(&mut c.text).changed();
            ui.horizontal(|ui| {
                for (i, col) in COMMENT_COLORS.iter().enumerate() {
                    let (r, resp) = ui.allocate_exact_size(vec2(22.0, 18.0), Sense::click());
                    ui.painter().rect_filled(r, Rounding::same(3.0), Color32::from_rgb(col[0], col[1], col[2]));
                    if c.color as usize == i {
                        ui.painter().rect_stroke(r, Rounding::same(3.0), Stroke::new(2.0_f32, Color32::WHITE));
                    }
                    if resp.clicked() {
                        c.color = i as u8;
                        changed = true;
                    }
                }
            });
            if changed {
                if !self.edit_open {
                    self.snap();
                    self.edit_open = true;
                }
                self.g.comments[ci] = c;
            }
            if ui.button("Delete frame").clicked() {
                self.acts.push(Act::Delete);
            }
            return;
        }
        if let Some(id) = self.active.filter(|a| self.sel.contains(a) && self.g.nodes.contains_key(a)) {
            self.node_props(ui, id);
            return;
        }
        if !self.sel.is_empty() {
            ui.label(format!("{} nodes selected", self.sel.len()));
            return;
        }
        self.graph_props(ui);
        self.published_params(ui);
    }

    fn published_params(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new(egui::RichText::new("PUBLISHED PARAMETERS").strong()).default_open(false).show(ui, |ui| {
            if self.g.exposed.is_empty() {
                ui.small("Click 'P' next to a parameter to publish it. Published parameters can be set when rendering a .mca package.");
            }
            let mut remove = None;
            let mut changed = false;
            let mut list = self.g.exposed.clone();
            for (i, e) in list.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    changed |= ui.add(egui::TextEdit::singleline(&mut e.label).desired_width(110.0)).changed();
                    ui.label(format!("#{}.{}", e.node, e.param));
                    changed |= ui.add(egui::DragValue::new(&mut e.min).speed(0.01)).changed();
                    changed |= ui.add(egui::DragValue::new(&mut e.max).speed(0.01)).changed();
                    if ui.small_button("x").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if changed || remove.is_some() {
                if !self.edit_open {
                    self.snap();
                    self.edit_open = true;
                }
                if let Some(i) = remove {
                    list.remove(i);
                }
                self.g.exposed = list;
            }
        });
    }

    fn node_props(&mut self, ui: &mut egui::Ui, id: u32) {
        let node = self.g.nodes[&id].clone();
        let kind = node.kind.clone();
        let title = if kind == "output" { usage_label(node.strs.get("usage").map(String::as_str).unwrap_or("basecolor")) } else { label(&kind) };
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(6.0, 20.0), Sense::hover());
            ui.painter().rect_filled(r, 1.0, kind_color(&kind));
            ui.label(egui::RichText::new(format!("{title}  #{id}")).size(15.0).strong());
        });
        if let Some(sp) = spec(&kind) {
            ui.small(sp.doc);
        }
        if let Some(e) = self.node_err.get(&id) {
            ui.colored_label(Color32::from_rgb(255, 120, 110), e);
        }
        let mut nums: Vec<(String, f32)> = Vec::new();
        let mut strs: Vec<(String, Option<String>)> = Vec::new();
        let mut expose: Option<(String, f32, f32)> = None;
        let get = |k: &str, d: f32| node.nums.get(k).copied().unwrap_or(d);

        // ---- inputs overview
        let mut sl = slots(&kind);
        // a graph node only has inputs when that graph defines Input nodes
        if kind == "subgraph" && !node.strs.get("graph").and_then(|g| self.g.subgraphs.get(g)).map(|sg| sg.nodes.values().any(|x| x.kind == "input")).unwrap_or(true) {
            sl.clear();
        }
        if !sl.is_empty() {
            egui::CollapsingHeader::new(egui::RichText::new("INPUTS").strong()).default_open(true).show(ui, |ui| {
                for (name, req) in &sl {
                    match node.ins.get(*name) {
                        Some(src) => {
                            ui.label(format!("{name}  <-  #{src}"));
                        }
                        None => {
                            ui.colored_label(Color32::from_gray(140), format!("{name}  (not connected{})", if *req { ", required" } else { "" }));
                        }
                    }
                }
            });
        }

        // ---- output size (like Designer: relative to input / relative to parent / absolute, plus a power-of-two modifier)
        if !matches!(kind.as_str(), "comment" | "frame" | "dot") {
            egui::CollapsingHeader::new(egui::RichText::new("OUTPUT SIZE").strong()).default_open(true).show(ui, |ui| {
                let mode = get("osize_mode", 0.0).round() as i32;
                let names = ["Relative to input", "Relative to parent", "Absolute"];
                ui.horizontal(|ui| {
                    row_label(ui, "Size mode");
                    egui::ComboBox::from_id_salt(("osize_mode", id)).selected_text(names[mode.clamp(0, 2) as usize]).show_ui(ui, |ui| {
                        for (i, n) in names.iter().enumerate() {
                            if ui.selectable_label(mode == i as i32, *n).clicked() {
                                nums.push(("osize_mode".into(), i as f32));
                            }
                        }
                    });
                });
                let parent = self.g.size;
                if mode == 2 {
                    let e = get("osize_abs", 9.0).round().clamp(4.0, 13.0) as i32;
                    ui.horizontal(|ui| {
                        row_label(ui, "Size");
                        egui::ComboBox::from_id_salt(("osize_abs", id)).selected_text(format!("{0} x {0}", 1usize << e)).show_ui(ui, |ui| {
                            for k in 4..=13 {
                                if ui.selectable_label(e == k, format!("{0} x {0}", 1usize << k)).clicked() {
                                    nums.push(("osize_abs".into(), k as f32));
                                }
                            }
                        });
                    });
                } else {
                    let mut rel = get("osize_rel", 0.0).round() as i32;
                    ui.horizontal(|ui| {
                        row_label(ui, "Modifier");
                        if ui.add(egui::Slider::new(&mut rel, -12..=12)).changed() {
                            nums.push(("osize_rel".into(), rel as f32));
                        }
                    });
                    let shown = self.nominal_node_size(id);
                    ui.small(format!("Result: {0} x {0}  (graph {1} x {1})", shown, parent));
                }
                ui.small("A lower size computes faster: give heavy nodes (AO, blurs, noises) a smaller size.");
            });
        }

        egui::CollapsingHeader::new(egui::RichText::new("PARAMETERS").strong()).default_open(true).show(ui, |ui| {
            // ---- node-specific editors
            match kind.as_str() {
                "color" => {
                    let gray = get("gray", 0.0) > 0.5;
                    ui.horizontal(|ui| {
                        row_label(ui, "Mode");
                        if ui.selectable_label(!gray, "Color").clicked() {
                            nums.push(("gray".into(), 0.0));
                        }
                        if ui.selectable_label(gray, "Grayscale").clicked() {
                            nums.push(("gray".into(), 1.0));
                        }
                    });
                    if gray {
                        let mut v = get("v", 0.5);
                        ui.horizontal(|ui| {
                            row_label(ui, "Value");
                            if ui.add(egui::Slider::new(&mut v, 0.0..=1.0)).changed() {
                                nums.push(("v".into(), v));
                            }
                        });
                    } else {
                        let mut rgb = [get("r", 0.5), get("g", 0.5), get("b", 0.5)];
                        let mut ch = false;
                        ui.horizontal(|ui| {
                            ui.add_space(4.0);
                            ch |= color_wheel(ui, Id::new(("cwheel", id)), &mut rgb);
                            ui.vertical(|ui| {
                                let (r, _) = ui.allocate_exact_size(vec2(54.0, 36.0), Sense::hover());
                                ui.painter().rect_filled(r, 3.0, c32(rgb));
                                ui.painter().rect_stroke(r, 3.0, Stroke::new(1.0_f32, Color32::from_gray(15)));
                                let mut hex = rgb_hex(rgb);
                                if ui.add(egui::TextEdit::singleline(&mut hex).desired_width(60.0)).changed() {
                                    if let Some(c) = parse_hex(&hex) {
                                        rgb = c;
                                        ch = true;
                                    }
                                }
                            });
                        });
                        for (i, nm) in ["Red", "Green", "Blue"].iter().enumerate() {
                            ui.horizontal(|ui| {
                                row_label(ui, nm);
                                ch |= ui.add(egui::Slider::new(&mut rgb[i], 0.0..=1.0)).changed();
                            });
                        }
                        if ch {
                            nums.push(("r".into(), rgb[0]));
                            nums.push(("g".into(), rgb[1]));
                            nums.push(("b".into(), rgb[2]));
                        }
                    }
                    let mut a = get("a", 1.0);
                    ui.horizontal(|ui| {
                        row_label(ui, "Alpha");
                        if ui.add(egui::Slider::new(&mut a, 0.0..=1.0)).changed() {
                            nums.push(("a".into(), a));
                        }
                    });
                }
                "colorize" => {
                    let mut stops = stops_from_node(&node);
                    if gradient_editor(ui, Id::new(("grad", id)), &mut stops) {
                        for i in 0..16 {
                            for k in ["p", "r", "g", "b"] {
                                nums.push((format!("{k}{i}"), f32::NAN));
                            }
                        }
                        for (i, (p, c)) in stops.iter().enumerate() {
                            nums.push((format!("p{i}"), *p));
                            nums.push((format!("r{i}"), c[0]));
                            nums.push((format!("g{i}"), c[1]));
                            nums.push((format!("b{i}"), c[2]));
                        }
                    }
                }
                "output" => {
                    let usage = node.strs.get("usage").cloned().unwrap_or_else(|| "basecolor".into());
                    let names: Vec<&str> = OUTPUT_NAMES.to_vec();
                    ui.horizontal(|ui| {
                        row_label(ui, "Usage");
                        egui::ComboBox::from_id_salt(("usage", id)).selected_text(usage_label(&usage)).show_ui(ui, |ui| {
                            for n in &names {
                                if ui.selectable_label(usage == *n, usage_label(n)).clicked() {
                                    strs.push(("usage".into(), Some(n.to_string())));
                                }
                            }
                        });
                    });
                    let mut ident = node.strs.get("identifier").cloned().unwrap_or_default();
                    ui.horizontal(|ui| {
                        row_label(ui, "Identifier");
                        if ui.add(egui::TextEdit::singleline(&mut ident).hint_text(usage.as_str()).desired_width(f32::INFINITY)).changed() {
                            let clean: String = ident.chars().filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-').collect();
                            strs.push(("identifier".into(), if clean.is_empty() { None } else { Some(clean) }));
                        }
                    });
                    ui.small(format!("Exports as {}.png at {0}x{0}. The 3D view uses this output as the '{}' channel.", self.g.size, usage));
                }
                "pixel_processor" => {
                    ui.small("vars: u v x y w h r g b l   funcs: sin cos abs sqrt min max pow floor fract clamp mix step smoothstep noise rand tex atan2 exp log");
                    for key in ["expr", "expr_g", "expr_b", "expr_a"] {
                        let mut s = node.strs.get(key).cloned().unwrap_or_default();
                        if ui.add(egui::TextEdit::singleline(&mut s).hint_text(key).desired_width(f32::INFINITY)).changed() {
                            strs.push((key.to_string(), if s.trim().is_empty() { None } else { Some(s.replace('"', "")) }));
                        }
                    }
                }
                "bitmap" => {
                    let file = node.strs.get("file").cloned().unwrap_or_default();
                    ui.label(if file.is_empty() { "(no file)".to_string() } else { file });
                    if ui.button("Browse PNG...").clicked() {
                        if let Some(p) = rfd::FileDialog::new().add_filter("PNG image", &["png"]).pick_file() {
                            strs.push(("file".into(), Some(p.to_string_lossy().to_string())));
                        }
                    }
                }
                "subgraph" => {
                    if let Some(g) = node.strs.get("graph") {
                        ui.label(format!("Instance of '{g}'"));
                        if ui.button("Open subgraph").clicked() {
                            self.acts.push(Act::EnterSub(g.clone()));
                        }
                        if let Some(sub) = self.g.subgraphs.get(g) {
                            let outs = sub.output_nodes();
                            if outs.len() > 1 {
                                let cur = node.strs.get("output").cloned().unwrap_or_default();
                                let shown = if cur.is_empty() { outs.iter().find(|o| o.0 == "out").or(outs.first()).map(|o| o.1.clone()).unwrap_or_default() } else { cur };
                                ui.horizontal(|ui| {
                                    row_label(ui, "Output");
                                    egui::ComboBox::from_id_salt(("subout", id)).selected_text(shown.clone()).show_ui(ui, |ui| {
                                        for o in &outs {
                                            if ui.selectable_label(o.1 == shown, &o.1).clicked() {
                                                strs.push(("output".into(), Some(o.1.clone())));
                                            }
                                        }
                                    });
                                });
                            }
                        }
                    }
                }
                _ => {}
            }

            // ---- generic parameter rows
            let mut rows: Vec<(String, String, PK)> = Vec::new();
            let exposable = kind != "subgraph";
            if kind == "subgraph" {
                if let Some(sub) = node.strs.get("graph").and_then(|g| self.g.subgraphs.get(g)) {
                    for (nid, inner) in &sub.nodes {
                        for pd in pdefs(&inner.kind) {
                            let pk = match (pd.k, inner.nums.get(pd.name).copied()) {
                                (PK::F { lo, hi, .. }, Some(d)) => PK::F { def: d, lo, hi },
                                (PK::I { lo, hi, .. }, Some(d)) => PK::I { def: d as i32, lo, hi },
                                (PK::E { opts, .. }, Some(d)) => PK::E { def: d as i32, opts },
                                (PK::B { .. }, Some(d)) => PK::B { def: d > 0.5 },
                                (k, None) => k,
                            };
                            rows.push((format!("{nid}.{}", pd.name), format!("{} #{nid}: {}", label(&inner.kind), pd.label), pk));
                        }
                    }
                }
            } else {
                rows = pdefs(&kind).into_iter().map(|pd| (pd.name.to_string(), pd.label.to_string(), pd.k)).collect();
            }
            if rows.is_empty() && !matches!(kind.as_str(), "color" | "colorize" | "output" | "pixel_processor" | "bitmap" | "subgraph" | "dot" | "to_gray" | "merge") {
                ui.small("No adjustable parameters.");
            }
            let mut draw = |ui: &mut egui::Ui, i: usize| {
                let (key, lbl, pk) = &rows[i];
                let cur = node.nums.get(key).copied();
                ui.horizontal(|ui| {
                    row_label(ui, lbl);
                    match *pk {
                        PK::F { def, lo, hi } => {
                            let mut v = cur.unwrap_or(def);
                            if ui.add(egui::Slider::new(&mut v, lo..=hi)).changed() {
                                nums.push((key.clone(), v));
                            }
                        }
                        PK::I { def, lo, hi } => {
                            let mut v = cur.unwrap_or(def as f32);
                            if ui.add(egui::Slider::new(&mut v, lo as f32..=hi as f32).step_by(1.0).fixed_decimals(0)).changed() {
                                nums.push((key.clone(), v.round()));
                            }
                        }
                        PK::E { def, opts } => {
                            let idx = (cur.unwrap_or(def as f32).round() as i32).clamp(0, opts.len() as i32 - 1) as usize;
                            egui::ComboBox::from_id_salt((id, key.as_str())).selected_text(opts[idx]).show_ui(ui, |ui| {
                                for (i, o) in opts.iter().enumerate() {
                                    if ui.selectable_label(i == idx, *o).clicked() {
                                        nums.push((key.clone(), i as f32));
                                    }
                                }
                            });
                        }
                        PK::B { def } => {
                            let mut b = cur.map(|v| v > 0.5).unwrap_or(def);
                            if ui.checkbox(&mut b, "").changed() {
                                nums.push((key.clone(), if b { 1.0 } else { 0.0 }));
                            }
                        }
                    }
                    if exposable {
                        let is_exp = self.g.exposed.iter().any(|e| e.node == id && e.param == *key);
                        let (lo, hi) = match *pk {
                            PK::F { lo, hi, .. } => (lo, hi),
                            PK::I { lo, hi, .. } => (lo as f32, hi as f32),
                            PK::E { opts, .. } => (0.0, opts.len() as f32 - 1.0),
                            PK::B { .. } => (0.0, 1.0),
                        };
                        if ui.selectable_label(is_exp, "P").on_hover_text("Publish this parameter (settable in .mca packages)").clicked() {
                            expose = Some((key.clone(), lo, hi));
                        }
                    }
                });
            };
            // many parameters: sort them into categories and sub categories (a flat list is fine for short ones)
            if rows.len() >= 8 && kind != "subgraph" {
                let mut cats: Vec<(&'static str, Vec<(&'static str, Vec<usize>)>)> = Vec::new();
                for (i, (key, _, _)) in rows.iter().enumerate() {
                    let (c, sc) = param_group(&kind, key);
                    let ci = match cats.iter().position(|x| x.0 == c) {
                        Some(ci) => ci,
                        None => {
                            cats.push((c, Vec::new()));
                            cats.len() - 1
                        }
                    };
                    let subs = &mut cats[ci].1;
                    match subs.iter().position(|x| x.0 == sc) {
                        Some(si) => subs[si].1.push(i),
                        None => subs.push((sc, vec![i])),
                    }
                }
                for (ci, (cat, subs)) in cats.iter().enumerate() {
                    egui::CollapsingHeader::new(egui::RichText::new(*cat).strong()).id_salt(("pcat", id, *cat)).default_open(ci < 2).show(ui, |ui| {
                        for (sub, idx) in subs {
                            if sub.is_empty() {
                                for &i in idx {
                                    draw(ui, i);
                                }
                            } else {
                                egui::CollapsingHeader::new(*sub).id_salt(("psub", id, *cat, *sub)).default_open(true).show(ui, |ui| {
                                    for &i in idx {
                                        draw(ui, i);
                                    }
                                });
                            }
                        }
                    });
                }
            } else {
                for i in 0..rows.len() {
                    draw(ui, i);
                }
            }
        });

        if !nums.is_empty() || !strs.is_empty() {
            if !self.edit_open {
                self.snap();
                self.edit_open = true;
            }
            if let Some(n) = self.g.nodes.get_mut(&id) {
                for (k, v) in nums {
                    if v.is_nan() {
                        n.nums.remove(&k);
                    } else {
                        n.nums.insert(k, v);
                    }
                }
                for (k, v) in strs {
                    match v {
                        Some(s) => {
                            n.strs.insert(k, s);
                        }
                        None => {
                            n.strs.remove(&k);
                        }
                    }
                }
            }
            self.dirty = true;
        }
        if let Some((key, lo, hi)) = expose {
            self.snap();
            if let Some(p) = self.g.exposed.iter().position(|e| e.node == id && e.param == key) {
                self.g.exposed.remove(p);
            } else {
                self.g.exposed.push(Exposed { label: format!("{} {} {}", label(&kind), id, key), node: id, param: key, min: lo, max: hi });
            }
        }
    }
}

/// Starting values a node gets when it is created in the editor, chosen to match the defaults Designer's own
/// library nodes start with. They are written into the node, so graphs saved earlier (which rely on the
/// engine's own fallbacks) keep rendering exactly as before.
pub fn new_node_defaults(kind: &str) -> &'static [(&'static str, f32)] {
    match kind {
        "polygon" => &[("sides", 6.0)],
        "triangle_grid" => &[("style", 1.0)],
        "color_match" => &[("method", 1.0)],
        "ao" => &[("mode", 1.0)],
        "tile_sampler" => &[("x_amount", 16.0), ("y_amount", 16.0)],
        "tile_generator" => &[("x_amount", 10.0), ("y_amount", 10.0)],
        "splatter_circular" => &[("number", 10.0), ("radius", 0.25), ("center_orientation", 1.0)],
        "splatter" => &[("grid_number", 4.0)],
        "histogram_range" => &[("range", 0.5)],
        "pow" => &[("exponent", 4.0)],
        "shape_drop_shadow" => &[("angle", 0.25), ("distance", 0.02), ("opacity", 0.5)],
        _ => &[],
    }
}

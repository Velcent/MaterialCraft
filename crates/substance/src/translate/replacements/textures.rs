//! The texture entries of the replacement table: Substance noise, grunge, spot, cell and pattern textures
//! MaterialCraft replaces with its own procedural textures ([`materialcraft::proc_textures`]), with the
//! translation of their parameters. The looks are look-alikes (judged by histogram and spectrum), so every
//! entry is an approximation; parameters without a counterpart are left out.

use super::Replacement;
use super::params::{Ours, Painter};

const LOOK_ALIKE: &str = "MaterialCraft look-alike: similar statistics, not the same pixels";

macro_rules! entry {
    ($painter:expr, $label:expr, $ours:expr, $translate:expr) => {
        Replacement {
            painter: $painter,
            label: $label,
            generator: $ours,
            note: LOOK_ALIKE,
            translate: $translate,
        }
    };
}

pub static TEXTURES: &[Replacement] = &[
    entry!(
        &["ratio_clouds_1", "clouds_1"],
        "Clouds 1 → MaterialCraft clouds soft",
        "clouds_1",
        noise
    ),
    entry!(
        &["ratio_clouds_2", "clouds_2"],
        "Clouds 2 → MaterialCraft clouds",
        "clouds_2",
        noise
    ),
    entry!(
        &["ratio_clouds_3", "clouds_3"],
        "Clouds 3 → MaterialCraft clouds fine",
        "clouds_3",
        noise
    ),
    entry!(
        &[
            "ratio_perlin_noise",
            "perlin_noise",
            "ratio_perlin_noise_zoom",
            "perlin_noise_zoom"
        ],
        "Perlin Noise → MaterialCraft perlin",
        "perlin",
        perlin
    ),
    entry!(
        &["ratio_fractal_sum_1", "ratio_fractal_sum_2"],
        "Fractal Sum → MaterialCraft fractal sum",
        "fractal_sum",
        noise
    ),
    entry!(
        &["ratio_moisture_noise", "moisture_noise"],
        "Moisture Noise → MaterialCraft moisture",
        "moisture",
        noise
    ),
    entry!(
        &[
            "ratio_anisotropic_noise",
            "ratio_directional_noise_2",
            "ratio_directional_noise_3",
            "ratio_directional_noise_4",
            "directionnal_noise"
        ],
        "Anisotropic / Directional Noise → MaterialCraft anisotropic noise",
        "anisotropic",
        anisotropic
    ),
    entry!(
        &["ratio_wood_fibers_1", "ratio_messy_fibers_1"],
        "Fibers → MaterialCraft fibers",
        "fibers",
        noise
    ),
    entry!(
        &["white_noise", "ratio_white_noise"],
        "White Noise → MaterialCraft white noise",
        "white_noise",
        noise
    ),
    entry!(
        &["ratio_grunge_map_001", "grunge_map_001"],
        "Grunge Map 001 → MaterialCraft grunge 1",
        "grunge_01",
        grunge
    ),
    entry!(
        &["ratio_grunge_map_002", "grunge_map_002"],
        "Grunge Map 002 → MaterialCraft grunge 2",
        "grunge_02",
        grunge
    ),
    entry!(
        &["ratio_grunge_map_003", "grunge_map_003"],
        "Grunge Map 003 → MaterialCraft grunge 3",
        "grunge_03",
        grunge
    ),
    entry!(
        &["ratio_grunge_map_004", "grunge_map_004"],
        "Grunge Map 004 → MaterialCraft grunge 4",
        "grunge_04",
        grunge
    ),
    entry!(
        &["ratio_grunge_map_005", "grunge_map_005"],
        "Grunge Map 005 → MaterialCraft grunge 5",
        "grunge_05",
        grunge
    ),
    entry!(
        &["ratio_grunge_map_006", "grunge_map_006"],
        "Grunge Map 006 → MaterialCraft grunge 6",
        "grunge_06",
        grunge
    ),
    entry!(
        &["ratio_grunge_map_007", "grunge_map_007"],
        "Grunge Map 007 → MaterialCraft grunge 7",
        "grunge_07",
        grunge
    ),
    entry!(
        &["ratio_grunge_map_008", "grunge_map_008"],
        "Grunge Map 008 → MaterialCraft grunge 8",
        "grunge_08",
        grunge
    ),
    entry!(
        &["ratio_grunge_map_009", "grunge_map_009"],
        "Grunge Map 009 → MaterialCraft grunge 9",
        "grunge_09",
        grunge
    ),
    entry!(
        &["ratio_grunge_map_010", "grunge_map_010"],
        "Grunge Map 010 → MaterialCraft grunge 10",
        "grunge_10",
        grunge
    ),
    entry!(
        &["ratio_grunge_map_011", "grunge_map_011"],
        "Grunge Map 011 → MaterialCraft grunge 11",
        "grunge_11",
        grunge
    ),
    entry!(
        &["ratio_grunge_map_012", "grunge_map_012"],
        "Grunge Map 012 → MaterialCraft grunge 12",
        "grunge_12",
        grunge
    ),
    entry!(
        &["ratio_grunge_map_013", "grunge_map_013"],
        "Grunge Map 013 → MaterialCraft grunge 13",
        "grunge_13",
        grunge
    ),
    entry!(
        &["ratio_grunge_map_014", "grunge_map_014"],
        "Grunge Map 014 → MaterialCraft grunge 14",
        "grunge_14",
        grunge
    ),
    entry!(
        &["ratio_grunge_map_015", "grunge_map_015"],
        "Grunge Map 015 → MaterialCraft grunge 15",
        "grunge_15",
        grunge
    ),
    entry!(
        &["ratio_dirt_1", "ratio_dirt_3", "ratio_dirt_5", "dirt_1"],
        "Dirt 1/3/5 → MaterialCraft dirt soft",
        "dirt_soft",
        noise
    ),
    entry!(
        &["grunge_rust_fine"],
        "Grunge Rust Fine → MaterialCraft rust fine",
        "rust_fine",
        noise
    ),
    entry!(
        &["ratio_grunge_leaks", "grunge_leak_small"],
        "Grunge Leaks → MaterialCraft leaks",
        "leaks",
        grunge
    ),
    entry!(
        &["grunge_plaster_cracked"],
        "Grunge Plaster Cracked → MaterialCraft plaster cracks",
        "plaster_cracks",
        grunge
    ),
    entry!(
        &["grunge_concrete", "grunge_concrete_dirty"],
        "Grunge Concrete → MaterialCraft concrete",
        "concrete",
        noise
    ),
    entry!(
        &[
            "ratio_grunge_scratches_rough",
            "grunge_scratches_rough",
            "grunge_scratches_dirty",
            "grunge_scratches_dusty",
            "grunge_scratches_2",
            "ratio_directional_scratches"
        ],
        "Grunge Scratches → MaterialCraft scratches",
        "scratches",
        scratches
    ),
    entry!(
        &["ratio_grunge_spots"],
        "Grunge Spots → MaterialCraft grunge spots",
        "grunge_spots",
        noise
    ),
    entry!(
        &["ratio_bnw_spots_1", "bnw_spots_1"],
        "BnW Spots 1 → MaterialCraft spots round",
        "spots_1",
        noise
    ),
    entry!(
        &["ratio_bnw_spots_2", "bnw_spots_2"],
        "BnW Spots 2 → MaterialCraft spots large",
        "spots_2",
        noise
    ),
    entry!(
        &["ratio_bnw_spots_3", "bnw_spots_3"],
        "BnW Spots 3 → MaterialCraft spots grainy",
        "spots_3",
        noise
    ),
    entry!(
        &["ratio_gaussian_spots_1", "gaussian_spots_1"],
        "Gaussian Spots 1 → MaterialCraft soft spots",
        "gaussian_spots_1",
        noise
    ),
    entry!(
        &["ratio_gaussian_spots_2", "gaussian_spots_2"],
        "Gaussian Spots 2 → MaterialCraft soft spots dense",
        "gaussian_spots_2",
        noise
    ),
    entry!(
        &["ratio_cells_1"],
        "Cells 1 → MaterialCraft cells soft",
        "cells_1",
        cells
    ),
    entry!(
        &["ratio_cells_2"],
        "Cells 2 → MaterialCraft cell borders",
        "cells_2",
        cells
    ),
    entry!(
        &["ratio_cells_3"],
        "Cells 3 → MaterialCraft cells hard",
        "cells_3",
        cells
    ),
    entry!(
        &["ratio_cells_4"],
        "Cells 4 → MaterialCraft cells flat",
        "cells_4",
        cells
    ),
    entry!(
        &["ratio_crystal_1"],
        "Crystal 1 → MaterialCraft crystal",
        "crystal_1",
        noise
    ),
    entry!(
        &["ratio_crystal_2"],
        "Crystal 2 → MaterialCraft crystal facets",
        "crystal_2",
        noise
    ),
    entry!(
        &["ratio_stripes"],
        "Stripes → MaterialCraft stripes",
        "stripes",
        stripes
    ),
    entry!(
        &["ratio_checker_1"],
        "Checker 1 → MaterialCraft checker",
        "checker",
        checker
    ),
    entry!(
        &["weave_fibers"],
        "Weave Fibers → MaterialCraft weave with gaps",
        "weave_gaps",
        noise
    ),
];

/// What every texture takes: seed, tone, histogram scan, disorder and scale.
fn common(p: &Painter) -> Ours {
    // Painter stores the second disorder ("Disorder") in percent
    let disorder = p
        .f("disorder")
        .filter(|d| *d > 0.0)
        .or_else(|| p.f("Disorder").map(|d| d / 100.0));
    Ours::default()
        .num("seed", p.f("$randomseed"))
        .num("balance", p.first(&["balance", "Balance"]))
        .num("contrast", p.first(&["contrast", "Contrast"]))
        .num("invert", p.first(&["invert", "Invert"]))
        .num("position", p.first(&["histogram_position", "Histogram_Position"]))
        .num("range", p.first(&["historgram_contrast", "Histogram_contrast"]))
        .num("disorder", disorder.map(|d| d.clamp(0.0, 1.0)))
}

fn noise(p: &Painter) -> Vec<(String, String)> {
    common(p).num("scale", p.f("scale")).done()
}

/// Painter's Perlin scale counts lattice cells; ours multiplies 8 cells.
fn perlin(p: &Painter) -> Vec<(String, String)> {
    common(p)
        .num("scale", p.f("scale").map(|s| (s / 8.0).round().max(1.0)))
        .done()
}

fn grunge(p: &Painter) -> Vec<(String, String)> {
    common(p)
        .num("brush", p.f("brush_pattern"))
        .num("cracks", p.f("cracks"))
        .done()
}

fn anisotropic(p: &Painter) -> Vec<(String, String)> {
    common(p)
        .num("x_amount", p.f("X_Amount"))
        .num("y_amount", p.f("Y_Amount"))
        .num("smoothness", p.first(&["Smoothness", "smoothness"]))
        .num("rotate", p.f("rotate"))
        .done()
}

fn scratches(p: &Painter) -> Vec<(String, String)> {
    common(p)
        .num("quantity", p.first(&["Scratch_Quantity", "scratch_quantity"]))
        .num("scratch_width", p.f("scratch_width"))
        .num("scratch_length", p.first(&["Scratch_Length", "scratch_length"]))
        .done()
}

fn cells(p: &Painter) -> Vec<(String, String)> {
    common(p)
        .num("scale", p.f("scale").map(|s| (s / 8.0).round().max(1.0)))
        .num("edge_width", p.f("edge_width"))
        .done()
}

fn stripes(p: &Painter) -> Vec<(String, String)> {
    common(p)
        .num("count", p.f("Stripes"))
        .num("width", p.f("Width"))
        .num("softness", p.f("Softness"))
        .num("shift", p.f("Shift"))
        .done()
}

fn checker(p: &Painter) -> Vec<(String, String)> {
    common(p).num("tiles", p.f("Tiling")).done()
}

//! The entries of the replacement table, one per Substance procedural MaterialCraft replaces, with the
//! translation of its parameters into those of our generator (see `materialcraft::generators` for what each
//! parameter of ours does). Parameters without a counterpart are left out and listed in the entry's note.

use super::Replacement;
use super::params::{Ours, Painter};
use materialcraft::generators as g;

pub static TABLE: &[Replacement] = &[
    Replacement {
        painter: &["mg_mask_editor", "MaskEditor"],
        label: "Mask Editor → MaterialCraft mask editor",
        generator: g::MASK_EDITOR,
        note: "texture inputs become MaterialCraft grunge looks; micro details and per-axis blend modes are left out",
        translate: mask_editor,
    },
    Replacement {
        painter: &["mg_dirt"],
        label: "Dirt → MaterialCraft dirt",
        generator: g::DIRT,
        note: "built-in grunge instead of Painter's; custom grunge and micro details are left out",
        translate: dirt,
    },
    Replacement {
        painter: &["mg_ambient_occlusion"],
        label: "Ambient Occlusion → MaterialCraft ambient occlusion",
        generator: g::AMBIENT_OCCLUSION,
        note: "texture input becomes a MaterialCraft grunge look; micro details are left out",
        translate: ambient_occlusion,
    },
    Replacement {
        painter: &["mg_metal_edge_wear"],
        label: "Metal Edge Wear → MaterialCraft metal edge wear",
        generator: g::METAL_EDGE_WEAR,
        note: "built-in grunge instead of Painter's; custom grunge and micro details are left out",
        translate: metal_edge_wear,
    },
    Replacement {
        painter: &["mg_curvature"],
        label: "Curvature → MaterialCraft curvature",
        generator: g::CURVATURE,
        note: "texture input becomes a MaterialCraft grunge look; micro details are left out",
        translate: curvature,
    },
    Replacement {
        painter: &["mg_mask_builder"],
        label: "Mask Builder (legacy) → MaterialCraft mask builder",
        generator: g::MASK_BUILDER,
        note: "scratches, scatter and custom grunges are left out",
        translate: mask_builder,
    },
    Replacement {
        painter: &["mg_uv_border_distance"],
        label: "UV Border Distance → MaterialCraft UV border distance",
        generator: g::UV_BORDER_DISTANCE,
        note: "islands from the surface map or the baked position map",
        translate: uv_border_distance,
    },
];

/// Our grunge look (1..15) for what feeds a texture input, by the kind of pattern its name suggests.
fn look(source: &str) -> f32 {
    let s = source.to_ascii_lowercase();
    let looks: [(&str, f32); 8] = [
        ("crack", 10.0),
        ("scratch", 5.0),
        ("rust", 9.0),
        ("spot", 12.0),
        ("cloud", 7.0),
        ("moisture", 7.0),
        ("leak", 2.0),
        ("dirt", 1.0),
    ];
    looks.iter().find(|(k, _)| s.contains(k)).map_or(1.0, |(_, l)| *l)
}

/// A texture part: copied only when a procedural feeds the input (a uniform or empty input adds nothing).
/// The input becomes our procedural texture standing in for it (`<ours>_texture`) where there is one, and a
/// grunge look by number (`<ours>_look`) for older readers and for the rest.
fn texture(o: Ours, p: &Painter, input: &str, ours: &str, theirs: &[(&str, &[&str])]) -> Ours {
    let Some(source) = p
        .input(input)
        .filter(|s| !matches!(*s, "uniform" | "bitmap" | "anchor"))
    else {
        return o;
    };
    let key = |k: &str| format!("{ours}_{k}");
    let o = theirs.iter().fold(o, |o, (k, names)| o.num(&key(k), p.first(names)));
    let o = o.num(&key("look"), Some(look(source)));
    match super::texture_for(source) {
        Some(id) => o.text(&key("texture"), id),
        None => o,
    }
}

fn finish(o: Ours, p: &Painter) -> Ours {
    o.nums(
        p,
        &[
            ("invert", "global_invert"),
            ("blur", "global_blur"),
            ("balance", "global_balance"),
            ("contrast", "global_contrast"),
        ],
    )
}

fn grunge_common(o: Ours, p: &Painter) -> Ours {
    o.nums(
        p,
        &[
            ("triplanar", "Use_Triplanar"),
            ("triplanar_contrast", "Triplanar_Blending_Contrast"),
            ("seed", "$randomseed"),
            ("invert", "invert"),
        ],
    )
}

fn dirt(p: &Painter) -> Vec<(String, String)> {
    let o = Ours::default().nums(
        p,
        &[
            ("level", "dirt_level"),
            ("contrast", "dirt_contrast"),
            ("grunge_amount", "grunge_amount"),
            ("grunge_scale", "Grunge_Scale"),
            ("edge_masking", "edges_masking"),
        ],
    );
    grunge_common(o, p).done()
}

fn metal_edge_wear(p: &Painter) -> Vec<(String, String)> {
    let o = Ours::default().nums(
        p,
        &[
            ("level", "Wear_Level"),
            ("contrast", "Wear_Contrast"),
            ("grunge_amount", "Grunge_Amount"),
            ("grunge_scale", "grunge_scale"),
            ("smoothness", "Edges_Smoothness"),
            ("ao_masking", "Ambient_Occlusion_Masking"),
            ("curvature_weight", "Curvature_Weight"),
        ],
    );
    grunge_common(o, p).done()
}

const TEXTURE_NEW: &[(&str, &[&str])] = &[
    ("opacity", &["texture_opacity"]),
    ("blend", &["texture_blendmode"]),
    ("scale", &["texture_scale"]),
    ("contrast", &["texture_contrast"]),
    ("brightness", &["texture_brightness"]),
    ("invert", &["texutre_invert", "texture_invert"]),
    ("triplanar", &["texture_triplanar"]),
    ("triplanar_contrast", &["texture_triplanar_contrast"]),
];

fn ambient_occlusion(p: &Painter) -> Vec<(String, String)> {
    let o = Ours::default().nums(
        p,
        &[
            ("ao_invert", "ao_invert"),
            ("ao_blur", "ao_blur"),
            ("ao_balance", "ao_balance"),
            ("ao_contrast", "ao_contrast"),
        ],
    );
    let o = texture(o, p, "texture", "tex1", TEXTURE_NEW);
    finish(o, p).num("seed", p.f("$randomseed")).done()
}

const SCALES: [&str; 7] = ["sharp", "fine", "soft", "medium", "large", "big", "huge"];

/// The seven curvature scales and the mode, from `curvature_<scale>` or `Curvature_<Scale>` names.
fn scales(mut o: Ours, p: &Painter) -> Ours {
    for s in SCALES {
        let mut chars = s.chars();
        let upper = format!(
            "Curvature_{}{}",
            chars.next().map(|c| c.to_ascii_uppercase()).unwrap_or_default(),
            chars.as_str()
        );
        o = o.num(&format!("curv_{s}"), p.first(&[&format!("curvature_{s}"), &upper]));
    }
    o.num("curv_mode", p.first(&["curvature_mode", "Curvature_Mode"]))
}

fn curvature(p: &Painter) -> Vec<(String, String)> {
    let o = scales(Ours::default(), p).nums(
        p,
        &[
            ("curv_contrast", "curvature_contrast"),
            ("curv_brightness", "curvature_brightness"),
            ("curv_invert", "curvature_invert"),
        ],
    );
    let o = texture(o, p, "texture", "tex1", TEXTURE_NEW);
    finish(o, p).num("seed", p.f("$randomseed")).done()
}

/// A part copied from the first name found of each pair (newer Painter names first, then older ones).
fn part(o: Ours, p: &Painter, ours: &str, theirs: &[(&str, &[&str])]) -> Ours {
    theirs
        .iter()
        .fold(o, |o, (k, names)| o.num(&format!("{ours}_{k}"), p.first(names)))
}

fn mask_editor(p: &Painter) -> Vec<(String, String)> {
    let mut o = texture(
        Ours::default(),
        p,
        "texture",
        "tex1",
        &[
            ("opacity", &["texture_opacity", "tex"]),
            ("blend", &["texture_blendmode", "tex_blend_mode"]),
            ("scale", &["texture_scale", "scale_tex"]),
            ("contrast", &["texture_contrast", "tex_contrast"]),
            ("brightness", &["texture_brightness", "tex_lum"]),
            ("invert", &["texutre_invert", "texture_invert", "invert_tex"]),
            ("triplanar", &["texture_triplanar", "tex1_trip"]),
            (
                "triplanar_contrast",
                &["texture_triplanar_contrast", "tex1_trip_control"],
            ),
        ],
    );
    o = texture(
        o,
        p,
        "texture_sec",
        "tex2",
        &[
            ("opacity", &["texture2_opacity", "tex2"]),
            ("blend", &["texture2_blendmode", "tex2_blend_mode"]),
            ("scale", &["texture2_scale", "tex2_scale"]),
            ("contrast", &["texture2_contrast", "tex2_contrast"]),
            ("brightness", &["texture2_brightness", "tex2_lum"]),
            ("invert", &["texture2_invert", "invert_tex_sec"]),
            ("triplanar", &["texture2_triplanar", "tex2_trip"]),
            (
                "triplanar_contrast",
                &["texture2_triplanar_contrast", "tex2_trip_control"],
            ),
        ],
    );
    o = part(
        o,
        p,
        "ao",
        &[
            ("opacity", &["ao_opacity", "ao"]),
            ("blend", &["ao_blendmode", "ao_blend_mode"]),
            ("invert", &["ao_invert", "invert_ao"]),
            ("blur", &["ao_blur"]),
            ("balance", &["ao_balance"]),
            ("contrast", &["ao_contrast"]),
        ],
    );
    o = scales(o, p);
    o = part(
        o,
        p,
        "curv",
        &[
            ("opacity", &["curvature_opacity", "curv"]),
            ("blend", &["curvature_blendmode", "curv_blend_mode"]),
            ("invert", &["curvature_invert", "invert_invert"]),
            ("contrast", &["curvature_contrast", "curv_contrast"]),
            ("brightness", &["curvature_brightness", "curv_lum"]),
        ],
    );
    o = part(
        o,
        p,
        "normal",
        &[
            ("opacity", &["ws_opacity", "ws"]),
            ("blend", &["ws_blendmode", "ws_blend"]),
            ("invert", &["ws_invert"]),
            ("balance", &["ws_balance", "world_balance"]),
            ("contrast", &["ws_contrast"]),
            ("brightness", &["ws_brightness", "ws_lum"]),
            (
                "x",
                &["ws_direction_rightleft_opacity", "Object_Space_Direction_Right_to_Left"],
            ),
            (
                "y",
                &["ws_direction_topbottom_opacity", "Object_Space_Direction_Top_To_Bottom"],
            ),
            (
                "z",
                &["ws_direction_frontback_opacity", "Object_Space_Direction_Front_to_Back"],
            ),
            (
                "x_invert",
                &[
                    "ws_direction_rightleft_invert",
                    "Object_Space_Direction_Right_to_Left_Invert",
                ],
            ),
            (
                "y_invert",
                &[
                    "ws_direction_topbottom_invert",
                    "Object_Space_Direction_Top_To_Bottom_Invert",
                ],
            ),
            (
                "z_invert",
                &[
                    "ws_direction_frontback_invert",
                    "Object_Space_Direction_Front_to_Back_Invert",
                ],
            ),
        ],
    );
    o = part(
        o,
        p,
        "pos",
        &[
            ("opacity", &["position_opacity", "Position_Gradient"]),
            ("blend", &["position_blendmode", "grad_blend"]),
            ("invert", &["position_invert", "grad_invert"]),
            ("balance", &["position_balance", "Gradient_Direction_Balance"]),
            ("contrast", &["position_contrast", "grad_contrast"]),
            ("brightness", &["position_brightness", "grad_lum"]),
            (
                "x",
                &[
                    "position_direction_rightleft_opacity",
                    "Gradient_Direction_Right_to_Left",
                ],
            ),
            (
                "y",
                &[
                    "position_direction_topbottom_opacity",
                    "Gradient_Direction_Top_To_Bottom",
                ],
            ),
            (
                "z",
                &[
                    "position_direction_frontback_opacity",
                    "Gradient_Direction_Front_to_Back",
                ],
            ),
            (
                "x_invert",
                &[
                    "position_direction_rightleft_invert",
                    "Gradient_Direction_Right_to_Left_Invert",
                ],
            ),
            (
                "y_invert",
                &[
                    "position_direction_topbottom_invert",
                    "Gradient_Direction_Top_To_Bottom_Invert",
                ],
            ),
            (
                "z_invert",
                &[
                    "position_direction_frontback_invert",
                    "Gradient_Direction_Front_to_Back_Invert",
                ],
            ),
        ],
    );
    o = part(
        o,
        p,
        "thick",
        &[
            ("opacity", &["thickness_opacity", "thick"]),
            ("blend", &["thickness_blendmode", "thick_blend"]),
            ("invert", &["thickness_invert", "thick_invert"]),
            ("blur", &["thickness_blur", "thick_blur"]),
            ("contrast", &["thickness_contrast", "Thickness_thick_Contrast"]),
            ("brightness", &["thickness_brightness", "Thickness_thick_Luminosity"]),
        ],
    );
    finish(o, p).num("seed", p.f("$randomseed")).done()
}

fn mask_builder(p: &Painter) -> Vec<(String, String)> {
    Ours::default()
        .nums(
            p,
            &[
                ("level", "Level"),
                ("contrast", "Contrast"),
                ("invert", "Invert"),
                ("grunge", "Grunge"),
                ("grunge_scale", "Grunge_Scale"),
                ("triplanar", "Use_Triplanar"),
                ("triplanar_contrast", "Triplanar_Blending_Contrast"),
                ("seed", "$randomseed"),
                ("ao", "AO"),
                ("ao_range", "AO_Range"),
                ("ao_contrast", "AO_Contrast"),
                ("ao_noise", "AO_Noisiness"),
                ("curvature", "Curvature"),
                ("convex_range", "Curvature_Convex_Range"),
                ("concave_range", "Curvature_Concave_Range"),
                ("curvature_smoothness", "Curvature_Smoothness"),
                ("curvature_boost", "Curvature_Level_Boost"),
                ("curvature_noise", "Curvature_Noisiness"),
                ("gradient", "Top_Down_Gradient"),
                ("gradient_range", "Gradient_Range"),
                ("gradient_contrast", "Gradient_Contrast"),
                ("gradient_invert", "Gradient_Invert"),
                ("normal", "World_Space_Normal"),
                ("normal_top", "World_Space_Normal_Top_Intensity"),
                ("normal_bottom", "World_Space_Normal_Bottom_Intensity"),
                ("normal_front", "World_Space_Normal_Front_Intensity"),
                ("normal_back", "World_Space_Normal_Back_Intensity"),
                ("normal_right", "World_Space_Normal_Right_Intensity"),
                ("normal_left", "World_Space_Normal_Left_Intensity"),
            ],
        )
        .done()
}

fn uv_border_distance(p: &Painter) -> Vec<(String, String)> {
    Ours::default()
        .nums(
            p,
            &[
                ("distance", "distance"),
                ("position", "position"),
                ("contrast", "contrast"),
                ("intensity", "Intensity"),
                ("invert", "invert"),
            ],
        )
        .done()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_follow_the_input_names() {
        assert_eq!(look("grunge_plaster_cracked"), 10.0);
        assert_eq!(look("ratio_gaussian_spots_2"), 12.0);
        assert_eq!(look("ratio_dirt_1"), 1.0);
        assert_eq!(look("something_else"), 1.0);
    }
}

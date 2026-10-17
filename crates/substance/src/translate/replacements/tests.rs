//! Tests of the replacement table: conversion keeps the original, plans decide, reconversion is stable.

use super::*;
use materialcraft::generators;

fn imported(name: &str, params: &[(&str, &str)]) -> Procedural {
    Procedural::new(format!("substance:{name}"))
        .with_params(params.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect())
}

#[test]
fn every_entry_names_a_registered_generator_or_filter_once() {
    for r in entries() {
        let registry = generators::builtin();
        let found = if is_filter(r) {
            registry.filter(r.generator).is_some()
        } else {
            registry.generator(r.generator).is_some()
        };
        assert!(found, "{}", r.generator);
        for name in r.painter {
            assert!(std::ptr::eq(find(name).unwrap(), r), "{name} listed twice");
        }
    }
    assert!(find("PBR_to_Skyrim").is_none());
    assert!(is_filter(find("blur").unwrap()) && !is_filter(find("mg_dirt").unwrap()));
}

#[test]
fn replacing_translates_parameters_and_keeps_the_original() {
    let dirt = imported(
        "mg_dirt",
        &[
            ("dirt_level", "0.6"),
            ("Grunge_Scale", "2"),
            ("$randomseed", "7"),
            ("unknown", "1"),
        ],
    );
    let plan = ProceduralPlan::all(Choice::Replace);
    let (r, ours) = replace(&dirt, &plan).unwrap();
    assert_eq!(r.generator, generators::DIRT);
    assert_eq!(ours.id, "dirt");
    assert_eq!(ours.param("level"), Some("0.6"));
    assert_eq!(ours.param("grunge_scale"), Some("2"));
    assert_eq!(ours.param("seed"), Some("7"));
    assert_eq!(ours.original.as_deref(), Some(&dirt));
    assert!(replace(&dirt, &ProceduralPlan::default()).is_none());
    assert!(
        replace(
            &dirt,
            &ProceduralPlan::all(Choice::Replace).with("mg_dirt", Choice::Keep)
        )
        .is_none()
    );
    assert!(replace(&imported("PBR_to_Skyrim", &[]), &plan).is_none());
    assert!(
        replace(&Procedural::new("dirt"), &plan).is_none(),
        "ours are not replaced again"
    );
}

#[test]
fn mask_editor_reads_both_naming_schemes_and_texture_inputs() {
    let newer = imported(
        "mg_mask_editor",
        &[
            ("ao_opacity", "1"),
            ("curvature_fine", "0.3"),
            ("texture_opacity", "0.5"),
            ("input:texture", "grunge_plaster_cracked"),
        ],
    );
    let older = imported(
        "mg_mask_editor",
        &[
            ("ao", "1"),
            ("Curvature_Fine", "0.3"),
            ("tex", "0.5"),
            ("input:texture", "uniform"),
        ],
    );
    let plan = ProceduralPlan::all(Choice::Replace);
    let (_, a) = replace(&newer, &plan).unwrap();
    let (_, b) = replace(&older, &plan).unwrap();
    for p in [&a, &b] {
        assert_eq!(p.param("ao_opacity"), Some("1"));
        assert_eq!(p.param("curv_fine"), Some("0.3"));
    }
    assert_eq!(
        (a.param("tex1_opacity"), a.param("tex1_look")),
        (Some("0.5"), Some("10"))
    );
    assert_eq!(a.param("tex1_texture"), Some("plaster_cracks"));
    assert_eq!(b.param("tex1_opacity"), None, "a uniform input adds no texture");
}

#[test]
fn reconversion_follows_the_plan_and_is_stable() {
    let original = imported("mg_curvature", &[("global_invert", "1")]);
    let replace_all = ProceduralPlan::all(Choice::Replace);
    let ours = reconvert(&original, &replace_all).unwrap();
    assert_eq!(ours.id, "curvature");
    assert_eq!(ours.param("invert"), Some("1"));
    assert!(reconvert(&ours, &replace_all).is_none(), "already current");
    assert_eq!(
        reconvert(&ours, &ProceduralPlan::default()),
        Some(original.clone()),
        "back to Painter's"
    );
    assert!(reconvert(&original, &ProceduralPlan::default()).is_none());
    assert_eq!(painter_name("substance:mg_dirt"), Some("mg_dirt"));
}

#[test]
fn texture_entries_translate_tone_seed_and_disorder() {
    let clouds = imported(
        "ratio_clouds_2",
        &[
            ("$randomseed", "12"),
            ("balance", "0.3"),
            ("historgram_contrast", "0.2"),
            ("Disorder", "50"),
            ("scale", "4"),
        ],
    );
    let (r, ours) = replace(&clouds, &ProceduralPlan::all(Choice::Replace)).unwrap();
    assert_eq!(r.generator, "clouds_2");
    assert_eq!(ours.param("seed"), Some("12"));
    assert_eq!(ours.param("balance"), Some("0.3"));
    assert_eq!(ours.param("range"), Some("0.2"));
    assert_eq!(ours.param("disorder"), Some("0.5"));
    assert_eq!(ours.param("scale"), Some("4"));
    assert_eq!(texture_for("ratio_grunge_map_012"), Some("grunge_12"));
    assert_eq!(texture_for("mg_dirt"), None, "generators are not textures");
}

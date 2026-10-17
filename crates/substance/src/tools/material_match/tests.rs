use super::look::LookBuilder;
use super::*;
use crate::tools::accuracy::ReplacementAccuracy;
use crate::tools::analyze::{ProceduralUse, Role};
use std::path::PathBuf;

fn look(base: [f32; 3], rough: f32, metal: f32) -> Look {
    let mut b = LookBuilder::default();
    for d in [-0.05, 0.0, 0.05] {
        b.add(Channel::BaseColor, [base[0] + d, base[1] + d, base[2] + d, 1.0], 1.0);
        b.add(Channel::Roughness, [rough + d; 4], 1.0);
        b.add(Channel::Metallic, [metal; 4], 1.0);
    }
    b.build()
}

fn material(reference: &str, name: &str, tags: &[&str], look: Option<Look>) -> TrayMaterial {
    TrayMaterial {
        reference: reference.into(),
        name: name.into(),
        tags: tags.iter().map(|t| t.to_string()).collect(),
        look,
    }
}

fn tray() -> TrayMaterials {
    TrayMaterials::from_materials(
        vec![],
        vec![
            material(
                "materials/iron-rusted.mca",
                "Rusted iron",
                &["metal", "iron", "rust", "worn"],
                Some(look([0.45, 0.25, 0.15], 0.8, 0.3)),
            ),
            material(
                "materials/chrome.mca",
                "Chrome",
                &["metal", "chrome", "polished"],
                Some(look([0.9, 0.9, 0.9], 0.05, 1.0)),
            ),
            material(
                "materials/moss.mca",
                "Moss",
                &["ground", "moss"],
                Some(look([0.25, 0.4, 0.1], 0.9, 0.0)),
            ),
            material("materials/broken.mca", "Broken", &[], None),
        ],
    )
}

fn painter(base: [f32; 3], rough: f32, metal: f32) -> PainterLook {
    PainterLook {
        look: look(base, rough, metal),
        layers: 1,
        open_masks: 0,
        coverage: 0.5,
    }
}

fn material_use(name: &str, set: &str) -> ProceduralUse {
    ProceduralUse {
        name: name.into(),
        role: Role::Fill,
        set: set.into(),
        layer: "L".into(),
        visible: true,
        params: vec![],
        inputs: vec![],
        material: true,
    }
}

#[test]
fn name_ranks_first_and_look_decides_without_a_name_match() {
    let tray = tray();
    // by name: rust → Rusted iron, though Painter's result is closer to moss
    let ranked = MaterialMatch::rank("rust_fine", Some(&painter([0.25, 0.4, 0.1], 0.9, 0.0)), &tray);
    assert_eq!(ranked[0].reference, "materials/iron-rusted.mca", "{ranked:?}");
    // no name matches: the look decides
    let ranked = MaterialMatch::rank("Swirls", Some(&painter([0.88, 0.88, 0.9], 0.1, 0.95)), &tray);
    assert_eq!(ranked[0].reference, "materials/chrome.mca");
    let look = ranked[0].look.as_ref().unwrap();
    assert_eq!(
        look.channels.iter().map(|(c, _)| *c).collect::<Vec<_>>(),
        [Channel::BaseColor, Channel::Roughness, Channel::Metallic]
    );
    assert!(look.overall > 0.8 && ranked[0].score > ranked[1].score);
    // a material we could not render scores nothing on the look
    let broken = ranked.iter().find(|s| s.reference == "materials/broken.mca").unwrap();
    assert_eq!((broken.score, broken.look.is_none()), (0.0, true));
    // Painter's look sampled under masks we could not compute counts less than the name
    let unsure = PainterLook {
        open_masks: 1,
        ..painter([0.25, 0.4, 0.1], 0.9, 0.0)
    };
    assert_eq!(unsure.confidence(), 0.25);
    let sure = MaterialMatch::rank("chrome_moss", Some(&painter([0.25, 0.4, 0.1], 0.9, 0.0)), &tray);
    let vague = MaterialMatch::rank("chrome_moss", Some(&unsure), &tray);
    let gap = |r: &[Suggestion]| {
        let s = |id: &str| r.iter().find(|s| s.reference == id).unwrap().score;
        s("materials/moss.mca") - s("materials/chrome.mca")
    };
    assert!(gap(&sure) > gap(&vague), "the look separates the two less when unsure");
    // without Painter's look: name only
    let ranked = MaterialMatch::rank("Moss", None, &tray);
    assert_eq!(
        (ranked[0].reference.as_str(), ranked[0].score),
        ("materials/moss.mca", 1.0)
    );
    assert!(ranked.iter().all(|s| s.look.is_none()));
}

#[test]
fn matches_choose_the_plan_and_print_a_plan_file() {
    let uses = vec![
        material_use("rust_fine", "Body"),
        material_use("Moss", "Body"),
        material_use("rust_fine", "Head"),
    ];
    let mut uses = uses;
    // a mask generator is no material
    uses.push(ProceduralUse {
        role: Role::MaskFill,
        material: false,
        ..material_use("mg_dirt", "Body")
    });
    let analysis = Analysis::of_uses("p.spp", uses, &ReplacementAccuracy::default());
    let looks = PainterLooks {
        looks: vec![("Moss".into(), painter([0.25, 0.4, 0.1], 0.9, 0.0))],
    };
    let matches = MaterialMatches::from_looks(&analysis, &looks, &tray());
    assert_eq!(
        matches.materials.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(),
        ["rust_fine", "Moss"]
    );
    let rust = &matches.materials[0];
    assert_eq!(
        (rust.uses, rust.visible_uses, rust.sets.clone()),
        (2, 2, vec!["Body".to_string(), "Head".to_string()])
    );
    assert!(rust.painter.is_none() && matches.materials[1].painter.is_some());
    assert_eq!(rust.suggestions().len(), 4);
    // the user's choice wins over the best suggestion
    let base = ProceduralPlan::default().with_material("Moss", Some("materials/chrome.mca"));
    let plan = matches.best_plan(base);
    assert_eq!(plan.material("rust_fine"), Some("materials/iron-rusted.mca"));
    assert_eq!(plan.material("Moss"), Some("materials/chrome.mca"));
    // the printed array, edited or not, is a plan file
    let printed = Value::object().with("materials", matches.to_json(&plan));
    assert_eq!(ProceduralPlan::parse(&printed.to_string()).unwrap().key(), plan.key());
    let kept = Value::object().with("materials", matches.to_json(&ProceduralPlan::default()));
    assert_eq!(
        ProceduralPlan::parse(&kept.to_string()).unwrap(),
        ProceduralPlan::default()
    );
}

#[test]
fn tray_materials_are_read_and_rendered_from_library_folders() {
    let dir = std::env::temp_dir().join(format!("mc-tray-materials-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("materials")).unwrap();
    std::fs::write(dir.join("materials/brick.mca"), materialcraft::graph::EXAMPLE).unwrap();
    std::fs::write(dir.join("materials/broken.mca"), "not a graph").unwrap();
    let tray = TrayMaterials::load(&[dir.clone(), PathBuf::from("/nonexistent")]);
    let refs: Vec<&str> = tray.materials().iter().map(|m| m.reference.as_str()).collect();
    assert_eq!(refs, ["materials/brick.mca", "materials/broken.mca"]);
    let brick = tray.materials()[0].look.as_ref().expect("the example graph renders");
    assert!(brick.channel(Channel::BaseColor).is_some() && brick.channel(Channel::Roughness).is_some());
    assert!(tray.materials()[1].look.is_none());
    assert!(materialcraft::material_fill::MaterialResolver::resolve(&tray.resolver(), "materials/brick.mca").is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

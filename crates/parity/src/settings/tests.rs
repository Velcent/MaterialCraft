//! What a case runs with: the importer's mapping plus the unmapped list per baker and the settings hash.

use super::mapping::usage;
use super::{MaterialBaking, Params, TweakList, TweakValue};
use materialcraft_substance::model::baking::BakerSettings;

fn list(baker: &str, tweaks: &[(&str, TweakValue)]) -> TweakList {
    TweakList {
        baker: baker.into(),
        tweaks: tweaks.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
    }
}

fn material() -> MaterialBaking {
    MaterialBaking {
        material_uid: Some(7),
        enabled: true,
        common: list("common", &[("Common.DilationWidth", TweakValue::Int(4))]),
        detail: list("detail", &[("Detail.UseCage", TweakValue::Bool(true))]),
        bakers: vec![BakerSettings {
            usage: Some(usage::AO),
            enabled: true,
            params: list(
                "GLMapBakerManager.AOFromDetail",
                &[("Detail.AO.GroundPlane", TweakValue::Bool(true))],
            ),
        }],
    }
}

#[test]
fn lists_unmapped_settings_per_baker_and_hashes_the_settings() {
    let p = Params::from_painter(Some(&material()), 512);
    assert_eq!((p.size, p.bake.padding), (512, 4));
    let ao = p.unmapped_for(Some(usage::AO));
    assert!(ao.iter().any(|t| t.contains("GroundPlane")));
    assert!(ao.iter().any(|t| t.contains("UseCage")));
    assert!(
        !p.unmapped_for(Some(usage::CURVATURE))
            .iter()
            .any(|t| t.contains("GroundPlane"))
    );
    assert_eq!(p.hash(), Params::from_painter(Some(&material()), 512).hash());
    assert_ne!(p.hash(), Params::from_painter(Some(&material()), 256).hash());
}

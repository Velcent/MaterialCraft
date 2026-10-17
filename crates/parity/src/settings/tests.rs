//! Synthetic settings: mapping onto ours and the unmapped list.

use super::mapping::usage;
use super::painter::{BakerSettings, MaterialBaking, TweakList, TweakValue};
use super::{HighPolyRequest, Params};
use materialcraft::bake::Attenuation;

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
        detail: list(
            "detail",
            &[
                ("Detail.LowAsHigh", TweakValue::Bool(false)),
                ("Detail.HipolyMesh", TweakValue::Text("file:///D:/x/Vase HP.obj".into())),
                ("Detail.MaxHeight", TweakValue::Float(0.02)),
                ("Detail.MaxDepth", TweakValue::Float(0.01)),
                ("Detail.SubSampling", TweakValue::Int(2)),
            ],
        ),
        bakers: vec![
            BakerSettings {
                usage: Some(usage::AO),
                enabled: true,
                params: list(
                    "GLMapBakerManager.AOFromDetail",
                    &[
                        ("Detail.SecondaryCommon.NbSecondary", TweakValue::Int(64)),
                        ("Detail.SecondaryCommon.MaxDistance", TweakValue::Float(0.5)),
                        ("Detail.AO.Attenuation", TweakValue::Int(2)),
                    ],
                ),
            },
            BakerSettings {
                usage: Some(usage::CURVATURE),
                enabled: false,
                params: list("c", &[("Detail.Curvature1.SamplingRadius", TweakValue::Float(0.1))]),
            },
        ],
    }
}

#[test]
fn maps_known_settings_and_lists_the_rest() {
    let p = Params::from_painter(Some(&material()), 512);
    assert_eq!((p.size, p.bake.size, p.bake.padding), (512, 512, 4));
    assert_eq!(
        (p.bake.ao_rays, p.bake.ao_dist, p.bake.max_dist, p.bake.rear_dist),
        (64, 0.5, 0.02, Some(0.01))
    );
    assert_eq!((p.bake.ao.attenuation, p.bake.samples), (Attenuation::Linear, 2));
    assert!(p.unmapped.is_empty(), "{:?}", p.unmapped);
    let mut odd = material();
    odd.detail
        .tweaks
        .push(("Detail.UseCage".into(), TweakValue::Bool(true)));
    odd.bakers[0]
        .params
        .tweaks
        .push(("Detail.AO.GroundPlane".into(), TweakValue::Bool(true)));
    let p = Params::from_painter(Some(&odd), 512);
    let ao = p.unmapped_for(Some(usage::AO));
    assert!(ao.iter().any(|t| t.contains("GroundPlane")));
    assert!(ao.iter().any(|t| t.contains("UseCage")));
    // Disabled bakers and other bakers' settings are not listed for AO.
    assert!(!ao.iter().any(|t| t.contains("SamplingRadius")));
    assert_eq!(p.hash(), Params::from_painter(Some(&material()), 512).hash());
    assert_ne!(p.hash(), Params::from_painter(Some(&material()), 256).hash());
}

#[test]
fn without_settings_uses_defaults_and_says_so() {
    let p = Params::from_painter(None, 128);
    assert_eq!(p.bake.size, 128);
    assert_eq!(p.unmapped.len(), 1);
}

#[test]
fn reads_the_high_poly_request() {
    let r = HighPolyRequest::of(&material());
    assert!(!r.low_as_high);
    assert_eq!(r.files, ["Vase HP.obj"]);
}

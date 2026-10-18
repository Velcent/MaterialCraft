//! Synthetic settings: mapping onto ours, the unmapped list and the document's configuration.

use super::{HighPolyRequest, MappedBake, usage};
use crate::model::baking::{BakerSettings, MaterialBaking, TweakList, TweakValue};
use materialcraft::bake::{Attenuation, MapKind};

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
    let m = MappedBake::of(Some(&material()), 512);
    let b = &m.settings;
    assert_eq!((b.size, b.padding), (512, 4));
    assert_eq!(
        (b.ao_rays, b.ao_dist, b.max_dist, b.rear_dist),
        (64, 0.5, 0.02, Some(0.01))
    );
    assert_eq!((b.ao.attenuation, b.samples), (Attenuation::Linear, 2));
    assert!(m.unmapped.is_empty(), "{:?}", m.unmapped);
    assert_eq!(m.enabled, [MapKind::AmbientOcclusion]);
    let mut odd = material();
    odd.detail
        .tweaks
        .push(("Detail.UseCage".into(), TweakValue::Bool(true)));
    odd.bakers[0]
        .params
        .tweaks
        .push(("Detail.AO.GroundPlane".into(), TweakValue::Bool(true)));
    let m = MappedBake::of(Some(&odd), 512);
    assert!(
        m.unmapped
            .iter()
            .any(|u| u.usage == Some(usage::AO) && u.text.contains("GroundPlane"))
    );
    assert!(
        m.unmapped
            .iter()
            .any(|u| u.usage.is_none() && u.text.contains("UseCage"))
    );
    // Disabled bakers are not looked at.
    assert!(!m.unmapped.iter().any(|u| u.text.contains("SamplingRadius")));
}

#[test]
fn the_configuration_bakes_the_same_and_keeps_enabled_bakers_and_the_source() {
    let m = MappedBake::of(Some(&material()), 512);
    let placement = materialcraft::paint::bake_config::HighPlacement {
        center: [1.0, 0.0, 0.0],
        radius: 4.0,
    };
    let c = m.config(Some(placement));
    assert_eq!(c.settings_for(m.settings.maps), m.settings);
    assert_eq!(c.included().kinds().collect::<Vec<_>>(), [MapKind::AmbientOcclusion]);
    assert_eq!(c.common.high.files, ["D:/x/Vase HP.obj"]);
    assert_eq!(c.common.high.placement, Some(placement));
    assert!(!c.common.high.low_as_high);
}

#[test]
fn without_settings_uses_defaults_and_says_so() {
    let m = MappedBake::of(None, 128);
    assert_eq!(m.settings.size, 128);
    assert_eq!(m.unmapped.len(), 1);
    assert_eq!(m.enabled.len(), 7);
}

#[test]
fn reads_the_high_poly_request() {
    let r = HighPolyRequest::of(&material());
    assert!(!r.low_as_high);
    assert_eq!(r.files, ["D:/x/Vase HP.obj"]);
}

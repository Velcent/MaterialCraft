use super::*;
use crate::json;
use crate::paint::Channel;

fn odd_preset() -> BrushPreset {
    BrushPreset {
        name: "Odd \"one\"".into(),
        size: 42.5,
        size_space: SizeSpace::Screen,
        hardness: 0.25,
        flow: 0.3,
        opacity: 0.7,
        spacing: 0.37,
        angle: 33.0,
        follow_direction: true,
        roundness: 0.6,
        jitter: Jitter { size: 0.1, flow: 0.2, angle: 0.3, position: 0.4 },
        pressure: PressureUse { size: false, flow: true },
        placement: Placement::Flat,
        max_angle: 60.0,
        ..BrushPreset::default()
    }
}

#[test]
fn presets_round_trip_through_json() {
    let p = odd_preset();
    let text = p.to_json().to_string();
    assert_eq!(BrushPreset::from_json(&json::parse(&text).unwrap()), Some(p));
    for s in SizeSpace::ALL {
        assert_eq!(SizeSpace::from_id(s.id()), Some(s));
    }
    for pl in Placement::ALL {
        assert_eq!(Placement::from_id(pl.id()), Some(pl));
    }
}

#[test]
fn missing_and_unknown_fields_take_defaults() {
    let v = json::parse(r#"{"size": 3, "future": [1, 2], "placement": "spiral"}"#).unwrap();
    let p = BrushPreset::from_json(&v).unwrap();
    assert_eq!(p.size, 3.0);
    assert_eq!(BrushPreset { size: 3.0, ..BrushPreset::default() }, p);
    assert!(BrushPreset::from_json(&json::parse("[1]").unwrap()).is_none());
}

#[test]
fn clamping_keeps_values_in_range() {
    let p = BrushPreset { flow: 3.0, opacity: f32::NAN, spacing: 0.0, roundness: -1.0, max_angle: 500.0, ..BrushPreset::default() }.clamped();
    assert_eq!((p.flow, p.opacity, p.spacing, p.roundness, p.max_angle), (1.0, 1.0, 0.01, 0.01, 180.0));
}

#[test]
fn recorded_brushes_round_trip_through_their_reference() {
    let r = RecordedBrush {
        preset: odd_preset(),
        values: vec![(Channel::BaseColor, [0.1, 0.2, 0.3, 1.0]), (Channel::User(3), [0.5; 4])],
        mask: 0.25,
        viewport: [1600.0, 900.0],
    };
    let reference = r.to_reference();
    assert!(reference.starts_with(REFERENCE_PREFIX));
    assert_eq!(RecordedBrush::from_reference(&reference), Some(r.clone()));
    assert_eq!(r.value(Channel::User(3)), Some([0.5; 4]));
    assert_eq!(r.value(Channel::Height), None);
    assert!(RecordedBrush::from_reference("substance:DataBrush/12").is_none());
    assert!(RecordedBrush::from_reference(&format!("{REFERENCE_PREFIX}{{broken")).is_none());
}

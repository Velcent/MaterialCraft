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
        retouch: Retouch { strength: 0.3, clone_offset: [-40, 7] },
    };
    let reference = r.to_reference();
    assert!(reference.starts_with(REFERENCE_PREFIX));
    assert_eq!(RecordedBrush::from_reference(&reference), Some(r.clone()));
    let plain = RecordedBrush { retouch: Retouch::default(), ..r.clone() };
    assert!(!plain.to_reference().contains("retouch"), "default retouch settings are not written");
    assert_eq!(RecordedBrush::from_reference(&plain.to_reference()), Some(plain));
    assert_eq!(r.value(Channel::User(3)), Some([0.5; 4]));
    assert_eq!(r.value(Channel::Height), None);
    assert!(RecordedBrush::from_reference("substance:DataBrush/12").is_none());
    assert!(RecordedBrush::from_reference(&format!("{REFERENCE_PREFIX}{{broken")).is_none());
}

#[test]
fn tips_round_trip_and_bad_ones_fall_back_to_round() {
    for tip in [Tip::Round, Tip::Procedural("chalk".into()), Tip::Alpha("alphas/scratch 2.png".into())] {
        let p = BrushPreset { tip: tip.clone(), ..odd_preset() };
        assert_eq!(BrushPreset::from_json(&json::parse(&p.to_json().to_string()).unwrap()).map(|p| p.tip), Some(tip));
    }
    let v = json::parse(r#"{"tip": "alpha"}"#).unwrap();
    assert_eq!(BrushPreset::from_json(&v).unwrap().tip, Tip::Round, "an alpha tip without its image is round");
    let v = json::parse(r#"{"version": 1, "tip": "round"}"#).unwrap();
    assert_eq!(BrushPreset::from_json(&v).unwrap().tip, Tip::Round, "version 1 presets still read");
}

/// The gate of roadmap step 45: a saved preset reloads bit for bit, and saving it again writes the same bytes.
#[test]
fn a_saved_preset_file_reloads_bit_identically() {
    let awkward = [0.1f32, 1.0 / 3.0, f32::MIN_POSITIVE, 0.123_456_79, 7.0e-5, 359.99997];
    for (i, x) in awkward.iter().enumerate() {
        let p = BrushPreset {
            size: *x,
            hardness: awkward[(i + 1) % awkward.len()],
            flow: awkward[(i + 2) % awkward.len()],
            angle: -*x,
            tip: if i % 2 == 0 { Tip::Procedural("spatter".into()) } else { Tip::Alpha("alphas/ü ñ \"q\".png".into()) },
            ..odd_preset()
        };
        let text = p.to_file_text();
        let back = BrushPreset::from_file_text(&text).unwrap();
        assert_eq!(back, p);
        let bits = |p: &BrushPreset| {
            let j = p.jitter;
            [p.size, p.hardness, p.flow, p.opacity, p.spacing, p.angle, p.roundness, p.max_angle, j.size, j.flow, j.angle, j.position].map(f32::to_bits)
        };
        assert_eq!(bits(&back), bits(&p));
        assert_eq!(back.to_file_text(), text, "saving the reloaded preset writes the same file");
    }
    // numbers are written short, and every f32 comes back exactly (a sweep over the bit patterns)
    assert!(BrushPreset::default().to_file_text().contains("\"size\": 0.08,"));
    let mut bits = 0x3000_0000u32;
    while bits < 0x4800_0000 {
        let x = f32::from_bits(bits);
        let back = BrushPreset::from_file_text(&BrushPreset { size: x, ..BrushPreset::default() }.to_file_text()).unwrap();
        assert_eq!(back.size.to_bits(), bits, "{x}");
        bits += 7919;
    }
    assert!(BrushPreset::from_file_text(r#"{"format": "materialcraft-starter"}"#).is_err());
    assert!(BrushPreset::from_file_text("[1]").is_err() && BrushPreset::from_file_text("{").is_err());
    assert_eq!(BrushPreset::from_file_text(r#"{"size": 2}"#).unwrap().size, 2.0, "hand-written presets need no tag");
}

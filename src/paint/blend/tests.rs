use super::*;

fn g(v: f32) -> [f32; 4] {
    [v, v, v, 1.0]
}

fn r(mode: BlendMode, b: f32, t: f32) -> f32 {
    mode.apply(g(b), g(t))[0]
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-5
}

#[test]
fn ids_round_trip_and_are_unique() {
    for m in BlendMode::ALL {
        assert_eq!(BlendMode::from_id(m.id()), Some(m));
    }
    let mut ids: Vec<_> = BlendMode::ALL.iter().map(|m| m.id()).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), BlendMode::ALL.len());
    assert_eq!(BlendMode::from_id("BlendingMode_Multiply"), None);
}

#[test]
fn separable_modes_match_hand_values() {
    use BlendMode::*;
    let (b, t) = (0.4, 0.7);
    let cases = [
        (Normal, 0.7),
        (Multiply, 0.28),
        (Divide, 0.4 / 0.7),
        (InverseDivide, 1.75),
        (Darken, 0.4),
        (Lighten, 0.7),
        (Add, 1.1),
        (Subtract, -0.3),
        (Difference, 0.3),
        (Exclusion, 0.4 + 0.7 - 2.0 * 0.28),
        (SignedAdd, 0.8),
        (Screen, 1.0 - 0.6 * 0.3),
        (Overlay, 0.56),                    // b < 0.5: 2bt
        (HardLight, 1.0 - 2.0 * 0.6 * 0.3), // t >= 0.5
        (ColorDodge, 1.0),                  // 0.4 / 0.3 clamped
        (ColorBurn, 1.0 - 0.6 / 0.7),
        (LinearBurn, 0.1),
        (LinearLight, 0.8),
        (PinLight, 0.4), // max(0.4, 0.4)
        (VividLight, (0.4f32 / (1.0 - 0.4)).min(1.0)),
        (SoftLight, 0.4 + 0.4 * (0.4f32.sqrt() - 0.4)),
        (Disable, 0.4),
    ];
    for (m, want) in cases {
        assert!(close(r(m, b, t), want), "{m:?}: {} != {want}", r(m, b, t));
    }
    assert!(close(r(SoftLight, 0.2, 0.3), 0.2 - 0.4 * 0.2 * 0.8));
    assert!(close(r(Overlay, 0.8, 0.5), 1.0 - 2.0 * 0.2 * 0.5));
    assert_eq!(r(Divide, 0.5, 0.0), 1.0);
    assert_eq!(r(Divide, 0.0, 0.0), 0.0);
    assert_eq!(r(ColorBurn, 0.5, 0.0), 0.0);
}

#[test]
fn nonseparable_modes() {
    let red = [1.0, 0.0, 0.0, 1.0];
    let grey = [0.5, 0.5, 0.5, 1.0];
    // Hue of red onto grey: grey has no saturation, so the result stays grey.
    let h = BlendMode::Hue.apply(grey, red);
    assert!(close(h[0], 0.5) && close(h[1], 0.5));
    // Luminosity of grey onto red keeps red's hue at lum 0.5.
    let v = BlendMode::Value.apply(red, grey);
    assert!(close(0.3 * v[0] + 0.59 * v[1] + 0.11 * v[2], 0.5));
    assert!(v[0] > v[1] && close(v[1], v[2]));
    let c = BlendMode::Color.apply(grey, red);
    assert!(c[0] > c[1]);
    let s = BlendMode::Saturation.apply(red, grey);
    assert!(close(s[0], s[1]) && close(s[1], s[2]));
}

#[test]
fn normal_map_modes() {
    let flat = [0.5, 0.5, 1.0, 1.0];
    let tilted = normal::encode(normal::normalize([0.4, 0.0, 1.0]));
    for m in [BlendMode::NormalMapCombine, BlendMode::NormalMapDetail, BlendMode::NormalMapInverseDetail] {
        let a = m.apply(flat, tilted);
        let b = m.apply(tilted, flat);
        for i in 0..3 {
            assert!(close(a[i], tilted[i]) && close(b[i], tilted[i]), "{m:?}");
        }
    }
}

#[test]
fn mix_uses_opacity_times_coverage() {
    let base = [0.2, 0.2, 0.2, 0.0];
    let top = [1.0, 1.0, 1.0, 0.5];
    let m = BlendMode::Normal.mix(base, top, 0.5);
    assert!(close(m[0], 0.2 + 0.8 * 0.25));
    assert!(close(m[3], 0.25));
    let m2 = BlendMode::Normal.mix([0.0, 0.0, 0.0, 0.5], [1.0, 1.0, 1.0, 1.0], 0.5);
    assert!(close(m2[3], 0.75));
    assert_eq!(BlendMode::Multiply.mix(base, [0.5, 0.5, 0.5, 0.0], 1.0), base);
    assert_eq!(BlendMode::Normal.mix(base, top, f32::NAN), base);
    let rep = BlendMode::Replace.mix([0.2, 0.2, 0.2, 1.0], [1.0, 1.0, 1.0, 0.0], 1.0);
    assert_eq!(rep, [1.0, 1.0, 1.0, 0.0]);
    assert_eq!(BlendMode::Disable.mix(base, top, 1.0), base);
    let n = BlendMode::NormalMapDetail.mix([0.5, 0.5, 1.0, 0.0], normal::encode(normal::normalize([1.0, 0.0, 1.0])), 0.5);
    let d = normal::decode(n);
    assert!(close(d[0] * d[0] + d[1] * d[1] + d[2] * d[2], 1.0));
}

use super::*;
use materialcraft::stamp::TexelFactor;

#[test]
fn the_keys_move_through_the_states_and_keep_the_mask() {
    let mut m = QuickMask::default();
    assert_eq!(m.state(), QuickMaskState::Off);
    assert!(!m.apply() && !m.invert(), "nothing to apply or invert while off");
    m.toggle();
    assert!(m.is_editing() && m.is_shown());
    assert!(m.invert() && m.is_inverted());
    m.begin_edit("s", (8, 8)).unwrap().set(1, 1, [1.0; 4]);
    assert!(m.apply());
    assert_eq!(m.state(), QuickMaskState::Applied);
    assert!(!m.invert(), "invert works in the edit state only");
    m.toggle();
    assert_eq!(m.state(), QuickMaskState::Off);
    assert!(m.image("s").is_some(), "switching off keeps the mask");
    m.toggle();
    m.toggle();
    assert_eq!(m.state(), QuickMaskState::Off, "toggling while editing switches it off");
}

#[test]
fn the_factor_applies_only_when_applied_and_at_the_set_size() {
    let mut m = QuickMask::default();
    m.toggle();
    m.begin_edit("s", (8, 8)).unwrap().set(2, 3, [1.0; 4]);
    assert!(m.factor("s", (8, 8)).is_none(), "editing: strokes paint the mask, not through it");
    m.apply();
    let f = m.factor("s", (8, 8)).unwrap();
    assert_eq!((f.factor(2, 3, None), f.factor(0, 0, None)), (0.0, 1.0), "protected texels get no paint");
    assert!(m.factor("s", (16, 16)).is_none(), "a mask from before a resolution change is not used");
    assert!(m.factor("other", (8, 8)).is_none());
    assert_eq!((m.protection("s", 2, 3), m.protection("s", 0, 0)), (1.0, 0.0));
    m.toggle();
    m.toggle();
    m.invert();
    m.apply();
    let f = m.factor("s", (8, 8)).unwrap();
    assert_eq!((f.factor(2, 3, None), f.factor(0, 0, None)), (1.0, 0.0), "inverted");
}

#[test]
fn strokes_and_clears_undo_in_their_own_history() {
    let mut m = QuickMask::default();
    m.begin_edit("s", (8, 8)).unwrap().set(0, 0, [1.0; 4]);
    m.begin_edit("s", (8, 8)).unwrap().set(1, 0, [1.0; 4]);
    assert!(m.clear("s") && !m.clear("s"));
    assert!(m.image("s").is_none());
    assert_eq!(m.undo().as_deref(), Some("s"));
    assert_eq!(m.protection("s", 1, 0), 1.0);
    m.undo();
    assert_eq!((m.protection("s", 0, 0), m.protection("s", 1, 0)), (1.0, 0.0));
    m.undo();
    assert!(m.image("s").is_none() && !m.can_undo());
    m.redo();
    m.redo();
    assert_eq!(m.protection("s", 1, 0), 1.0);
    assert!(m.can_redo());
    m.forget();
    assert!(m.image("s").is_none() && !m.can_redo());
}

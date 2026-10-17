use super::*;
use std::sync::Arc;

/// A toy command on a list of numbers. `Hold` keeps an allocation alive (stands in for old tiles).
#[derive(Debug, Clone, PartialEq)]
enum Toy {
    Set(usize, i32),
    Push(i32),
    Pop,
    Hold(Arc<[u8]>),
}

impl Command for Toy {
    type Target = Vec<i32>;
    type Error = String;

    fn apply(self, v: &mut Vec<i32>) -> Result<Toy, String> {
        match self {
            Toy::Set(i, x) => match v.get_mut(i) {
                Some(slot) => Ok(Toy::Set(i, std::mem::replace(slot, x))),
                None => Err(format!("no index {i}")),
            },
            Toy::Push(x) => {
                v.push(x);
                Ok(Toy::Pop)
            }
            Toy::Pop => v.pop().map(Toy::Push).ok_or_else(|| "empty".to_string()),
            Toy::Hold(a) => Ok(Toy::Hold(a)),
        }
    }

    fn absorb(&mut self, later: Toy) -> Option<Toy> {
        match (&*self, &later) {
            (Toy::Set(i, _), Toy::Set(j, _)) if i == j => None,
            _ => Some(later),
        }
    }

    fn retained(&self, f: &mut dyn FnMut(usize, usize)) {
        if let Toy::Hold(a) = self {
            f(Arc::as_ptr(a) as *const u8 as usize, a.len());
        }
    }
}

fn labels(h: &History<Toy>) -> Vec<String> {
    h.steps().map(|s| format!("{}{}", s.label, if s.done { "" } else { "*" })).collect()
}

#[test]
fn undo_redo_and_new_edits_drop_the_redo_steps() {
    let mut v = vec![0];
    let mut h = History::<Toy>::default();
    assert!(!h.undo(&mut v).unwrap());
    h.apply(&mut v, "a", None, Toy::Push(1)).unwrap();
    h.apply(&mut v, "b", None, Toy::Set(0, 5)).unwrap();
    h.apply_all(&mut v, "c", None, vec![Toy::Push(2), Toy::Push(3), Toy::Set(1, 9)]).unwrap();
    assert_eq!(v, [5, 9, 2, 3]);
    assert_eq!((h.undo_label(), h.redo_label()), (Some("c"), None));
    assert!(h.undo(&mut v).unwrap());
    assert_eq!(v, [5, 1]);
    assert!(h.undo(&mut v).unwrap());
    assert_eq!(v, [0, 1]);
    assert_eq!(labels(&h), ["a", "b*", "c*"]);
    assert!(h.redo(&mut v).unwrap());
    assert!(h.redo(&mut v).unwrap());
    assert!(!h.redo(&mut v).unwrap());
    assert_eq!(v, [5, 9, 2, 3]);
    h.jump_to(&mut v, 1).unwrap();
    assert_eq!(v, [0, 1]);
    h.apply(&mut v, "d", None, Toy::Push(7)).unwrap();
    assert_eq!(labels(&h), ["a", "d"]);
    assert_eq!(h.dropped(), 2);
    h.jump_to(&mut v, 0).unwrap();
    assert_eq!(v, [0]);
    h.jump_to(&mut v, usize::MAX).unwrap();
    assert_eq!(v, [0, 1, 7]);
    let serials: Vec<u64> = h.steps().map(|s| s.serial).collect();
    assert_eq!(serials, [0, 3]);
}

#[test]
fn a_failed_edit_records_nothing() {
    let mut v = vec![0];
    let mut h = History::<Toy>::default();
    assert!(h.apply_all(&mut v, "bad", None, vec![Toy::Push(1), Toy::Set(4, 1)]).is_err());
    assert_eq!(v, [0]);
    assert!(h.is_empty());
}

#[test]
fn coalescing_joins_edits_until_closed() {
    let mut v = vec![0, 0];
    let mut h = History::<Toy>::default();
    let drag = Some(CoalesceKey(7));
    for x in 1..=10 {
        h.apply(&mut v, "drag", drag, Toy::Set(0, x)).unwrap();
    }
    assert_eq!(h.len(), 1);
    // A different key starts a new step; so does the same key after close().
    h.apply(&mut v, "other", Some(CoalesceKey(8)), Toy::Set(1, 1)).unwrap();
    h.apply(&mut v, "drag", drag, Toy::Set(0, 20)).unwrap();
    h.close();
    h.apply(&mut v, "drag", drag, Toy::Set(0, 30)).unwrap();
    assert_eq!(h.len(), 4);
    // Undo closes the step too.
    h.undo(&mut v).unwrap();
    h.apply(&mut v, "drag", drag, Toy::Set(0, 40)).unwrap();
    assert_eq!(h.len(), 4);
    h.jump_to(&mut v, 1).unwrap();
    assert_eq!(v, [10, 0]);
    h.undo(&mut v).unwrap();
    assert_eq!(v, [0, 0]);
    // One edit in a drag that the command cannot fold stays a separate op of the same step.
    h.apply(&mut v, "mixed", drag, Toy::Set(0, 1)).unwrap();
    h.apply(&mut v, "mixed", drag, Toy::Push(3)).unwrap();
    h.apply(&mut v, "mixed", drag, Toy::Set(0, 2)).unwrap();
    assert_eq!(h.len(), 1);
    h.undo(&mut v).unwrap();
    assert_eq!(v, [0, 0]);
    h.redo(&mut v).unwrap();
    assert_eq!(v, [2, 0, 3]);
}

#[test]
fn budget_drops_oldest_steps_and_counts_shared_memory_once() {
    let mut v = vec![0];
    let mut h = History::<Toy>::new(1000);
    let blob = |n: usize| -> Arc<[u8]> { Arc::from(vec![0u8; n]) };
    let shared = blob(300);
    h.apply(&mut v, "1", None, Toy::Hold(shared.clone())).unwrap();
    h.apply(&mut v, "2", None, Toy::Hold(shared)).unwrap();
    assert_eq!(h.retained_bytes(&v), 300);
    h.apply(&mut v, "3", None, Toy::Hold(blob(600))).unwrap();
    assert_eq!((h.len(), h.retained_bytes(&v)), (3, 900));
    h.apply(&mut v, "4", None, Toy::Hold(blob(200))).unwrap();
    assert_eq!(labels(&h), ["3", "4"], "both steps holding the shared blob are dropped");
    assert_eq!(h.retained_bytes(&v), 800);
    h.undo(&mut v).unwrap();
    h.undo(&mut v).unwrap();
    h.apply(&mut v, "5", None, Toy::Hold(blob(1500))).unwrap();
    assert_eq!(labels(&h), ["5"], "the newest step stays even over budget");
    h.apply(&mut v, "6", None, Toy::Hold(blob(10))).unwrap();
    assert_eq!((labels(&h), h.retained_bytes(&v)), (vec!["6".to_string()], 10));
    h.undo(&mut v).unwrap();
    h.apply(&mut v, "7", None, Toy::Push(1)).unwrap();
    h.undo(&mut v).unwrap();
    h.set_budget(&v, 0);
    assert_eq!(h.len(), 1);
    h.clear();
    assert_eq!((h.len(), h.retained_bytes(&v), h.can_undo()), (0, 0, false));
}

#[test]
fn a_step_that_no_longer_applies_is_dropped_with_older_ones() {
    let mut v = vec![0];
    let mut h = History::<Toy>::default();
    h.apply(&mut v, "a", None, Toy::Push(1)).unwrap();
    h.apply(&mut v, "b", None, Toy::Push(2)).unwrap();
    h.apply(&mut v, "c", None, Toy::Set(2, 5)).unwrap();
    v.truncate(1); // changed outside the history
    assert!(h.undo(&mut v).is_err());
    assert_eq!(v, [0]);
    assert!(!h.can_undo());
    assert!(h.is_empty());
}

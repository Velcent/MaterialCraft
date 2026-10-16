//! [`Computability`]: how far one part of a stack (a layer's content for a channel, a mask, a blend mode,
//! a whole channel) can be reproduced from the document alone, with the reasons when it cannot.

/// At most this many distinct reasons are kept per part (the report stays readable).
const MAX_REASONS: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Computability {
    /// Reproduced as Painter computes it.
    #[default]
    Exact,
    /// Reproduced with a stand-in (a guessed blend mode, a resampled bitmap, …).
    Approximated(Vec<String>),
    /// Needs something we cannot evaluate yet (procedurals, strokes, …).
    NotComputable(Vec<String>),
}

impl Computability {
    pub fn approximated(reason: impl Into<String>) -> Computability {
        Computability::Approximated(vec![reason.into()])
    }

    pub fn not_computable(reason: impl Into<String>) -> Computability {
        Computability::NotComputable(vec![reason.into()])
    }

    pub fn is_exact(&self) -> bool {
        matches!(self, Computability::Exact)
    }

    /// Exact or approximated: there are pixels to show.
    pub fn is_computable(&self) -> bool {
        !matches!(self, Computability::NotComputable(_))
    }

    pub fn reasons(&self) -> &[String] {
        match self {
            Computability::Exact => &[],
            Computability::Approximated(r) | Computability::NotComputable(r) => r,
        }
    }

    fn rank(&self) -> u8 {
        match self {
            Computability::Exact => 0,
            Computability::Approximated(_) => 1,
            Computability::NotComputable(_) => 2,
        }
    }

    /// Combines with another part: the worse state wins, reasons of that state are merged.
    pub fn and(&mut self, other: &Computability) {
        if other.rank() > self.rank() {
            *self = match other {
                Computability::Approximated(_) => Computability::Approximated(Vec::new()),
                Computability::NotComputable(_) => Computability::NotComputable(Vec::new()),
                Computability::Exact => Computability::Exact,
            };
        }
        if other.rank() == self.rank() {
            if let Computability::Approximated(r) | Computability::NotComputable(r) = self {
                for reason in other.reasons() {
                    if !r.contains(reason) && r.len() < MAX_REASONS {
                        r.push(reason.clone());
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worst_state_wins_and_reasons_merge() {
        let mut c = Computability::Exact;
        c.and(&Computability::approximated("guess"));
        assert_eq!(c, Computability::approximated("guess"));
        c.and(&Computability::not_computable("strokes"));
        c.and(&Computability::approximated("other guess"));
        c.and(&Computability::not_computable("strokes"));
        c.and(&Computability::not_computable("filter"));
        assert_eq!(c.reasons(), ["strokes", "filter"]);
        assert!(!c.is_computable());
        let mut e = Computability::Exact;
        e.and(&Computability::Exact);
        assert!(e.is_exact());
    }
}

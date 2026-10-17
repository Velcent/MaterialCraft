//! The search text parser (AS-15, ADR 0008 q-search-syntax).
//!
//! - Terms are separated by spaces or commas; all terms must match (AND).
//! - A plain word matches if a field *contains* it; `!word` matches only an *exact* field value.
//! - `"two words"` is one term with a space; `\` escapes the next character (`\"`, `\\`, `\,`).
//! - `field:value` limits a term to one field (`name:`, `tag:`, `path:`, `library:`, `usage:`,
//!   `format:`, `source:`); an unknown prefix is plain text.
//! - `-term` excludes what matches; `-`, `!` and the field combine: `-tag:!old`, `!name:rust`.
//!
//! Parsing never fails: an unclosed quote runs to the end, and an empty term is dropped.

use super::{Field, Mode, Term};

/// One raw character of a token and whether it was literal (quoted or escaped), so `"-x"` and `\!x`
/// are text, not operators.
#[derive(Clone, Copy)]
struct Ch {
    c: char,
    literal: bool,
}

/// Parse search text into terms.
pub(super) fn parse(text: &str) -> Vec<Term> {
    tokens(text).iter().filter_map(|t| term(t)).collect()
}

fn tokens(text: &str) -> Vec<Vec<Ch>> {
    let mut out = Vec::new();
    let mut cur: Vec<Ch> = Vec::new();
    let mut quoted = false;
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let Some(next) = chars.next() {
                    cur.push(Ch { c: next, literal: true });
                }
            }
            '"' => quoted = !quoted,
            c if !quoted && (c.is_whitespace() || c == ',') => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(Ch { c, literal: quoted }),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn term(chars: &[Ch]) -> Option<Term> {
    let mut rest = chars;
    let mut exclude = false;
    let mut exact = false;
    // Leading operators, in any order.
    while let Some((first, tail)) = rest.split_first() {
        match (first.c, first.literal) {
            ('-', false) if !exclude => exclude = true,
            ('!', false) if !exact => exact = true,
            _ => break,
        }
        rest = tail;
    }
    let mut field = Field::Any;
    if let Some(colon) = rest.iter().position(|ch| ch.c == ':' && !ch.literal) {
        let prefix: String = rest.iter().take(colon).map(|ch| ch.c).collect();
        if rest.iter().take(colon).all(|ch| !ch.literal)
            && let Some(f) = Field::from_prefix(&prefix)
        {
            field = f;
            rest = rest.get(colon + 1..).unwrap_or_default();
            if let Some((first, tail)) = rest.split_first()
                && first.c == '!'
                && !first.literal
            {
                exact = true;
                rest = tail;
            }
        }
    }
    let value: String = rest.iter().map(|ch| ch.c).collect::<String>().to_lowercase();
    if value.is_empty() {
        return None;
    }
    Some(Term { field, mode: if exact { Mode::Exact } else { Mode::Contains }, value, exclude })
}

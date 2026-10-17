//! [`name_score`]: how well a Painter material's name fits one of our materials by its name and tags. Names are
//! split into lowercase words (`Rust_Fine`, `rustFine`, `rust-fine 02` → `rust`, `fine`); a word matches one of
//! ours when they are equal or one starts with the other (`rust` / `rusted`, at least four letters), and half
//! when both belong to the same broad family of everyday material words (`steel` and `iron` are metals).

/// Words that say nothing about the look.
const FILLER: [&str; 12] = [
    "material", "mat", "new", "graph", "base", "smart", "bitmap", "default", "copy", "layer", "fill", "the",
];

/// Broad families of material words (plain English, ours).
const FAMILIES: [&[&str]; 8] = [
    &[
        "metal",
        "metallic",
        "steel",
        "iron",
        "chrome",
        "copper",
        "gold",
        "silver",
        "aluminium",
        "aluminum",
        "brass",
        "bronze",
        "tin",
        "rust",
        "rusted",
    ],
    &[
        "wood", "wooden", "oak", "walnut", "pine", "plank", "planks", "log", "bark", "timber", "carved",
    ],
    &[
        "stone",
        "rock",
        "marble",
        "concrete",
        "brick",
        "slate",
        "cobblestone",
        "granite",
        "pavers",
        "cliff",
        "obsidian",
        "sandstone",
        "strata",
        "tiles",
        "crystal",
    ],
    &[
        "fabric", "cloth", "woven", "denim", "velvet", "cotton", "wool", "leather", "rope", "linen", "silk",
    ],
    &["plastic", "rubber", "vinyl", "polymer"],
    &[
        "ground", "mud", "sand", "dirt", "soil", "gravel", "moss", "grass", "asphalt", "earth", "leaves",
    ],
    &["paint", "painted", "plaster", "coating", "lacquer", "enamel"],
    &[
        "skin", "flesh", "scales", "creature", "alien", "zombie", "elephant", "veins",
    ],
];

/// The words of a name: split at non-letters and lower-to-upper case changes, lowercase, without digits,
/// single letters and filler words.
pub fn words(name: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut prev_lower = false;
    for ch in name.chars() {
        let boundary = !ch.is_alphabetic() || (ch.is_uppercase() && prev_lower);
        if boundary && !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
        if ch.is_alphabetic() {
            current.extend(ch.to_lowercase());
        }
        prev_lower = ch.is_lowercase();
    }
    if !current.is_empty() {
        out.push(current);
    }
    out.retain(|w| w.chars().count() > 1 && !FILLER.contains(&w.as_str()));
    out.dedup();
    out
}

fn same_word(a: &str, b: &str) -> bool {
    a == b || (a.len().min(b.len()) >= 4 && (a.starts_with(b) || b.starts_with(a)))
}

fn family(w: &str) -> Option<usize> {
    FAMILIES.iter().position(|f| f.iter().any(|x| same_word(x, w)))
}

/// 0..1: the share of the Painter name's words that match our material's name or tags (a family match counts
/// half). 0 when the Painter name has no words.
pub fn name_score(painter: &str, name: &str, tags: &[String]) -> f32 {
    let theirs = words(painter);
    if theirs.is_empty() {
        return 0.0;
    }
    let mut ours = words(name);
    for t in tags {
        ours.extend(words(t));
    }
    let mut score = 0.0;
    for w in &theirs {
        if ours.iter().any(|o| same_word(o, w)) {
            score += 1.0;
        } else if let Some(f) = family(w)
            && ours.iter().any(|o| family(o) == Some(f))
        {
            score += 0.5;
        }
    }
    (score / theirs.len() as f32).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(t: &[&str]) -> Vec<String> {
        t.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn names_split_into_words() {
        assert_eq!(words("Rust_Fine"), ["rust", "fine"]);
        assert_eq!(words("rustFine 02"), ["rust", "fine"]);
        assert_eq!(words("PW_WoodLog"), ["pw", "wood", "log"]);
        assert_eq!(words("New_Graph"), Vec::<String>::new());
        assert_eq!(words("steel_002_bitmap"), ["steel"]);
    }

    #[test]
    fn exact_prefix_and_family_matches() {
        let iron = ("Rusted iron", tags(&["metal", "iron", "rust", "worn"]));
        let oak = ("Oak", tags(&["wood", "oak"]));
        assert_eq!(name_score("rust_fine", iron.0, &iron.1), 0.5);
        assert_eq!(name_score("iron_raw_damaged", iron.0, &iron.1), 1.0 / 3.0);
        assert_eq!(name_score("rust_fine", oak.0, &oak.1), 0.0);
        assert_eq!(name_score("steel_rough", iron.0, &iron.1), 0.25, "steel is a metal");
        assert_eq!(
            name_score("Carved_Wood", oak.0, &oak.1),
            0.75,
            "wood matches, carved is a wood word"
        );
        assert_eq!(name_score("New_Graph", oak.0, &oak.1), 0.0);
    }
}

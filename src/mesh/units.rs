//! [`LengthUnit`]: what one mesh unit measures, for displacement in real-world units (Paint mode VS-08). glTF
//! meshes are in metres; OBJ files don't say, so their unit is unknown until the user sets it.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LengthUnit {
    Millimetre,
    Centimetre,
    Metre,
    Inch,
    Foot,
}

impl LengthUnit {
    pub const ALL: [(LengthUnit, &'static str); 5] = [
        (LengthUnit::Millimetre, "Millimetres"),
        (LengthUnit::Centimetre, "Centimetres"),
        (LengthUnit::Metre, "Metres"),
        (LengthUnit::Inch, "Inches"),
        (LengthUnit::Foot, "Feet"),
    ];

    /// Metres per unit.
    pub fn metres(self) -> f32 {
        match self {
            LengthUnit::Millimetre => 0.001,
            LengthUnit::Centimetre => 0.01,
            LengthUnit::Metre => 1.0,
            LengthUnit::Inch => 0.0254,
            LengthUnit::Foot => 0.3048,
        }
    }

    /// The id stored in files.
    pub fn id(self) -> &'static str {
        match self {
            LengthUnit::Millimetre => "mm",
            LengthUnit::Centimetre => "cm",
            LengthUnit::Metre => "m",
            LengthUnit::Inch => "in",
            LengthUnit::Foot => "ft",
        }
    }

    pub fn from_id(id: &str) -> Option<LengthUnit> {
        LengthUnit::ALL.iter().map(|(u, _)| *u).find(|u| u.id() == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip() {
        for (u, _) in LengthUnit::ALL {
            assert_eq!(LengthUnit::from_id(u.id()), Some(u));
        }
        assert_eq!(LengthUnit::from_id("parsec"), None);
    }
}

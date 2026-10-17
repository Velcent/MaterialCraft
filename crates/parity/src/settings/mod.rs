//! Baking settings: Painter's ([`painter`]), what they ask for as high-poly input ([`HighPolyRequest`]),
//! and how they map onto ours ([`Params`], [`mapping`]).

mod bakers;
pub mod mapping;
pub mod painter;
mod reader;
mod request;
#[cfg(test)]
mod tests;

pub use mapping::Unmapped;
pub use painter::{BAKING_INI, MaterialBaking, PainterBaking, TweakList, TweakValue};
pub use request::HighPolyRequest;

use materialcraft::bake::BakeSettings;

/// Most samples per texel side used to match a larger stored reference (memory: 4 x the texels).
const MAX_REFERENCE_SAMPLES: u32 = 2;

/// What one case runs with: our bake settings plus what of Painter's could not be mapped.
#[derive(Debug, Clone)]
pub struct Params {
    /// Output size (square), also the size everything is compared at.
    pub size: usize,
    pub bake: BakeSettings,
    pub unmapped: Vec<Unmapped>,
}

impl Params {
    fn new(bake: BakeSettings, unmapped: Vec<Unmapped>) -> Params {
        Params {
            size: bake.size,
            bake,
            unmapped,
        }
    }

    /// Painter's maps stored larger than compared are area-averaged down, which antialiases them: sample our
    /// texels as finely (up to 2 x 2) so both sides are averaged alike.
    pub fn match_reference_size(&mut self, stored: usize) {
        let ratio = (stored / self.size.max(1)).clamp(1, MAX_REFERENCE_SAMPLES as usize) as u32;
        self.bake.samples = self.bake.samples.max(ratio);
    }

    /// Unmapped settings that concern the baker of `usage` (plus the shared ones).
    pub fn unmapped_for(&self, usage: Option<u64>) -> Vec<&str> {
        self.unmapped
            .iter()
            .filter(|u| u.usage.is_none() || u.usage == usage)
            .map(|u| u.text.as_str())
            .collect()
    }

    /// A short hash of the settings our side ran with (FNV-1a), to tell runs with different settings apart.
    pub fn hash(&self) -> String {
        let text = format!("{}|{:?}", self.size, self.bake);
        let h = text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, c| {
            (h ^ u64::from(c)).wrapping_mul(0x0100_0000_01b3)
        });
        format!("{h:016x}")
    }
}

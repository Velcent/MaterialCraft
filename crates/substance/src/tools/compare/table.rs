//! [`CompareTable`]: the result of `mcsub compare`, one row per texture set × exported map.

use super::metrics::{Diff, Fit};
use std::fmt;

#[derive(Debug, Clone)]
pub struct CompareRow {
    pub set: String,
    /// The export's map name, e.g. `basecolor` or `orm[1]`.
    pub map: String,
    pub file: String,
    /// The export is newer than the project file (so it shows the saved state); `None` if the project's
    /// time is unknown.
    pub newer: Option<bool>,
    pub diff: Diff,
    /// Mean absolute error against the vertically flipped export (row-order check).
    pub flipped_mae: f64,
    pub fit: Fit,
}

#[derive(Debug, Clone, Default)]
pub struct CompareTable {
    pub rows: Vec<CompareRow>,
    /// Sets without exports, unreadable files and similar.
    pub notes: Vec<String>,
}

const LEGEND: &str = "MAE, max and flipMAE in 8-bit steps (1 = 1/255), over texels inside UV islands without a \
1-texel border; flipMAE = against the upside-down export; fit: export ≈ gain × ours + offset (first component)";

impl fmt::Display for CompareTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{:<18} {:<14} {:>9} {:>7} {:>5} {:>6} {:>7} {:>7} {:>14}  file",
            "set", "map", "texels", "MAE", "max", "PSNR", "<=2/255", "flipMAE", "fit gain/off"
        )?;
        for r in &self.rows {
            writeln!(
                f,
                "{:<18} {:<14} {:>9} {:>7.2} {:>5.0} {:>6.1} {:>6.1}% {:>7.2} {:>7.3}/{:<6.3}  {}{}",
                r.set,
                r.map,
                r.diff.texels,
                r.diff.mae * 255.0,
                r.diff.max * 255.0,
                r.diff.psnr,
                r.diff.within,
                r.flipped_mae * 255.0,
                r.fit.gain,
                r.fit.offset,
                r.file,
                if r.newer == Some(false) {
                    " (older than project)"
                } else {
                    ""
                }
            )?;
        }
        writeln!(f, "{LEGEND}")?;
        for n in &self.notes {
            writeln!(f, "note: {n}")?;
        }
        Ok(())
    }
}

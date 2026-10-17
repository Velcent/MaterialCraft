//! The three images of a measured case as PNGs in the run folder: Painter's (`theirs`), ours, and the
//! error heatmap (the substance crate's: black → red → yellow at 16/255, dark blue outside the islands).

use crate::run::CaseImages;
use materialcraft::tex::Tex;
use materialcraft_substance::tools::compare::write_heatmap;
use materialcraft_substance::tools::png_out::PngMap;
use std::io;
use std::path::Path;

/// Paths of a case's images, relative to the report page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseFiles {
    pub theirs: String,
    pub ours: String,
    pub diff: String,
}

/// Writes `images/<stem>_{theirs,ours,diff}.png` below `run_dir`.
pub fn write_case(run_dir: &Path, stem: &str, images: &CaseImages) -> io::Result<CaseFiles> {
    let dir = run_dir.join("images");
    std::fs::create_dir_all(&dir)?;
    let rel = |kind: &str| format!("images/{stem}_{kind}.png");
    let files = CaseFiles {
        theirs: rel("theirs"),
        ours: rel("ours"),
        diff: rel("diff"),
    };
    let png = PngMap { gray: false, bits: 8 };
    png.write(&opaque(images.theirs), &run_dir.join(&files.theirs))?;
    png.write(&opaque(images.ours), &run_dir.join(&files.ours))?;
    write_heatmap(
        images.errors,
        images.mask,
        images.ours.w,
        images.ours.h,
        &run_dir.join(&files.diff),
    )?;
    Ok(files)
}

/// RGB only (alpha would hide parts of maps that use it for something else).
fn opaque(tex: &Tex) -> Tex {
    tex.map(|p| [p[0], p[1], p[2], 1.0])
}

/// A file-name-safe stem: letters, digits, `-` and `_` only.
pub fn stem(index: usize, parts: &[&str]) -> String {
    let joined: String = parts
        .join("_")
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' })
        .collect();
    format!("{index:04}_{joined}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_three_pngs() {
        let dir = std::env::temp_dir().join(format!("mcparity-images-{}", std::process::id()));
        let tex = Tex::solid(4, 4, [0.5, 0.5, 0.5, 0.0]);
        let images = CaseImages {
            ours: &tex,
            theirs: &tex,
            mask: &[true; 16],
            errors: &[0.0; 16],
        };
        let files = write_case(&dir, &stem(3, &["baked.ao", "Vase / Cup"]), &images).unwrap();
        assert_eq!(files.ours, "images/0003_baked_ao_Vase___Cup_ours.png");
        for f in [&files.theirs, &files.ours, &files.diff] {
            assert!(dir.join(f).is_file());
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

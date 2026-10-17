// SPDX-License-Identifier: MIT OR Apache-2.0
use std::{env, fs, path::PathBuf};

/// Embeds assets/MaterialCraftLogo.png (or one in the project root) when it exists.
fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let out = PathBuf::from(env::var("OUT_DIR").unwrap_or_default());
    let mut found: Option<PathBuf> = None;
    for dir in [root.join("assets"), root.clone()] {
        if let Ok(rd) = fs::read_dir(&dir) {
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().to_lowercase();
                if n.starts_with("materialcraftlogo") && n.ends_with(".png") {
                    found = Some(e.path());
                }
            }
        }
    }
    println!("cargo:rerun-if-changed=assets");
    println!("cargo:rerun-if-changed=build.rs");
    let code = match found {
        Some(p) => {
            println!("cargo:rerun-if-changed={}", p.display());
            format!("pub const LOGO_PNG: Option<&[u8]> = Some(include_bytes!({:?}));", p.to_string_lossy())
        }
        None => "pub const LOGO_PNG: Option<&[u8]> = None;".to_string(),
    };
    let _ = fs::write(out.join("logo.rs"), code);
}

// Copied from PhotoCraft, apps/photocraft/src/logging.rs (`Filter`, `parse_level`, `matches` and
// their tests), revision 57437ea. Adapted: MaterialCraft's crate names. See ATTRIBUTION.md.
//
// Copyright (c) 2026 ArtCraft Team and the PhotoCraft contributors.
// Licensed under either of the Apache License, Version 2.0 or the MIT license, at your option.

//! Per-target level filter parsed from `RUST_LOG`-style directives.

use log::LevelFilter;

/// The built-in filter when `RUST_LOG` is unset or empty: our own crates at info, everything
/// else (glow, winit, the portal) at warn.
pub const DEFAULT_FILTER: &str = "warn,materialcraft*=info";

/// Per-target level filter parsed from env_logger-style directives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filter {
    default: LevelFilter,
    directives: Vec<(String, LevelFilter)>,
}

impl Filter {
    /// Parse `spec`; unknown levels and empty parts are skipped. Without a bare level, targets no
    /// directive names log errors only.
    pub fn parse(spec: &str) -> Filter {
        let mut f = Filter { default: LevelFilter::Error, directives: Vec::new() };
        for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            match part.split_once('=') {
                Some((name, level)) => {
                    let name = name.trim();
                    if let (false, Some(level)) = (name.is_empty(), parse_level(level)) {
                        f.directives.push((name.to_owned(), level));
                    }
                }
                None => match parse_level(part) {
                    Some(level) => f.default = level,
                    // A bare target name: everything from it (env_logger does the same).
                    None => f.directives.push((part.to_owned(), LevelFilter::Trace)),
                },
            }
        }
        f
    }

    /// The filter from `RUST_LOG`, or [`DEFAULT_FILTER`] when it is unset or empty.
    pub fn from_env() -> Filter {
        let spec = std::env::var("RUST_LOG").ok().filter(|s| !s.trim().is_empty());
        Filter::parse(spec.as_deref().unwrap_or(DEFAULT_FILTER))
    }

    /// The level that applies to `target` (the most specific matching directive wins).
    pub fn level_for(&self, target: &str) -> LevelFilter {
        self.directives.iter().filter(|(name, _)| matches(name, target)).max_by_key(|(name, _)| name.len()).map_or(self.default, |(_, level)| *level)
    }

    /// The most verbose level any target can reach (for `log::set_max_level`).
    pub fn max(&self) -> LevelFilter {
        self.directives.iter().map(|(_, level)| *level).fold(self.default, Ord::max)
    }
}

fn parse_level(s: &str) -> Option<LevelFilter> {
    s.trim().parse().ok()
}

/// `name` matches `target` itself and its submodules (`a` matches `a` and `a::b`, not `ab`);
/// `name*` matches every target starting with `name`.
fn matches(name: &str, target: &str) -> bool {
    match name.strip_suffix('*') {
        Some(prefix) => target.starts_with(prefix),
        None => target.strip_prefix(name).is_some_and(|rest| rest.is_empty() || rest.starts_with("::")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_filter_shows_materialcraft_info_and_other_crates_warnings() {
        let f = Filter::parse(DEFAULT_FILTER);
        assert_eq!(f.level_for("materialcraft"), LevelFilter::Info);
        assert_eq!(f.level_for("materialcraft_editor::gui_paint"), LevelFilter::Info);
        assert_eq!(f.level_for("materialcraft_substance::read"), LevelFilter::Info);
        assert_eq!(f.level_for("egui_glow::painter"), LevelFilter::Warn);
        assert_eq!(f.level_for("winit"), LevelFilter::Warn);
        assert_eq!(f.max(), LevelFilter::Info);
    }

    #[test]
    fn directives_follow_env_logger_and_the_most_specific_one_wins() {
        let f = Filter::parse("info,glow=error,materialcraft_editor=trace,materialcraft_editor::logging=off");
        assert_eq!(f.level_for("eframe"), LevelFilter::Info);
        assert_eq!(f.level_for("glow::gl46"), LevelFilter::Error);
        assert_eq!(f.level_for("materialcraft_editor::gui_paint"), LevelFilter::Trace);
        assert_eq!(f.level_for("materialcraft_editor::logging"), LevelFilter::Off);
        assert_eq!(f.max(), LevelFilter::Trace);
        // A module name is matched at `::` boundaries, not as a bare prefix.
        assert_eq!(f.level_for("glow_extra"), LevelFilter::Info);
        // A bare target name sets that target to the most verbose level, as env_logger does.
        assert_eq!(Filter::parse("winit").level_for("winit::platform"), LevelFilter::Trace);
        assert_eq!(Filter::parse("winit").level_for("eframe"), LevelFilter::Error);
    }

    #[test]
    fn hostile_specs_never_panic_and_fall_back_sensibly() {
        for spec in ["", ",,,", "=", "=debug", "materialcraft=", "nonsense=loud", "🦀=info", "*", "a=b=c", "warn,,materialcraft=DEBUG"] {
            let f = Filter::parse(spec);
            let _ = f.level_for("materialcraft");
            let _ = f.max();
        }
        assert_eq!(Filter::parse("warn,,materialcraft=DEBUG").level_for("materialcraft"), LevelFilter::Debug);
        assert_eq!(Filter::parse("nonsense=loud").level_for("nonsense"), LevelFilter::Error);
    }
}

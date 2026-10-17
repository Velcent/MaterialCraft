//! [`RunDir`]: the cache folder of one run (`$XDG_CACHE_HOME/materialcraft-parity/<run id>`), outside the repo:
//! report pages and images show the user's art and must never end up in version control.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct RunDir {
    pub id: String,
    pub path: PathBuf,
}

impl RunDir {
    /// A new folder under `root` (default: the user's cache folder), named after the current UTC time.
    pub fn create(root: Option<&Path>) -> std::io::Result<RunDir> {
        let root = match root {
            Some(r) => r.to_path_buf(),
            None => default_root().ok_or_else(|| std::io::Error::other("neither XDG_CACHE_HOME nor HOME is set"))?,
        };
        let id = run_id(SystemTime::now());
        let path = root.join(&id);
        std::fs::create_dir_all(&path)?;
        Ok(RunDir { id, path })
    }
}

fn default_root() -> Option<PathBuf> {
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    Some(cache.join("materialcraft-parity"))
}

/// `20261017-142530` (UTC).
pub fn run_id(time: SystemTime) -> String {
    let secs = time.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let (y, m, d) = civil_date((secs / 86_400) as i64);
    let s = secs % 86_400;
    format!("{y:04}{m:02}{d:02}-{:02}{:02}{:02}", s / 3600, s / 60 % 60, s % 60)
}

/// ISO 8601 UTC timestamp (`2026-10-17T14:25:30Z`).
pub fn iso_time(time: SystemTime) -> String {
    let id = run_id(time);
    format!(
        "{}-{}-{}T{}:{}:{}Z",
        &id[0..4],
        &id[4..6],
        &id[6..8],
        &id[9..11],
        &id[11..13],
        &id[13..15]
    )
}

/// Days since 1970-01-01 → (year, month, day); Howard Hinnant's algorithm.
fn civil_date(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn formats_utc_times() {
        let t = UNIX_EPOCH + Duration::from_secs(1_792_247_130);
        assert_eq!(run_id(t), "20261017-142530");
        assert_eq!(iso_time(t), "2026-10-17T14:25:30Z");
        assert_eq!(run_id(UNIX_EPOCH), "19700101-000000");
        assert_eq!(civil_date(-1), (1969, 12, 31));
    }
}

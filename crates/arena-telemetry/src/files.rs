//! Where the lines lie: `<dir>/<service>/<YYYY-MM-DD>.jsonl`, a file a service and UTC day, and how long they stay
//! (`Retention`: 14 days, 2 GB in all, the oldest day deleted first).

use crate::time;
use std::path::{Path, PathBuf};

/// The variable naming the directory every process writes its telemetry to. Unset or empty: none is written.
pub const ENV: &str = "ARENA_TELEMETRY_DIR";

/// The directory on the kiosk, where the live server, the watchdog and the smoke test meet:
/// `~/services/arena/telemetry`.
pub fn default_dir() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join("services/arena/telemetry")
}

/// `ARENA_TELEMETRY_DIR`, when set and not empty.
pub fn env_dir() -> Option<PathBuf> {
    std::env::var_os(ENV)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// `ARENA_TELEMETRY_DIR`, else `default_dir()`: where a reader looks.
pub fn dir() -> PathBuf {
    env_dir().unwrap_or_else(default_dir)
}

/// The file of `service` on `day` (`2026-11-18`).
pub fn path(dir: &Path, service: &str, day: &str) -> PathBuf {
    dir.join(service).join(format!("{day}.jsonl"))
}

/// One file of telemetry: its service, its day and its size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct File {
    pub service: String,
    pub day: String,
    pub path: PathBuf,
    pub bytes: u64,
}

/// Whether `name` is a day file's name, `YYYY-MM-DD.jsonl`; its day.
fn day_of(name: &str) -> Option<&str> {
    let day = name.strip_suffix(".jsonl")?;
    (day.len() == 10 && time::parse_ts(day).is_some()).then_some(day)
}

/// Every day file under `dir`, oldest day first (then by service). A missing `dir` has none.
pub fn list(dir: &Path) -> std::io::Result<Vec<File>> {
    let mut out = Vec::new();
    let services = match std::fs::read_dir(dir) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e),
    };
    for s in services.flatten() {
        if !s.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let service = s.file_name().to_string_lossy().to_string();
        let Ok(days) = std::fs::read_dir(s.path()) else {
            continue;
        };
        for f in days.flatten() {
            let name = f.file_name().to_string_lossy().to_string();
            let Some(day) = day_of(&name) else { continue };
            let bytes = f.metadata().map_or(0, |m| m.len());
            out.push(File {
                service: service.clone(),
                day: day.to_string(),
                path: f.path(),
                bytes,
            });
        }
    }
    out.sort_by(|a, b| (&a.day, &a.service).cmp(&(&b.day, &b.service)));
    Ok(out)
}

/// How long the files stay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Retention {
    /// Days kept, today included: a file of a day older than this is deleted.
    pub days: i64,
    /// The most all the files may take; past it the oldest days go first.
    pub max_bytes: u64,
}

impl Default for Retention {
    fn default() -> Retention {
        Retention {
            days: 14,
            max_bytes: 2_000_000_000,
        }
    }
}

impl Retention {
    /// Delete what is past the retention, as of `now_us`: the days older than `days`, then the oldest files until the
    /// rest fit in `max_bytes` (today's last of all). Another process deleting the same file at once is no error.
    /// The files deleted.
    pub fn apply(&self, dir: &Path, now_us: i64) -> std::io::Result<Vec<File>> {
        let oldest = time::day(now_us - (self.days - 1) * time::DAY);
        let mut files = list(dir)?;
        let mut gone = Vec::new();
        let mut keep = Vec::new();
        for f in files.drain(..) {
            if f.day < oldest {
                gone.push(f);
            } else {
                keep.push(f);
            }
        }
        let mut total: u64 = keep.iter().map(|f| f.bytes).sum();
        let mut keep = keep.into_iter();
        while total > self.max_bytes {
            let Some(f) = keep.next() else { break };
            total -= f.bytes;
            gone.push(f);
        }
        for f in &gone {
            match std::fs::remove_file(&f.path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
        }
        Ok(gone)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("arena-telemetry-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn write(dir: &Path, service: &str, day: &str, bytes: usize) -> std::io::Result<()> {
        let p = path(dir, service, day);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(p, vec![b'x'; bytes])
    }

    #[test]
    fn old_days_go_then_the_oldest_until_it_fits() -> std::io::Result<()> {
        let dir = temp("retention");
        let now = time::parse_ts("2026-11-18T12:00").unwrap_or(0);
        write(&dir, "arena-server", "2026-11-01", 10)?; // past 14 days
        write(&dir, "arena-server", "2026-11-05", 10)?; // the 14th day: kept by age
        write(&dir, "arena-server", "2026-11-17", 40)?;
        write(&dir, "arena-bot", "2026-11-17", 30)?;
        write(&dir, "arena-server", "2026-11-18", 50)?;
        std::fs::write(dir.join("arena-server/notes.txt"), "not a day")?;
        let r = Retention {
            days: 14,
            max_bytes: 100,
        };
        let gone: Vec<String> = r
            .apply(&dir, now)?
            .into_iter()
            .map(|f| format!("{}/{}", f.service, f.day))
            .collect();
        // by age the 1st; then 140 bytes are left: the 5th and the bot's 17th (oldest, then by service) go
        assert_eq!(
            gone,
            [
                "arena-server/2026-11-01",
                "arena-server/2026-11-05",
                "arena-bot/2026-11-17"
            ]
        );
        let left: Vec<String> = list(&dir)?
            .into_iter()
            .map(|f| format!("{}/{}", f.service, f.day))
            .collect();
        assert_eq!(left, ["arena-server/2026-11-17", "arena-server/2026-11-18"]);
        assert!(dir.join("arena-server/notes.txt").exists());
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }
}

//! A small event log for hand checks: one line per app-level event (activated, board shown, desktop switched…),
//! appended to ~/Library/Logs/MingDesk/events.log on the Mac. Never window titles or other content.

use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Keeps the file small: past this size it starts over.
const MAX_BYTES: u64 = 256 * 1024;

fn path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Library/Logs/MingDesk/events.log"))
}

pub fn event(what: &str) {
    let Some(path) = path() else { return };
    let _ = std::fs::create_dir_all(path.parent().unwrap());
    let restart = std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_BYTES);
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(!restart)
        .write(true)
        .truncate(restart)
        .open(&path);
    if let Ok(mut f) = file {
        let ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis());
        let _ = writeln!(f, "{ms} {what}");
    }
}

//! Keeping the desk live (PLAN.md §6, M1). The platform reports changes as they happen; a refresh thread in
//! lib.rs waits for a burst to settle, then asks `Live::step` whether to take a snapshot and what to tell the
//! board. The decisions live here, away from threads and the OS, so they're tested over the fake platform.
//!
//! The rules: window changes cost a snapshot only while the board (or the switcher) is showing; while it's
//! hidden they just mark the desk stale. Display changes always cost one, so setups and ghosts stay right even
//! when nobody is looking. While showing, a timer refreshes too, as a backstop for apps that don't report.

use crate::desk::{self, Desk, Known};
use crate::platform::Platform;
use std::path::Path;
use std::time::Duration;

/// How long to wait after the first change of a burst (a drag fires dozens) before taking the snapshot.
pub const SETTLE: Duration = Duration::from_millis(100);
/// The backstop timer while the board is showing.
pub const POLL: Duration = Duration::from_secs(2);

/// What happened since the last step.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tick {
    pub windows: bool,
    pub displays: bool,
    /// The backstop timer ran out with nothing reported.
    pub timer: bool,
    /// The board or switcher is on screen.
    pub showing: bool,
}

/// What the refresh thread should do after a step.
#[derive(Debug, Default)]
pub struct Step {
    /// Send this desk to the board.
    pub emit: Option<Desk>,
    /// The remembered displays and setups changed: save them.
    pub save_known: bool,
}

#[derive(Debug, Default)]
pub struct Live {
    last: Option<Desk>,
    /// Windows changed while nobody was looking.
    stale: bool,
}

impl Live {
    pub fn step(
        &mut self,
        tick: Tick,
        platform: &dyn Platform,
        known: &mut Known,
    ) -> Result<Step, String> {
        if !platform.permission().usable() {
            return Ok(Step::default());
        }
        let wanted = tick.displays || (tick.showing && (tick.windows || tick.timer || self.stale));
        if !wanted {
            self.stale |= tick.windows;
            return Ok(Step::default());
        }
        let snapshot = platform.snapshot()?;
        let save_known = known.remember(&snapshot.displays);
        let desk = desk::build(&snapshot, known);
        self.stale = false;
        let changed = self.last.as_ref() != Some(&desk);
        self.last = Some(desk.clone());
        Ok(Step {
            emit: (changed && tick.showing).then_some(desk),
            save_known,
        })
    }
}

/// The remembered displays and setups, or none if the file is missing or unreadable (a fresh start, not an
/// error: MingDesk learns them again as it sees them).
pub fn load(path: &Path) -> Known {
    std::fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice::<Known>(&b).ok())
        .map(Known::tidy)
        .unwrap_or_default()
}

/// Writes through a temporary file, so a crash mid-write never leaves a half file.
pub fn save(path: &Path, known: &Known) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_vec_pretty(known).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::fake::{FakePlatform, display, window};
    use crate::platform::{Permission, Rect};

    const LAPTOP: Rect = Rect::new(0.0, 0.0, 1512.0, 982.0);

    fn fake() -> FakePlatform {
        FakePlatform::with(
            vec![display("laptop", LAPTOP, true)],
            vec![window(1, "TextEdit", "a", LAPTOP, 0)],
        )
    }

    fn taken(p: &FakePlatform) -> u32 {
        *p.snapshots_taken.lock().unwrap()
    }

    const SHOWING: Tick = Tick {
        windows: false,
        displays: false,
        timer: false,
        showing: true,
    };

    #[test]
    fn window_changes_while_hidden_cost_nothing_and_are_caught_up_on_showing() {
        let (p, mut known, mut live) = (fake(), Known::default(), Live::default());
        let step = live
            .step(
                Tick {
                    windows: true,
                    ..Tick::default()
                },
                &p,
                &mut known,
            )
            .unwrap();
        assert!(step.emit.is_none());
        assert_eq!(taken(&p), 0);

        // Shown again with nothing new reported: the stale flag alone brings a refresh.
        let step = live.step(SHOWING, &p, &mut known).unwrap();
        assert_eq!(taken(&p), 1);
        assert_eq!(step.emit.unwrap().displays[0].windows[0].id, 1);
    }

    #[test]
    fn the_board_hears_only_about_real_changes() {
        let (p, mut known, mut live) = (fake(), Known::default(), Live::default());
        assert!(
            live.step(
                Tick {
                    windows: true,
                    ..SHOWING
                },
                &p,
                &mut known
            )
            .unwrap()
            .emit
            .is_some()
        );
        assert!(
            live.step(
                Tick {
                    timer: true,
                    ..SHOWING
                },
                &p,
                &mut known
            )
            .unwrap()
            .emit
            .is_none(),
            "same desk"
        );
        p.snapshot
            .lock()
            .unwrap()
            .windows
            .push(window(2, "Preview", "b.pdf", LAPTOP, 1));
        let desk = live
            .step(
                Tick {
                    timer: true,
                    ..SHOWING
                },
                &p,
                &mut known,
            )
            .unwrap()
            .emit
            .unwrap();
        assert_eq!(desk.displays[0].windows.len(), 2);
        assert_eq!(taken(&p), 3);
    }

    #[test]
    fn display_changes_are_remembered_even_while_hidden() {
        let (p, mut known, mut live) = (fake(), Known::default(), Live::default());
        let step = live
            .step(
                Tick {
                    displays: true,
                    ..Tick::default()
                },
                &p,
                &mut known,
            )
            .unwrap();
        assert!(step.save_known);
        assert!(step.emit.is_none(), "nobody is looking");
        assert_eq!(known.setups.len(), 1);
        assert!(
            !live
                .step(
                    Tick {
                        displays: true,
                        ..Tick::default()
                    },
                    &p,
                    &mut known
                )
                .unwrap()
                .save_known
        );
    }

    #[test]
    fn nothing_happens_without_access() {
        let (p, mut known, mut live) = (fake(), Known::default(), Live::default());
        *p.permission.lock().unwrap() = Permission::Missing;
        let step = live
            .step(
                Tick {
                    displays: true,
                    windows: true,
                    ..SHOWING
                },
                &p,
                &mut known,
            )
            .unwrap();
        assert!(step.emit.is_none() && !step.save_known);
        assert_eq!(taken(&p), 0);
    }

    #[test]
    fn known_displays_survive_a_restart() {
        let dir = std::env::temp_dir().join(format!("mingdesk-test-{}", std::process::id()));
        let path = dir.join("nested").join("known.json");
        assert_eq!(load(&path), Known::default(), "no file yet");
        let mut known = Known::default();
        known.remember(&[display("laptop", LAPTOP, true)]);
        save(&path, &known).unwrap();
        assert_eq!(load(&path), known);
        std::fs::write(&path, b"not json").unwrap();
        assert_eq!(
            load(&path),
            Known::default(),
            "a damaged file means starting fresh"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

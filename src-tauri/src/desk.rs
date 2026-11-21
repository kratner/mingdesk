//! The desk: what the board's List and Desk views show (PLAN.md §8, §9). Built from a platform snapshot plus the
//! displays MingDesk remembers, with no OS calls of its own, so all of it is tested over the fake platform.

use crate::platform::{Display, Platform, Rect, Snapshot, WindowInfo};
use serde::{Deserialize, Serialize};

/// Space left between the real displays and a ghost moved out of their way on the Desk view, in points.
const GHOST_GAP: f64 = 120.0;

/// One display on the board, with its windows.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeskDisplay {
    pub id: String,
    pub name: String,
    /// Where the Desk view draws it: the real frame, or for a ghost a free spot if its old frame is now taken.
    pub frame: Rect,
    pub main: bool,
    /// False for a ghost: a display MingDesk has seen that isn't connected now.
    pub connected: bool,
    /// Stacking order, front-most first; minimized windows last.
    pub windows: Vec<WindowInfo>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Desk {
    /// Connected displays (main first, then left to right, top to bottom), then ghosts.
    pub displays: Vec<DeskDisplay>,
    /// Windows on no connected display, e.g. left behind on a monitor that was unplugged.
    pub offscreen: Vec<WindowInfo>,
    /// Everything the Desk view has to fit: every display drawn, ghosts included.
    pub bounds: Rect,
}

impl Desk {
    pub fn windows(&self) -> impl Iterator<Item = &WindowInfo> {
        self.displays
            .iter()
            .flat_map(|d| d.windows.iter())
            .chain(self.offscreen.iter())
    }
}

/// Every display MingDesk has seen, so a disconnected one can be drawn as a ghost (PLAN.md §9).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct KnownDisplays {
    /// In the order first seen; a display seen again keeps its place and takes its latest name and frame.
    pub displays: Vec<Display>,
}

impl KnownDisplays {
    /// Records the connected displays. Returns true when something changed (worth saving).
    pub fn remember(&mut self, connected: &[Display]) -> bool {
        let mut changed = false;
        for d in connected {
            match self.displays.iter_mut().find(|k| k.id == d.id) {
                Some(k) if k == d => {}
                Some(k) => {
                    *k = d.clone();
                    changed = true;
                }
                None => {
                    self.displays.push(d.clone());
                    changed = true;
                }
            }
        }
        changed
    }

    /// Forgets a ghost the user no longer wants drawn.
    #[allow(dead_code)] // wired to the board's "Forget" on a ghost in the UI step
    pub fn forget(&mut self, id: &str) -> bool {
        let before = self.displays.len();
        self.displays.retain(|d| d.id != id);
        self.displays.len() != before
    }
}

/// The display a window belongs to: the one it overlaps most (PLAN.md §9). None if it's on no display.
pub fn display_for<'a>(frame: &Rect, displays: &'a [Display]) -> Option<&'a Display> {
    displays
        .iter()
        .map(|d| (d, d.frame.overlap(frame)))
        .filter(|(_, area)| *area > 0.0)
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(d, _)| d)
}

fn display_order(a: &Display, b: &Display) -> std::cmp::Ordering {
    b.main
        .cmp(&a.main)
        .then(a.frame.x.total_cmp(&b.frame.x))
        .then(a.frame.y.total_cmp(&b.frame.y))
}

/// Front-most first; minimized windows after every visible one, by app then title.
fn stacking_order(a: &WindowInfo, b: &WindowInfo) -> std::cmp::Ordering {
    a.minimized
        .cmp(&b.minimized)
        .then(if a.minimized {
            a.app.cmp(&b.app).then(a.title.cmp(&b.title))
        } else {
            a.order.cmp(&b.order)
        })
        .then(a.id.cmp(&b.id))
}

/// Builds the desk the board shows from one snapshot and the displays MingDesk remembers.
pub fn build(snapshot: &Snapshot, known: &KnownDisplays) -> Desk {
    let mut connected = snapshot.displays.clone();
    connected.sort_by(display_order);

    let mut displays: Vec<DeskDisplay> = connected
        .iter()
        .map(|d| DeskDisplay {
            id: d.id.clone(),
            name: d.name.clone(),
            frame: d.frame,
            main: d.main,
            connected: true,
            windows: Vec::new(),
        })
        .collect();

    let mut offscreen = Vec::new();
    for w in &snapshot.windows {
        match display_for(&w.frame, &connected) {
            Some(d) => displays
                .iter_mut()
                .find(|x| x.id == d.id)
                .unwrap()
                .windows
                .push(w.clone()),
            None => offscreen.push(w.clone()),
        }
    }
    for d in &mut displays {
        d.windows.sort_by(stacking_order);
    }
    offscreen.sort_by(stacking_order);

    let mut bounds = displays.iter().map(|d| d.frame).reduce(|a, b| a.union(&b));
    for ghost in known
        .displays
        .iter()
        .filter(|k| !connected.iter().any(|c| c.id == k.id))
    {
        let mut frame = ghost.frame;
        if displays.iter().any(|d| d.frame.overlap(&frame) > 0.0) {
            // Its old place is taken (by a display connected now, or another ghost): draw it to the right.
            let b = bounds.unwrap_or_default();
            frame = Rect::new(b.right() + GHOST_GAP, b.y, frame.w, frame.h);
        }
        bounds = Some(bounds.map_or(frame, |b| b.union(&frame)));
        displays.push(DeskDisplay {
            id: ghost.id.clone(),
            name: ghost.name.clone(),
            frame,
            main: false,
            connected: false,
            windows: Vec::new(),
        });
    }

    Desk {
        displays,
        offscreen,
        bounds: bounds.unwrap_or_default(),
    }
}

/// The switcher's search (PLAN.md §6, M1): window ids, best first. An empty query lists every window front-most
/// first. Otherwise every word of the query must match the window's app name or title, in order of its letters
/// though not necessarily together; whole-word and start-of-word matches rank higher.
pub fn search(desk: &Desk, query: &str) -> Vec<u32> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let mut hits: Vec<(i64, &WindowInfo)> = desk
        .windows()
        .filter_map(|w| {
            let hay = format!("{} {}", w.app, w.title).to_lowercase();
            words
                .iter()
                .try_fold(0i64, |total, word| {
                    word_score(&hay, word).map(|s| total + s)
                })
                .map(|s| (s, w))
        })
        .collect();
    hits.sort_by(|a, b| b.0.cmp(&a.0).then(stacking_order(a.1, b.1)));
    hits.into_iter().map(|(_, w)| w.id).collect()
}

fn word_score(hay: &str, word: &str) -> Option<i64> {
    if let Some(at) = hay.find(word) {
        let starts_word = at == 0
            || !hay[..at]
                .chars()
                .next_back()
                .is_some_and(char::is_alphanumeric);
        return Some(if starts_word { 300 } else { 200 });
    }
    // Letters in order with gaps ("txed" in "textedit"): weaker, and weaker still the more it skips.
    let mut skipped = 0i64;
    let mut rest = hay.chars();
    for c in word.chars() {
        loop {
            match rest.next() {
                Some(h) if h == c => break,
                Some(_) => skipped += 1,
                None => return None,
            }
        }
    }
    Some((100 - skipped).max(1))
}

/// What the board and the switcher can ask MingDesk to do to a window.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Action {
    Focus { window: u32 },
    Minimize { window: u32 },
    Close { window: u32 },
    BringAppForward { pid: i32 },
}

pub fn perform(platform: &dyn Platform, action: &Action) -> Result<(), String> {
    match *action {
        Action::Focus { window } => platform.focus(window),
        Action::Minimize { window } => platform.minimize(window),
        Action::Close { window } => platform.close(window),
        Action::BringAppForward { pid } => platform.bring_app_forward(pid),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::fake::{FakePlatform, display, window};

    // Keith's desk: the laptop (main) with two externals, one left of it and one above, so x and y go negative.
    const LAPTOP: Rect = Rect::new(0.0, 0.0, 1512.0, 982.0);
    const LEFT: Rect = Rect::new(-2560.0, -458.0, 2560.0, 1440.0);
    const ABOVE: Rect = Rect::new(0.0, -1440.0, 2560.0, 1440.0);

    #[test]
    fn the_test_displays_dont_overlap_as_real_ones_cant() {
        assert_eq!(
            LAPTOP.overlap(&LEFT) + LAPTOP.overlap(&ABOVE) + LEFT.overlap(&ABOVE),
            0.0
        );
    }

    fn three() -> Vec<Display> {
        // Deliberately not in board order.
        vec![
            display("above", ABOVE, false),
            display("laptop", LAPTOP, true),
            display("left", LEFT, false),
        ]
    }

    fn snap(displays: Vec<Display>, windows: Vec<WindowInfo>) -> Snapshot {
        Snapshot { displays, windows }
    }

    fn ids(d: &DeskDisplay) -> Vec<u32> {
        d.windows.iter().map(|w| w.id).collect()
    }

    #[test]
    fn displays_come_main_first_then_left_to_right() {
        let desk = build(&snap(three(), vec![]), &KnownDisplays::default());
        let order: Vec<&str> = desk.displays.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(order, ["laptop", "left", "above"]);
        assert!(desk.displays.iter().all(|d| d.connected));
        assert_eq!(desk.bounds, LAPTOP.union(&LEFT).union(&ABOVE));
    }

    #[test]
    fn each_window_goes_to_the_display_it_overlaps_most() {
        let windows = vec![
            window(
                1,
                "TextEdit",
                "notes.txt",
                Rect::new(100.0, 100.0, 600.0, 400.0),
                0,
            ),
            window(
                2,
                "Preview",
                "plan.pdf",
                Rect::new(-1000.0, 0.0, 800.0, 600.0),
                1,
            ),
            // Spans the laptop and the left display, mostly on the left one.
            window(
                3,
                "Finder",
                "Downloads",
                Rect::new(-500.0, 100.0, 700.0, 400.0),
                2,
            ),
            // Spans the laptop and the display above, mostly on the laptop.
            window(
                4,
                "Safari",
                "Docs",
                Rect::new(200.0, -100.0, 500.0, 600.0),
                3,
            ),
            window(
                5,
                "Terminal",
                "zsh",
                Rect::new(0.0, -1300.0, 900.0, 600.0),
                4,
            ),
        ];
        let desk = build(&snap(three(), windows), &KnownDisplays::default());
        assert_eq!(ids(&desk.displays[0]), [1, 4], "laptop");
        assert_eq!(ids(&desk.displays[1]), [2, 3], "left");
        assert_eq!(ids(&desk.displays[2]), [5], "above");
        assert!(desk.offscreen.is_empty());
    }

    #[test]
    fn a_window_on_no_display_is_offscreen() {
        let lost = window(
            9,
            "Notes",
            "Shopping",
            Rect::new(5000.0, 5000.0, 400.0, 300.0),
            0,
        );
        let desk = build(&snap(three(), vec![lost]), &KnownDisplays::default());
        assert_eq!(desk.offscreen.iter().map(|w| w.id).collect::<Vec<_>>(), [9]);
        assert!(desk.displays.iter().all(|d| d.windows.is_empty()));
    }

    #[test]
    fn windows_are_in_stacking_order_with_minimized_last() {
        let at = Rect::new(10.0, 10.0, 300.0, 200.0);
        let windows = vec![
            WindowInfo {
                minimized: true,
                ..window(1, "Preview", "b.pdf", at, 99)
            },
            window(2, "TextEdit", "back", at, 7),
            WindowInfo {
                minimized: true,
                ..window(3, "Notes", "a", at, 99)
            },
            window(4, "TextEdit", "front", at, 0),
        ];
        let desk = build(
            &snap(vec![display("laptop", LAPTOP, true)], windows),
            &KnownDisplays::default(),
        );
        assert_eq!(ids(&desk.displays[0]), [4, 2, 3, 1]);
    }

    #[test]
    fn a_remembered_display_that_is_unplugged_becomes_a_ghost() {
        let mut known = KnownDisplays::default();
        assert!(known.remember(&three()));
        assert!(!known.remember(&three()), "nothing new the second time");

        // The display above is unplugged: its old place is free, so the ghost stays where it was.
        let laptop_and_left = vec![
            display("laptop", LAPTOP, true),
            display("left", LEFT, false),
        ];
        let desk = build(&snap(laptop_and_left.clone(), vec![]), &known);
        let ghost = desk.displays.last().unwrap();
        assert_eq!(
            (ghost.id.as_str(), ghost.connected, ghost.frame),
            ("above", false, ABOVE)
        );
        assert_eq!(desk.bounds, LAPTOP.union(&LEFT).union(&ABOVE));

        // Plugged back in, it's a real display again and not also a ghost.
        let desk = build(&snap(three(), vec![]), &known);
        assert_eq!(desk.displays.len(), 3);
        assert!(desk.displays.iter().all(|d| d.connected));
    }

    #[test]
    fn a_ghost_whose_place_is_taken_is_drawn_beside_the_desk() {
        let mut known = KnownDisplays::default();
        known.remember(&[
            display("laptop", LAPTOP, true),
            display("old", Rect::new(1512.0, 0.0, 1920.0, 1080.0), false),
        ]);
        // A different monitor now sits where the old one was.
        let now = vec![
            display("laptop", LAPTOP, true),
            display("new", Rect::new(1512.0, 0.0, 2560.0, 1440.0), false),
        ];
        let desk = build(&snap(now, vec![]), &known);
        let ghost = desk.displays.iter().find(|d| d.id == "old").unwrap();
        assert_eq!(
            ghost.frame,
            Rect::new(1512.0 + 2560.0 + GHOST_GAP, 0.0, 1920.0, 1080.0)
        );
        assert!(
            desk.displays
                .iter()
                .filter(|d| d.id != "old")
                .all(|d| d.frame.overlap(&ghost.frame) == 0.0)
        );
        assert_eq!(desk.bounds.right(), ghost.frame.right());
    }

    #[test]
    fn remembering_a_moved_display_updates_it_and_forget_drops_it() {
        let mut known = KnownDisplays::default();
        known.remember(&three());
        let moved = display("left", Rect::new(-1920.0, 0.0, 1920.0, 1080.0), false);
        assert!(known.remember(std::slice::from_ref(&moved)));
        assert_eq!(known.displays.iter().find(|d| d.id == "left"), Some(&moved));
        assert_eq!(known.displays.len(), 3);
        assert!(known.forget("left"));
        assert!(!known.forget("left"));
    }

    fn search_desk() -> Desk {
        let at = Rect::new(10.0, 10.0, 300.0, 200.0);
        let windows = vec![
            window(1, "TextEdit", "notes.txt", at, 2),
            window(2, "Preview", "Quarterly plan.pdf", at, 0),
            window(3, "Finder", "Downloads", at, 1),
            WindowInfo {
                minimized: true,
                ..window(4, "Preview", "context.pdf", at, 99)
            },
        ];
        build(
            &snap(vec![display("laptop", LAPTOP, true)], windows),
            &KnownDisplays::default(),
        )
    }

    #[test]
    fn empty_search_lists_every_window_front_most_first() {
        assert_eq!(search(&search_desk(), "  "), [2, 3, 1, 4]);
    }

    #[test]
    fn search_matches_app_and_title_and_ranks_word_starts_first() {
        let desk = search_desk();
        // "text" starts the word "TextEdit" but sits inside "context".
        assert_eq!(search(&desk, "text"), [1, 4]);
        assert_eq!(search(&desk, "preview plan"), [2]);
        assert_eq!(
            search(&desk, "PDF"),
            [2, 4],
            "case doesn't matter; ties keep stacking order"
        );
        assert_eq!(search(&desk, "txed"), [1], "letters in order with gaps");
        assert!(search(&desk, "zebra").is_empty());
    }

    #[test]
    fn actions_reach_the_platform_and_unknown_windows_fail() {
        let p = FakePlatform::with(
            vec![display("laptop", LAPTOP, true)],
            vec![window(7, "TextEdit", "a", LAPTOP, 0)],
        );
        let pid = p.snapshot.lock().unwrap().windows[0].pid;
        perform(&p, &Action::Focus { window: 7 }).unwrap();
        perform(&p, &Action::Minimize { window: 7 }).unwrap();
        perform(&p, &Action::Close { window: 7 }).unwrap();
        perform(&p, &Action::BringAppForward { pid }).unwrap();
        assert!(perform(&p, &Action::Close { window: 8 }).is_err());
        assert_eq!(
            *p.actions.lock().unwrap(),
            [
                "focus 7",
                "minimize 7",
                "close 7",
                format!("forward {pid}").as_str(),
                "close 8"
            ]
        );
    }

    #[test]
    fn actions_and_the_desk_speak_the_boards_json() {
        let a: Action = serde_json::from_str(r#"{"kind":"bringAppForward","pid":42}"#).unwrap();
        assert_eq!(a, Action::BringAppForward { pid: 42 });
        let json = serde_json::to_value(search_desk()).unwrap();
        let w = &json["displays"][0]["windows"][0];
        assert_eq!(w["onCurrentDesktop"], true);
        assert_eq!(w["appId"], "com.example.preview");
        assert_eq!(json["displays"][0]["frame"]["w"], 1512.0);
    }
}

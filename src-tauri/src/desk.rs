//! The desk: what the board's List and Desk views show (PLAN.md §8, §9). Built from a platform snapshot plus the
//! displays MingDesk remembers, with no OS calls of its own, so all of it is tested over the fake platform.

use crate::platform::{Desktop, Display, Platform, Rect, Snapshot, WindowInfo};
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
    /// False for a ghost: a display of the current setup that isn't connected now.
    pub connected: bool,
    /// Stacking order, front-most first; minimized windows last.
    pub windows: Vec<WindowInfo>,
    /// Its desktops and full-screen spaces, in Mission Control's order; empty when the OS won't say.
    pub desktops: Vec<Desktop>,
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
    /// The setup these displays belong to; None before MingDesk has seen any.
    pub setup: Option<SetupSummary>,
    /// Every other setup MingDesk knows, listed under its name rather than drawn.
    pub other_setups: Vec<SetupSummary>,
}

impl Desk {
    pub fn windows(&self) -> impl Iterator<Item = &WindowInfo> {
        self.displays
            .iter()
            .flat_map(|d| d.windows.iter())
            .chain(self.offscreen.iter())
    }
}

/// A display setup: the set of displays connected together, e.g. "Home" (laptop + two externals on the dock)
/// and "Office" (lid closed, one wide monitor). PLAN.md §9.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Setup {
    /// The display ids, sorted and joined: the same set always gets the same id.
    pub id: String,
    /// The display names joined with " + " until the user renames it.
    pub name: String,
    /// Sorted display ids.
    pub displays: Vec<String>,
}

/// The current setup, or another one, as the board lists it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupSummary {
    pub id: String,
    pub name: String,
    pub display_names: Vec<String>,
}

/// Every display and setup MingDesk has seen (PLAN.md §9).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Known {
    /// In the order first seen; a display seen again keeps its place and takes its latest name and frame.
    pub displays: Vec<Display>,
    pub setups: Vec<Setup>,
}

fn set_of(connected: &[Display]) -> Vec<String> {
    let mut ids: Vec<String> = connected.iter().map(|d| d.id.clone()).collect();
    ids.sort();
    ids.dedup();
    ids
}

impl Known {
    /// Records the connected displays, and their set as a setup unless it is part of a known setup with some
    /// displays missing (a monitor that didn't wake, the lid closed at home): those show as ghosts instead,
    /// until the user says it's a setup of its own (`adopt`). Returns true when something changed.
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
        let set = set_of(connected);
        let known = self.setups.iter().any(|s| s.displays == set);
        let part_of_one = self
            .setups
            .iter()
            .any(|s| is_proper_subset(&set, &s.displays));
        if !set.is_empty() && !known && !part_of_one {
            self.add_setup(set);
            changed = true;
        }
        changed
    }

    /// "This is a setup of its own": records the connected set even though it's part of a bigger one.
    pub fn adopt(&mut self, connected: &[Display]) -> bool {
        let set = set_of(connected);
        if set.is_empty() || self.setups.iter().any(|s| s.displays == set) {
            return false;
        }
        self.add_setup(set);
        true
    }

    /// Gives a setup the user's name for it ("Home", "Office"). Blank names are refused.
    pub fn rename_setup(&mut self, id: &str, name: &str) -> bool {
        let name = name.trim();
        match self.setups.iter_mut().find(|s| s.id == id) {
            Some(s) if !name.is_empty() && s.name != name => {
                s.name = name.to_string();
                true
            }
            _ => false,
        }
    }

    /// "Forget this setup": drops it, and any display no other setup uses.
    pub fn forget_setup(&mut self, id: &str) -> bool {
        let before = self.setups.len();
        self.setups.retain(|s| s.id != id);
        let setups = &self.setups;
        self.displays
            .retain(|d| setups.iter().any(|s| s.displays.contains(&d.id)));
        self.setups.len() != before
    }

    fn add_setup(&mut self, set: Vec<String>) {
        let names: Vec<String> = set.iter().map(|id| self.name_of(id)).collect();
        self.setups.push(Setup {
            id: set.join(","),
            name: names.join(" + "),
            displays: set,
        });
    }

    fn name_of(&self, id: &str) -> String {
        self.displays
            .iter()
            .find(|d| d.id == id)
            .map_or_else(|| id.to_string(), |d| d.name.clone())
    }

    /// The setup the connected displays belong to: the exact one, or else the smallest known setup that holds
    /// them all (whose other displays are then missing).
    pub fn current(&self, connected: &[Display]) -> Option<&Setup> {
        let set = set_of(connected);
        self.setups.iter().find(|s| s.displays == set).or_else(|| {
            self.setups
                .iter()
                .filter(|s| is_proper_subset(&set, &s.displays))
                .min_by_key(|s| s.displays.len())
        })
    }

    fn summary(&self, setup: &Setup) -> SetupSummary {
        SetupSummary {
            id: setup.id.clone(),
            name: setup.name.clone(),
            display_names: setup.displays.iter().map(|id| self.name_of(id)).collect(),
        }
    }
}

fn is_proper_subset(small: &[String], big: &[String]) -> bool {
    !small.is_empty() && small.len() < big.len() && small.iter().all(|id| big.contains(id))
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

/// Builds the desk the board shows from one snapshot and the displays and setups MingDesk remembers.
pub fn build(snapshot: &Snapshot, known: &Known) -> Desk {
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
            desktops: snapshot
                .desktops
                .iter()
                .filter(|x| x.display == d.id)
                .cloned()
                .collect(),
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
    let current = known.current(&connected);
    let missing = current.map_or(Vec::new(), |setup| {
        known
            .displays
            .iter()
            .filter(|k| setup.displays.contains(&k.id) && !connected.iter().any(|c| c.id == k.id))
            .collect()
    });
    for ghost in missing {
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
            desktops: Vec::new(),
        });
    }

    Desk {
        displays,
        offscreen,
        bounds: bounds.unwrap_or_default(),
        setup: current.map(|c| known.summary(c)),
        other_setups: known
            .setups
            .iter()
            .filter(|s| current.is_none_or(|c| c.id != s.id))
            .map(|s| known.summary(s))
            .collect(),
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
        Snapshot {
            displays,
            windows,
            desktops: Vec::new(),
        }
    }

    fn ids(d: &DeskDisplay) -> Vec<u32> {
        d.windows.iter().map(|w| w.id).collect()
    }

    #[test]
    fn displays_come_main_first_then_left_to_right() {
        let desk = build(&snap(three(), vec![]), &Known::default());
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
        let desk = build(&snap(three(), windows), &Known::default());
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
        let desk = build(&snap(three(), vec![lost]), &Known::default());
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
            &Known::default(),
        );
        assert_eq!(ids(&desk.displays[0]), [4, 2, 3, 1]);
    }

    #[test]
    fn a_remembered_display_that_is_unplugged_becomes_a_ghost() {
        let mut known = Known::default();
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
        // At home the external is main; when it doesn't wake, the laptop becomes main and takes the origin.
        let ext = display("ext", Rect::new(0.0, 0.0, 2560.0, 1440.0), true);
        let mut known = Known::default();
        known.remember(&[
            ext,
            display("laptop", Rect::new(2560.0, 458.0, 1512.0, 982.0), false),
        ]);
        let desk = build(&snap(vec![display("laptop", LAPTOP, true)], vec![]), &known);
        let ghost = desk.displays.iter().find(|d| d.id == "ext").unwrap();
        assert!(!ghost.connected);
        assert_eq!(
            ghost.frame,
            Rect::new(1512.0 + GHOST_GAP, 0.0, 2560.0, 1440.0)
        );
        assert_eq!(desk.displays[0].frame.overlap(&ghost.frame), 0.0);
        assert_eq!(desk.bounds.right(), ghost.frame.right());
    }

    // The office: lid closed, so no built-in display, and one very wide monitor.
    fn office() -> Vec<Display> {
        vec![display("wide", Rect::new(0.0, 0.0, 5120.0, 1440.0), true)]
    }

    #[test]
    fn home_and_office_are_separate_setups_with_no_ghosts() {
        let mut known = Known::default();
        known.remember(&three());
        assert!(known.remember(&office()), "a new setup");
        assert_eq!(known.setups.len(), 2);

        let at_office = build(&snap(office(), vec![]), &known);
        assert_eq!(
            at_office.displays.len(),
            1,
            "home's displays aren't drawn as ghosts"
        );
        assert_eq!(at_office.setup.as_ref().unwrap().name, "Display wide");
        assert_eq!(at_office.other_setups.len(), 1);
        assert_eq!(
            at_office.other_setups[0].display_names,
            ["Display above", "Display laptop", "Display left"]
        );

        let at_home = build(&snap(three(), vec![]), &known);
        assert!(at_home.displays.iter().all(|d| d.connected));
        assert_eq!(at_home.other_setups[0].name, "Display wide");
    }

    #[test]
    fn lid_closed_at_home_shows_the_laptop_as_a_ghost_until_adopted() {
        let mut known = Known::default();
        known.remember(&three());
        let lid_closed = vec![display("left", LEFT, false), display("above", ABOVE, false)];
        assert!(
            !known.remember(&lid_closed),
            "part of Home, so not a setup of its own"
        );

        let desk = build(&snap(lid_closed.clone(), vec![]), &known);
        let ghosts: Vec<&str> = desk
            .displays
            .iter()
            .filter(|d| !d.connected)
            .map(|d| d.id.as_str())
            .collect();
        assert_eq!(ghosts, ["laptop"]);
        assert_eq!(
            desk.setup.as_ref().unwrap().display_names.len(),
            3,
            "still the Home setup"
        );

        assert!(known.adopt(&lid_closed));
        assert!(!known.adopt(&lid_closed));
        let desk = build(&snap(lid_closed, vec![]), &known);
        assert!(desk.displays.iter().all(|d| d.connected));
        assert_eq!(desk.other_setups.len(), 1, "Home is now another setup");
    }

    #[test]
    fn missing_displays_count_against_the_smallest_setup_that_holds_the_rest() {
        let mut known = Known::default();
        let pair = vec![
            display("laptop", LAPTOP, true),
            display("left", LEFT, false),
        ];
        known.remember(&three());
        known.adopt(&pair);
        // Only the laptop: both setups hold it; the pair is the closer one, so only "left" is a ghost.
        let desk = build(&snap(vec![display("laptop", LAPTOP, true)], vec![]), &known);
        let ghosts: Vec<&str> = desk
            .displays
            .iter()
            .filter(|d| !d.connected)
            .map(|d| d.id.as_str())
            .collect();
        assert_eq!(ghosts, ["left"]);
    }

    #[test]
    fn remembering_a_moved_display_updates_it_and_forgetting_a_setup_drops_its_displays() {
        let mut known = Known::default();
        known.remember(&three());
        let moved = display("left", Rect::new(-1920.0, 0.0, 1920.0, 1080.0), false);
        assert!(known.remember(std::slice::from_ref(&moved)));
        assert_eq!(known.displays.iter().find(|d| d.id == "left"), Some(&moved));
        assert_eq!(known.displays.len(), 3);
        known.remember(&office());
        assert!(known.forget_setup("above,laptop,left"));
        assert!(!known.forget_setup("above,laptop,left"));
        assert_eq!(known.setups.len(), 1);
        assert_eq!(
            known
                .displays
                .iter()
                .map(|d| d.id.as_str())
                .collect::<Vec<_>>(),
            ["wide"]
        );
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
            &Known::default(),
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

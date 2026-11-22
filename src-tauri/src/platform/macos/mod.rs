//! macOS: the Accessibility permission (M0), and seeing and acting on windows (M1).
//!
//! macOS ties the permission to the app's code signature, so it sticks across rebuilds only when every build is
//! signed with the same certificate (scripts/dev-cert.sh makes one for this Mac). Under `tauri dev` the binary
//! isn't an app bundle and macOS charges the permission to whatever launched it (VS Code, Terminal), so check the
//! onboarding with the bundled app (`npm run app`).
//!
//! A snapshot joins three sources: CoreGraphics lists every window front to back on any desktop (cg.rs);
//! Accessibility gives titles and state and does the actions (ax.rs); the window server's private spaces calls
//! say which desktop each window is on (spaces.rs). PLAN.md §2, §7.12.

mod appkit;
mod ax;
mod cf;
mod cg;
mod ffi;
mod spaces;
mod watch;

use super::{Desktop, Permission, Platform, Sink, Snapshot, WindowInfo};
use ax::Element;
use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
use core_foundation::string::{CFString, CFStringRef};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::time::Duration;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> bool;
    static kAXTrustedCheckOptionPrompt: CFStringRef;
}

const ACCESSIBILITY_SETTINGS: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";

/// Smallest window MingDesk lists; smaller layer-0 windows are helpers and slivers, not windows people use.
const MIN_SIDE: f64 = 40.0;
/// Where windows not on screen (other desktops, minimized) start in the stacking order.
const STACK_REST: u32 = 100_000;
/// Time allowed per app to reach its windows on other desktops during a snapshot.
const REACH_BUDGET: Duration = Duration::from_millis(150);

pub struct MacPlatform;

/// Which sources a snapshot uses; everything on in the app, switched off one by one by the probe.
#[derive(Debug, Clone, Copy)]
pub struct Sources {
    /// Reach windows on other desktops through the private remote-token call.
    pub other_desktops: bool,
    /// Read desktops and each window's desktop through the private spaces calls.
    pub desktops: bool,
}

impl Sources {
    pub const ALL: Sources = Sources {
        other_desktops: true,
        desktops: true,
    };
}

/// Where a listed window's details came from (the probe reports it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The app's own Accessibility window list.
    Accessibility,
    /// Reached on another desktop by element number.
    OtherDesktop,
    /// CoreGraphics only: no title, no actions beyond activating the app.
    WindowListOnly,
}

pub fn read(sources: Sources) -> (Snapshot, Vec<Source>) {
    let own = std::process::id() as i32;
    let displays: Vec<_> = cg::displays().into_iter().map(|(_, d)| d).collect();
    let desktops: Vec<Desktop> = if sources.desktops {
        spaces::desktops(&displays)
    } else {
        Vec::new()
    };
    let current: HashSet<u64> = desktops
        .iter()
        .filter(|d| d.current)
        .map(|d| d.id)
        .collect();
    let full_screen_desktops: HashSet<u64> = desktops
        .iter()
        .filter(|d| d.full_screen)
        .map(|d| d.id)
        .collect();

    let all = cg::windows();
    let stack = cg::stacking();
    let candidates: Vec<&cg::CgWindow> = all
        .iter()
        .filter(|w| {
            w.layer == 0
                && w.alpha > 0.0
                && w.frame.w >= MIN_SIDE
                && w.frame.h >= MIN_SIDE
                && w.pid != own
        })
        .collect();
    let pids: BTreeSet<i32> = candidates.iter().map(|w| w.pid).collect();
    let front = appkit::frontmost_pid();

    let mut windows = Vec::new();
    let mut found_by = Vec::new();
    for pid in pids {
        let info = appkit::app_info(pid);
        let app = Element::app(pid);
        let mut by_id: HashMap<u32, (ax::AxWindow, Source)> = app
            .windows()
            .into_iter()
            .filter_map(|e| Some((e.window_id()?, (ax::describe(e), Source::Accessibility))))
            .collect();
        let mine: Vec<&&cg::CgWindow> = candidates.iter().filter(|w| w.pid == pid).collect();
        // With desktops known, a window on none is an off-screen helper (apps park them at the screen's edge);
        // the probe showed Chrome, VS Code, Finder, Slack and iTerm2 each keep one.
        let desktop_of: HashMap<u32, Option<u64>> = mine
            .iter()
            .map(|w| {
                (
                    w.id,
                    if sources.desktops {
                        spaces::desktop_of(w.id)
                    } else {
                        None
                    },
                )
            })
            .collect();
        let parked = |w: &cg::CgWindow| {
            sources.desktops && !desktops.is_empty() && desktop_of[&w.id].is_none()
        };
        let wanted: HashSet<u32> = mine
            .iter()
            .filter(|w| !w.onscreen && !parked(w) && !by_id.contains_key(&w.id))
            .map(|w| w.id)
            .collect();
        if sources.other_desktops && info.regular && !wanted.is_empty() {
            for (id, e) in ax::remote_windows(pid, &wanted, REACH_BUDGET) {
                by_id.insert(id, (ax::describe(e), Source::OtherDesktop));
            }
        }
        let focused = if front == Some(pid) {
            app.element("AXFocusedWindow")
        } else {
            None
        };

        for w in mine {
            let (title, frame, minimized, ax_full_screen, is_focused, source) =
                match by_id.get(&w.id) {
                    Some((ax, _)) if !ax.standard => continue,
                    Some((ax, source)) => (
                        ax.title.clone(),
                        ax.frame.unwrap_or(w.frame),
                        ax.minimized,
                        ax.full_screen,
                        focused.as_ref() == Some(&ax.element),
                        *source,
                    ),
                    // Seen only by CoreGraphics: keep a regular app's window that is off this desktop or minimized;
                    // one on the desktop showing now that Accessibility doesn't know is an invisible helper
                    // (Chrome keeps a 403×84 one).
                    None if info.regular
                        && !w.onscreen
                        && !parked(w)
                        && !desktop_of[&w.id].is_some_and(|d| current.contains(&d)) =>
                    {
                        (
                            w.name.clone(),
                            w.frame,
                            false,
                            false,
                            false,
                            Source::WindowListOnly,
                        )
                    }
                    None => continue,
                };
            let desktop = desktop_of[&w.id];
            let on_current_desktop = match desktop {
                Some(d) if !current.is_empty() => current.contains(&d),
                _ => w.onscreen || minimized,
            };
            windows.push(WindowInfo {
                id: w.id,
                pid,
                app: w.owner.clone(),
                app_id: info.bundle_id.clone(),
                title,
                frame,
                minimized,
                full_screen: ax_full_screen
                    || desktop.is_some_and(|d| full_screen_desktops.contains(&d)),
                on_current_desktop,
                desktop,
                focused: is_focused,
                // On-screen windows in true stacking order; the rest after them.
                order: stack.get(&w.id).copied().unwrap_or(STACK_REST + w.order),
            });
            found_by.push(source);
        }
    }
    // MingDesk's own board, while it's on screen (Keith, 2026-11-21: it helps to see where the board is).
    // Read from CoreGraphics only: Accessibility calls into our own process from here could wait on our own
    // main thread. Actions on it go through Tauri in lib.rs, not through this platform.
    for w in all
        .iter()
        .filter(|w| w.pid == own && w.layer == 0 && w.onscreen && w.frame.w >= MIN_SIDE)
    {
        let desktop = if sources.desktops {
            spaces::desktop_of(w.id)
        } else {
            None
        };
        windows.push(WindowInfo {
            id: w.id,
            pid: own,
            app: "MingDesk".into(),
            app_id: Some("live.fourounces.mingdesk".into()),
            title: if w.name.is_empty() {
                "MingDesk".into()
            } else {
                w.name.clone()
            },
            frame: w.frame,
            minimized: false,
            full_screen: false,
            on_current_desktop: true,
            desktop,
            focused: front == Some(own),
            order: stack.get(&w.id).copied().unwrap_or(STACK_REST + w.order),
        });
        found_by.push(Source::WindowListOnly);
    }
    (
        Snapshot {
            displays,
            windows,
            desktops,
        },
        found_by,
    )
}

/// The Accessibility element for a window, on this desktop or another.
fn element_for(window: u32) -> Result<(i32, Element), String> {
    let w = cg::windows()
        .into_iter()
        .find(|w| w.id == window)
        .ok_or("That window has closed.")?;
    let app = Element::app(w.pid);
    if let Some(e) = app
        .windows()
        .into_iter()
        .find(|e| e.window_id() == Some(window))
    {
        return Ok((w.pid, e));
    }
    ax::remote_windows(w.pid, &HashSet::from([window]), Duration::from_millis(400))
        .into_iter()
        .next()
        .map(|(_, e)| (w.pid, e))
        .ok_or_else(|| format!("{} won't let MingDesk reach that window.", w.owner))
}

impl Platform for MacPlatform {
    fn name(&self) -> &'static str {
        "macOS"
    }

    fn permission(&self) -> Permission {
        // SAFETY: no arguments; safe to call from any thread.
        if unsafe { AXIsProcessTrusted() } {
            Permission::Granted
        } else {
            Permission::Missing
        }
    }

    fn request_permission(&self) {
        // SAFETY: the key is a constant exported by ApplicationServices; the dictionary lives across the call.
        unsafe {
            let key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
            let options = CFDictionary::from_CFType_pairs(&[(key, CFBoolean::true_value())]);
            AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef());
        }
    }

    fn open_permission_settings(&self) -> Result<(), String> {
        std::process::Command::new("open")
            .arg(ACCESSIBILITY_SETTINGS)
            .status()
            .map_err(|e| e.to_string())
            .and_then(|s| {
                if s.success() {
                    Ok(())
                } else {
                    Err(format!("open exited with {s}"))
                }
            })
    }

    fn snapshot(&self) -> Result<Snapshot, String> {
        Ok(read(Sources::ALL).0)
    }

    fn focus(&self, window: u32) -> Result<(), String> {
        let (pid, e) = element_for(window)?;
        if e.boolean("AXMinimized") == Some(true) {
            e.set_bool("AXMinimized", false)?;
        }
        e.perform("AXRaise")?;
        let _ = e.set_bool("AXMain", true);
        appkit::activate(pid, false)
    }

    fn minimize(&self, window: u32) -> Result<(), String> {
        element_for(window)?.1.set_bool("AXMinimized", true)
    }

    fn close(&self, window: u32) -> Result<(), String> {
        let (_, e) = element_for(window)?;
        e.element("AXCloseButton")
            .ok_or("That window has no close button.")?
            .perform("AXPress")
    }

    fn bring_app_forward(&self, pid: i32) -> Result<(), String> {
        appkit::activate(pid, true)
    }

    fn watch(&self, sink: Sink) {
        watch::start(sink);
    }
}

#[cfg(test)]
mod probe {
    use super::*;
    use std::time::Instant;

    /// What MingDesk sees on this Mac, with each source switched on in turn. Reads only; changes nothing.
    /// Needs Accessibility for whatever runs it (VS Code or Terminal):
    /// `cargo test probe_this_mac -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn probe_this_mac() {
        let configs = [
            (
                "public calls only",
                Sources {
                    other_desktops: false,
                    desktops: false,
                },
            ),
            (
                "+ desktops",
                Sources {
                    other_desktops: false,
                    desktops: true,
                },
            ),
            ("+ desktops + other-desktop windows", Sources::ALL),
        ];
        for (label, sources) in configs {
            let started = Instant::now();
            let (snap, found_by) = read(sources);
            let titled = snap.windows.iter().filter(|w| !w.title.is_empty()).count();
            let reachable = found_by
                .iter()
                .filter(|s| **s != Source::WindowListOnly)
                .count();
            println!(
                "{label}: {} windows, {titled} with titles, {reachable} reachable for actions, {} desktops, {:?}",
                snap.windows.len(),
                snap.desktops.len(),
                started.elapsed()
            );
        }

        let (snap, found_by) = read(Sources::ALL);
        println!("\nDisplays:");
        for d in &snap.displays {
            println!("  {} | {} | {:?} | main={}", d.name, d.id, d.frame, d.main);
            for x in snap.desktops.iter().filter(|x| x.display == d.id) {
                println!(
                    "     desktop {} id={} {}{}",
                    x.index,
                    x.id,
                    if x.full_screen {
                        "full screen"
                    } else {
                        "desktop"
                    },
                    if x.current { " (current)" } else { "" }
                );
            }
        }
        println!("\nWindows (front to back):");
        let mut rows: Vec<_> = snap.windows.iter().zip(found_by).collect();
        rows.sort_by_key(|(w, _)| w.order);
        for (w, source) in rows {
            let title: String = w.title.chars().take(48).collect();
            if w.title.chars().count() > 48 {
                println!("         full title: {}", w.title);
            }
            let desktop = w
                .desktop
                .and_then(|id| snap.desktops.iter().find(|d| d.id == id))
                .map_or("?".to_string(), |d| {
                    let display = snap
                        .displays
                        .iter()
                        .position(|x| x.id == d.display)
                        .map_or(0, |i| i + 1);
                    format!(
                        "D{display}.{}{}",
                        d.index,
                        if d.full_screen { "fs" } else { "" }
                    )
                });
            println!(
                "  {:>6} {:<18} {:<48} {:<15?} desk={:<4} here={} min={} fs={} focus={} {:?}",
                w.id,
                w.app.chars().take(18).collect::<String>(),
                title,
                source,
                desktop,
                w.on_current_desktop as u8,
                w.minimized as u8,
                w.full_screen as u8,
                w.focused as u8,
                w.frame
            );
        }
    }

    /// Watches TextEdit windows this test opens itself, minimizes, focuses and closes them, and reports what
    /// the watcher heard at each step. Touches no other window. Needs Accessibility for VS Code or Terminal:
    /// `cargo test watch_hears_its_own_textedit_windows -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn watch_hears_its_own_textedit_windows() {
        use crate::platform::Change;
        use std::sync::{Arc, Mutex};
        use std::thread::sleep;

        let dir = std::env::temp_dir().join(format!("mingdesk-probe-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let open = |name: &str| {
            let path = dir.join(name);
            std::fs::write(&path, "MingDesk probe: safe to close.\n").unwrap();
            std::process::Command::new("open")
                .args(["-a", "TextEdit"])
                .arg(&path)
                .status()
                .unwrap();
        };
        let find = |title: &str| {
            read(Sources::ALL)
                .0
                .windows
                .into_iter()
                .find(|w| w.app == "TextEdit" && w.title == title)
        };
        let wait_for = |title: &str, present: bool| {
            for _ in 0..40 {
                if find(title).is_some() == present {
                    return;
                }
                sleep(Duration::from_millis(250));
            }
            panic!("{title} never became present={present}");
        };

        // Close anything an earlier run of this probe left open (its own files only).
        for w in read(Sources::ALL).0.windows {
            if w.app == "TextEdit" && (w.title == "mingdesk-a.txt" || w.title == "mingdesk-b.txt") {
                let _ = MacPlatform.close(w.id);
            }
        }
        wait_for("mingdesk-a.txt", false);
        wait_for("mingdesk-b.txt", false);

        open("mingdesk-a.txt");
        wait_for("mingdesk-a.txt", true);
        let heard = Arc::new(Mutex::new(Vec::<(Instant, Change)>::new()));
        let log = heard.clone();
        MacPlatform.watch(Arc::new(move |c| {
            log.lock().unwrap().push((Instant::now(), c))
        }));
        sleep(Duration::from_secs(1));

        let mac = MacPlatform;
        let step = |label: &str, act: &dyn Fn()| {
            heard.lock().unwrap().clear();
            let at = Instant::now();
            act();
            sleep(Duration::from_millis(1500));
            let events = heard.lock().unwrap();
            let first = events.first().map(|(t, _)| t.duration_since(at));
            println!(
                "{label:<26} {} events, first after {:?}",
                events.len(),
                first
            );
            events.len()
        };
        let a = find("mingdesk-a.txt").unwrap().id;
        let minimized = step("minimize a", &|| mac.minimize(a).unwrap());
        assert!(find("mingdesk-a.txt").unwrap().minimized);
        let focused = step("focus a (restores it)", &|| mac.focus(a).unwrap());
        let a_now = find("mingdesk-a.txt").unwrap();
        assert!(!a_now.minimized, "restored: {a_now:?}");
        // Bringing another app forward is up to macOS: it lets the app the user is in hand over focus, and the
        // test process is never that app. Reported, not required; the board and switcher are checked by hand.
        println!("{:<26} focused={}", "  after focus", a_now.focused);
        let created = step("open b", &|| open("mingdesk-b.txt"));
        wait_for("mingdesk-b.txt", true);
        let b = find("mingdesk-b.txt").unwrap().id;
        let closed = step("close a and b", &|| {
            mac.close(a).unwrap();
            mac.close(b).unwrap();
        });
        wait_for("mingdesk-a.txt", false);
        wait_for("mingdesk-b.txt", false);
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(
            minimized > 0 && focused > 0 && created > 0 && closed > 0,
            "every step was heard"
        );
    }
}

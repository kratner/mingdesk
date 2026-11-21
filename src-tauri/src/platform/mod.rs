//! What MingDesk needs from the operating system, behind one trait so each OS gets its own implementation
//! (PLAN.md §4). M0 covered the permission MingDesk needs before it can see other apps' windows; M1 adds the
//! snapshot of every display and window, and focusing, minimizing and closing windows.
//!
//! Coordinates: one system throughout (PLAN.md §9), global points with the origin at the top-left of the main
//! display and y growing downwards. That is what CoreGraphics and Accessibility use on the Mac; each platform
//! converts anything else at its edge.

use serde::{Deserialize, Serialize};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(target_os = "macos"))]
mod other;

/// Whether MingDesk may see and control other apps' windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Permission {
    /// Allowed: on the Mac, MingDesk is ticked under Accessibility.
    Granted,
    /// Not yet: the board shows the steps to allow it.
    Missing,
    /// This OS asks for no permission (Windows, Linux on X11).
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    NotNeeded,
}

impl Permission {
    pub fn usable(self) -> bool {
        matches!(self, Permission::Granted | Permission::NotNeeded)
    }
}

/// A rectangle in global points (see the module comment).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }
    pub fn right(&self) -> f64 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }
    /// Area shared with `other`; 0 when they don't touch.
    pub fn overlap(&self, other: &Rect) -> f64 {
        let w = self.right().min(other.right()) - self.x.max(other.x);
        let h = self.bottom().min(other.bottom()) - self.y.max(other.y);
        if w > 0.0 && h > 0.0 { w * h } else { 0.0 }
    }
    /// The smallest rectangle holding both.
    pub fn union(&self, other: &Rect) -> Rect {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Rect::new(
            x,
            y,
            self.right().max(other.right()) - x,
            self.bottom().max(other.bottom()) - y,
        )
    }
}

/// A display the OS reports as connected. Mirrored displays come through once (the platform keeps one of each
/// mirror set), so every display here is a separate place a window can be.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Display {
    /// Stable across unplugging and waking (on the Mac, the display's UUID), unlike the OS's display number.
    pub id: String,
    /// What System Settings calls it, e.g. "Built-in Retina Display" or "DELL U2723QE".
    pub name: String,
    pub frame: Rect,
    /// The display with the menu bar, whose top-left corner is the origin.
    pub main: bool,
}

/// One window as the OS reports it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowInfo {
    /// The OS's window number (CGWindowID on the Mac); what the actions take.
    pub id: u32,
    pub pid: i32,
    pub app: String,
    /// Bundle id on the Mac, executable path elsewhere; None when the OS won't say.
    pub app_id: Option<String>,
    /// Empty when the OS won't say, e.g. a window on another desktop that Accessibility can't see.
    pub title: String,
    pub frame: Rect,
    pub minimized: bool,
    pub full_screen: bool,
    /// False for a window on another Mission Control desktop: listed, but MingDesk can't move it (PLAN.md §8).
    pub on_current_desktop: bool,
    /// The window that has keyboard focus, if it's this one.
    pub focused: bool,
    /// Stacking order, 0 = front-most. Minimized windows have no place in it and come after every visible one.
    pub order: u32,
}

/// Everything the OS reports at one moment.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct Snapshot {
    pub displays: Vec<Display>,
    pub windows: Vec<WindowInfo>,
}

pub trait Platform: Send + Sync {
    /// Short name of the OS, shown in the board's footer.
    fn name(&self) -> &'static str;
    fn permission(&self) -> Permission;
    /// Asks the OS to list MingDesk in its permission settings (and, on the Mac, shows the system prompt once).
    fn request_permission(&self);
    /// Opens the settings page where the user allows MingDesk.
    fn open_permission_settings(&self) -> Result<(), String>;

    /// Every connected display and every app window, now.
    fn snapshot(&self) -> Result<Snapshot, String>;
    /// Brings the window to the front and gives it focus, activating its app (and restoring it if minimized).
    fn focus(&self, window: u32) -> Result<(), String>;
    fn minimize(&self, window: u32) -> Result<(), String>;
    /// Asks the window to close, as its close button would; the app may still ask to save first.
    fn close(&self, window: u32) -> Result<(), String>;
    /// Activates the app with all of its windows in front.
    fn bring_app_forward(&self, pid: i32) -> Result<(), String>;
}

pub fn current() -> Box<dyn Platform> {
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacPlatform)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Box::new(other::OtherPlatform)
    }
}

#[cfg(test)]
pub mod fake {
    use super::*;
    use std::sync::Mutex;

    /// A platform the test sets up (permission, displays, windows), recording what MingDesk asked of it.
    pub struct FakePlatform {
        pub permission: Mutex<Permission>,
        pub requests: Mutex<u32>,
        pub settings_opened: Mutex<u32>,
        pub snapshot: Mutex<Snapshot>,
        /// Every action, as "focus 12", "close 7", "forward 501".
        pub actions: Mutex<Vec<String>>,
    }

    impl FakePlatform {
        pub fn new(permission: Permission) -> Self {
            Self {
                permission: Mutex::new(permission),
                requests: Mutex::new(0),
                settings_opened: Mutex::new(0),
                snapshot: Mutex::new(Snapshot::default()),
                actions: Mutex::new(Vec::new()),
            }
        }

        pub fn with(displays: Vec<Display>, windows: Vec<WindowInfo>) -> Self {
            let p = Self::new(Permission::Granted);
            *p.snapshot.lock().unwrap() = Snapshot { displays, windows };
            p
        }

        fn record(&self, verb: &str, id: i64) -> Result<(), String> {
            self.actions.lock().unwrap().push(format!("{verb} {id}"));
            let snap = self.snapshot.lock().unwrap();
            let known = if verb == "forward" {
                snap.windows.iter().any(|w| i64::from(w.pid) == id)
            } else {
                snap.windows.iter().any(|w| i64::from(w.id) == id)
            };
            if known {
                Ok(())
            } else {
                Err(format!("{verb}: nothing with id {id}"))
            }
        }
    }

    /// A display for tests.
    pub fn display(id: &str, frame: Rect, main: bool) -> Display {
        Display {
            id: id.into(),
            name: format!("Display {id}"),
            frame,
            main,
        }
    }

    /// A visible window for tests; tweak fields with struct update syntax.
    pub fn window(id: u32, app: &str, title: &str, frame: Rect, order: u32) -> WindowInfo {
        WindowInfo {
            id,
            pid: 100 + app.len() as i32,
            app: app.into(),
            app_id: Some(format!("com.example.{}", app.to_lowercase())),
            title: title.into(),
            frame,
            minimized: false,
            full_screen: false,
            on_current_desktop: true,
            focused: false,
            order,
        }
    }

    impl Platform for FakePlatform {
        fn name(&self) -> &'static str {
            "fake"
        }
        fn permission(&self) -> Permission {
            *self.permission.lock().unwrap()
        }
        fn request_permission(&self) {
            *self.requests.lock().unwrap() += 1;
        }
        fn open_permission_settings(&self) -> Result<(), String> {
            *self.settings_opened.lock().unwrap() += 1;
            Ok(())
        }
        fn snapshot(&self) -> Result<Snapshot, String> {
            Ok(self.snapshot.lock().unwrap().clone())
        }
        fn focus(&self, window: u32) -> Result<(), String> {
            self.record("focus", window.into())
        }
        fn minimize(&self, window: u32) -> Result<(), String> {
            self.record("minimize", window.into())
        }
        fn close(&self, window: u32) -> Result<(), String> {
            self.record("close", window.into())
        }
        fn bring_app_forward(&self, pid: i32) -> Result<(), String> {
            self.record("forward", pid.into())
        }
    }

    #[test]
    fn overlap_and_union() {
        let a = Rect::new(0.0, 0.0, 100.0, 100.0);
        assert_eq!(a.overlap(&Rect::new(50.0, 50.0, 100.0, 100.0)), 2500.0);
        assert_eq!(
            a.overlap(&Rect::new(100.0, 0.0, 10.0, 10.0)),
            0.0,
            "touching edges don't overlap"
        );
        assert_eq!(
            a.overlap(&Rect::new(-50.0, -50.0, 60.0, 60.0)),
            100.0,
            "negative coordinates"
        );
        assert_eq!(
            a.union(&Rect::new(-10.0, 120.0, 20.0, 20.0)),
            Rect::new(-10.0, 0.0, 110.0, 140.0)
        );
    }
}

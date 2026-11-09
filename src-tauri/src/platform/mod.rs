//! What MingDesk needs from the operating system, behind one trait so each OS gets its own implementation
//! (PLAN.md §4). M0 covers the permission MingDesk needs before it can see other apps' windows; M1 adds
//! listing, focusing and closing windows.

use serde::Serialize;

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

pub trait Platform: Send + Sync {
    /// Short name of the OS, shown in the board's footer.
    fn name(&self) -> &'static str;
    fn permission(&self) -> Permission;
    /// Asks the OS to list MingDesk in its permission settings (and, on the Mac, shows the system prompt once).
    fn request_permission(&self);
    /// Opens the settings page where the user allows MingDesk.
    fn open_permission_settings(&self) -> Result<(), String>;
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

    /// A platform whose permission the test sets, recording what MingDesk asked of it.
    pub struct FakePlatform {
        pub permission: Mutex<Permission>,
        pub requests: Mutex<u32>,
        pub settings_opened: Mutex<u32>,
    }

    impl FakePlatform {
        pub fn new(permission: Permission) -> Self {
            Self { permission: Mutex::new(permission), requests: Mutex::new(0), settings_opened: Mutex::new(0) }
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
    }
}

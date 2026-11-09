//! Windows and Linux (X11) let an app list and place other apps' windows without a permission prompt
//! (PLAN.md §2). Their window code arrives in M6 and M7; until then MingDesk runs there with an empty board.

use super::{Permission, Platform};

pub struct OtherPlatform;

impl Platform for OtherPlatform {
    fn name(&self) -> &'static str {
        if cfg!(target_os = "windows") { "Windows" } else { "Linux" }
    }
    fn permission(&self) -> Permission {
        Permission::NotNeeded
    }
    fn request_permission(&self) {}
    fn open_permission_settings(&self) -> Result<(), String> {
        Ok(())
    }
}

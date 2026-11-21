//! Windows and Linux (X11) let an app list and place other apps' windows without a permission prompt
//! (PLAN.md §2). Their window code arrives in M6 and M7; until then MingDesk runs there with an empty board.

use super::{Permission, Platform, Snapshot};

const NOT_YET: &str = "Window control on this OS arrives with its milestone (PLAN.md §6).";

pub struct OtherPlatform;

impl Platform for OtherPlatform {
    fn name(&self) -> &'static str {
        if cfg!(target_os = "windows") {
            "Windows"
        } else {
            "Linux"
        }
    }
    fn permission(&self) -> Permission {
        Permission::NotNeeded
    }
    fn request_permission(&self) {}
    fn open_permission_settings(&self) -> Result<(), String> {
        Ok(())
    }

    fn snapshot(&self) -> Result<Snapshot, String> {
        Ok(Snapshot::default())
    }
    fn focus(&self, _window: u32) -> Result<(), String> {
        Err(NOT_YET.into())
    }
    fn minimize(&self, _window: u32) -> Result<(), String> {
        Err(NOT_YET.into())
    }
    fn close(&self, _window: u32) -> Result<(), String> {
        Err(NOT_YET.into())
    }
    fn bring_app_forward(&self, _pid: i32) -> Result<(), String> {
        Err(NOT_YET.into())
    }
}

//! The few AppKit calls MingDesk needs: display names, and running apps (bundle id, regular or not,
//! activating). Sent as plain messages; NSString and NSArray are toll-free bridged with CFString and CFArray.

use super::ffi::NSApplicationActivateAllWindows;
use core_foundation::base::TCFType;
use core_foundation::string::{CFString, CFStringRef};
use objc2::msg_send;
use objc2::rc::autoreleasepool;
use objc2::runtime::{AnyClass, AnyObject};
use std::collections::HashMap;

fn string_of(obj: *mut AnyObject) -> Option<String> {
    // SAFETY: callers pass an NSString (or null), which is a CFString.
    (!obj.is_null())
        .then(|| unsafe { CFString::wrap_under_get_rule(obj as CFStringRef) }.to_string())
}

/// Display names as System Settings shows them, keyed by CoreGraphics display number.
pub fn screen_names() -> HashMap<u32, String> {
    let mut names = HashMap::new();
    let Some(class) = AnyClass::get(c"NSScreen") else {
        return names;
    };
    autoreleasepool(|_| unsafe {
        // SAFETY: documented NSScreen API; reading it off the main thread is what menu-bar tools commonly do.
        let screens: *mut AnyObject = msg_send![class, screens];
        if screens.is_null() {
            return;
        }
        let count: usize = msg_send![screens, count];
        let key = CFString::from_static_string("NSScreenNumber");
        for i in 0..count {
            let screen: *mut AnyObject = msg_send![screens, objectAtIndex: i];
            let info: *mut AnyObject = msg_send![screen, deviceDescription];
            let number: *mut AnyObject =
                msg_send![info, objectForKey: key.as_concrete_TypeRef() as *mut AnyObject];
            if number.is_null() {
                continue;
            }
            let id: u32 = msg_send![number, unsignedIntValue];
            // localizedName arrived in macOS 10.15; on Mojave the board numbers the displays instead.
            let has_name: bool = msg_send![screen, respondsToSelector: objc2::sel!(localizedName)];
            // Some displays (a virtual one, a screen caught mid-switch as the lid closes) give an empty name:
            // number those too.
            if has_name
                && let Some(name) = string_of(msg_send![screen, localizedName])
                && !name.trim().is_empty()
            {
                names.insert(id, name);
            }
        }
    });
    names
}

fn running_app(pid: i32) -> *mut AnyObject {
    let Some(class) = AnyClass::get(c"NSRunningApplication") else {
        return std::ptr::null_mut();
    };
    // SAFETY: documented class method; returns an autoreleased object or nil.
    unsafe { msg_send![class, runningApplicationWithProcessIdentifier: pid] }
}

pub struct AppInfo {
    pub bundle_id: Option<String>,
    /// A normal app with a Dock icon (not a menu-bar helper or background agent).
    pub regular: bool,
}

pub fn app_info(pid: i32) -> AppInfo {
    autoreleasepool(|_| {
        let app = running_app(pid);
        if app.is_null() {
            return AppInfo {
                bundle_id: None,
                regular: false,
            };
        }
        // SAFETY: documented NSRunningApplication properties.
        unsafe {
            let policy: isize = msg_send![app, activationPolicy];
            AppInfo {
                bundle_id: string_of(msg_send![app, bundleIdentifier]),
                regular: policy == 0,
            }
        }
    })
}

/// The process of the app in front, which owns keyboard focus.
pub fn frontmost_pid() -> Option<i32> {
    let class = AnyClass::get(c"NSWorkspace")?;
    autoreleasepool(|_| unsafe {
        // SAFETY: documented NSWorkspace API.
        let workspace: *mut AnyObject = msg_send![class, sharedWorkspace];
        let app: *mut AnyObject = msg_send![workspace, frontmostApplication];
        (!app.is_null()).then(|| msg_send![app, processIdentifier])
    })
}

/// Brings the app forward; with `all_windows`, every one of its windows comes in front of other apps'.
pub fn activate(pid: i32, all_windows: bool) -> Result<(), String> {
    autoreleasepool(|_| {
        let app = running_app(pid);
        if app.is_null() {
            return Err(format!("no running app with process {pid}"));
        }
        let options = if all_windows {
            NSApplicationActivateAllWindows
        } else {
            0
        };
        // SAFETY: documented NSRunningApplication method.
        let ok: bool = unsafe { msg_send![app, activateWithOptions: options] };
        if ok {
            Ok(())
        } else {
            Err("macOS didn't let MingDesk bring that app forward".into())
        }
    })
}

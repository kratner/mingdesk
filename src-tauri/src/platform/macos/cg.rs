//! CoreGraphics: every window the window server knows (front to back, on any desktop) and every display.
//! Needs no permission; window titles come from Accessibility instead, since CoreGraphics only gives them
//! to apps with Screen Recording.

use super::appkit;
use super::cf::{self, Dict};
use super::ffi::*;
use crate::platform::{Display, Rect};
use core_foundation::array::CFArray;
use core_foundation::base::{CFType, TCFType};
use core_foundation::string::{CFString, CFStringRef};

/// One window as CoreGraphics reports it.
#[derive(Debug, Clone)]
pub struct CgWindow {
    pub id: u32,
    pub pid: i32,
    pub owner: String,
    /// Empty without Screen Recording, which MingDesk doesn't ask for.
    pub name: String,
    pub frame: Rect,
    /// 0 for normal app windows; menus, the Dock and overlays sit on other layers.
    pub layer: i64,
    pub alpha: f64,
    /// On the current desktop and not minimized.
    pub onscreen: bool,
    /// Position in the window server's list: front-most first.
    pub order: u32,
}

pub fn windows() -> Vec<CgWindow> {
    // SAFETY: plain C call; the result follows the create rule and may be null.
    let raw = unsafe {
        CGWindowListCopyWindowInfo(
            kCGWindowListOptionAll | kCGWindowListExcludeDesktopElements,
            kCGNullWindowID,
        )
    };
    if raw.is_null() {
        return Vec::new();
    }
    // SAFETY: non-null CFArray of CFDictionary, owned by us (create rule).
    let list: CFArray<Dict> = unsafe { CFArray::wrap_under_create_rule(raw) };
    list.iter()
        .enumerate()
        .filter_map(|(i, d)| {
            let bounds = cf::get(&d, "kCGWindowBounds").and_then(|b| cf::dict(&b))?;
            let num = |k: &str| {
                cf::get(&bounds, k)
                    .and_then(|v| cf::float(&v))
                    .unwrap_or(0.0)
            };
            Some(CgWindow {
                id: cf::int(&cf::get(&d, "kCGWindowNumber")?)? as u32,
                pid: cf::int(&cf::get(&d, "kCGWindowOwnerPID")?)? as i32,
                owner: cf::get(&d, "kCGWindowOwnerName")
                    .and_then(|v| cf::string(&v))
                    .unwrap_or_default(),
                name: cf::get(&d, "kCGWindowName")
                    .and_then(|v| cf::string(&v))
                    .unwrap_or_default(),
                frame: Rect::new(num("X"), num("Y"), num("Width"), num("Height")),
                layer: cf::get(&d, "kCGWindowLayer")
                    .and_then(|v| cf::int(&v))
                    .unwrap_or(-1),
                alpha: cf::get(&d, "kCGWindowAlpha")
                    .and_then(|v| cf::float(&v))
                    .unwrap_or(1.0),
                onscreen: cf::get(&d, "kCGWindowIsOnscreen")
                    .and_then(|v| cf::boolean(&v))
                    .unwrap_or(false),
                order: i as u32,
            })
        })
        .collect()
}

/// Connected displays, one per mirror set, with the CoreGraphics number each has right now (for matching
/// against NSScreen and the spaces list) alongside MingDesk's stable `Display`.
pub fn displays() -> Vec<(CGDirectDisplayID, Display)> {
    let mut ids = [0u32; 32];
    let mut count = 0u32;
    // SAFETY: the buffer holds 32 ids and we pass 32.
    if unsafe { CGGetActiveDisplayList(32, ids.as_mut_ptr(), &mut count) } != 0 {
        return Vec::new();
    }
    let names = appkit::screen_names();
    ids[..count as usize]
        .iter()
        // A display mirroring another is the same place, not a second one (PLAN.md §9).
        .filter(|&&id| unsafe { CGDisplayMirrorsDisplay(id) } == 0)
        .enumerate()
        .map(|(i, &id)| {
            // SAFETY: plain C calls on an id the OS just gave us.
            let b = unsafe { CGDisplayBounds(id) };
            let main = unsafe { CGDisplayIsMain(id) } != 0;
            let display = Display {
                id: uuid_of(id).unwrap_or_else(|| format!("display-{id}")),
                name: names
                    .get(&id)
                    .cloned()
                    .unwrap_or_else(|| format!("Display {}", i + 1)),
                frame: Rect::new(b.origin.x, b.origin.y, b.size.width, b.size.height),
                main,
            };
            (id, display)
        })
        .collect()
}

fn uuid_of(id: CGDirectDisplayID) -> Option<String> {
    // SAFETY: create rule; null when the display has no UUID.
    let uuid = unsafe { CGDisplayCreateUUIDFromDisplayID(id) };
    if uuid.is_null() {
        return None;
    }
    // SAFETY: we own `uuid` (create rule); CFUUIDCreateString also follows the create rule.
    let uuid = unsafe { CFType::wrap_under_create_rule(uuid) };
    let text = unsafe { CFUUIDCreateString(std::ptr::null(), uuid.as_CFTypeRef()) };
    if text.is_null() {
        return None;
    }
    Some(unsafe { CFString::wrap_under_create_rule(text as CFStringRef) }.to_string())
}

//! Desktops (Mission Control spaces) per display, and which window is on which. Private window-server calls
//! (PLAN.md §7.12); if they fail MingDesk reports no desktops and works from the public calls alone.

use super::cf::{self, Dict};
use super::ffi::*;
use crate::platform::{Desktop, Display};
use core_foundation::array::CFArray;
use core_foundation::base::{CFType, TCFType};
use core_foundation::number::CFNumber;

/// Space type for a window's full-screen space.
const FULL_SCREEN: i64 = 4;

fn space_id(d: &Dict) -> Option<u64> {
    cf::get(d, "ManagedSpaceID")
        .or_else(|| cf::get(d, "id64"))
        .and_then(|v| cf::int(&v))
        .map(|v| v as u64)
}

/// Every display's desktops, in Mission Control's order. `displays` maps the identifiers the window server
/// uses (display UUIDs, or "Main" when displays share spaces) to MingDesk's display ids.
pub fn desktops(displays: &[Display]) -> Vec<Desktop> {
    // SAFETY: private but long-stable calls; the result follows the create rule and may be null.
    let raw = unsafe { CGSCopyManagedDisplaySpaces(CGSMainConnectionID()) };
    if raw.is_null() {
        return Vec::new();
    }
    let list: CFArray<Dict> = unsafe { CFArray::wrap_under_create_rule(raw) };
    let main = displays.iter().find(|d| d.main).map(|d| d.id.clone());
    let mut out = Vec::new();
    for entry in list.iter() {
        let Some(ident) = cf::get(&entry, "Display Identifier").and_then(|v| cf::string(&v)) else {
            continue;
        };
        let display = if ident == "Main" {
            match &main {
                Some(m) => m.clone(),
                None => continue,
            }
        } else {
            match displays.iter().find(|d| d.id.eq_ignore_ascii_case(&ident)) {
                Some(d) => d.id.clone(),
                None => continue,
            }
        };
        let current = cf::get(&entry, "Current Space")
            .and_then(|v| cf::dict(&v))
            .and_then(|d| space_id(&d));
        let Some(spaces) = cf::get(&entry, "Spaces").and_then(|v| cf::array(&v)) else {
            continue;
        };
        for (i, space) in spaces.iter().enumerate() {
            let Some(space) = cf::dict(&space) else {
                continue;
            };
            let Some(id) = space_id(&space) else { continue };
            let kind = cf::get(&space, "type")
                .and_then(|v| cf::int(&v))
                .unwrap_or(0);
            out.push(Desktop {
                id,
                display: display.clone(),
                index: i as u32 + 1,
                full_screen: kind == FULL_SCREEN,
                current: current == Some(id),
            });
        }
    }
    out
}

/// The desktop a window is on (the first, for a window shown on every desktop).
pub fn desktop_of(window: u32) -> Option<u64> {
    let ids = CFArray::from_CFTypes(&[CFNumber::from(window as i64)]);
    // SAFETY: private call; create rule, may be null. Mask 7 = every kind of space.
    let raw =
        unsafe { CGSCopySpacesForWindows(CGSMainConnectionID(), 7, ids.as_concrete_TypeRef()) };
    if raw.is_null() {
        return None;
    }
    let spaces: CFArray<CFType> = unsafe { CFArray::wrap_under_create_rule(raw) };
    spaces.iter().find_map(|s| cf::int(&s)).map(|s| s as u64)
}

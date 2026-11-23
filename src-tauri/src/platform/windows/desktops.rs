//! Virtual desktops (Task View).
//!
//! Public: IVirtualDesktopManager says, per window, whether it's on the desktop showing now and which desktop
//! (GUID) it's on. Not public: the list of desktops, their order and names. MingDesk reads the order from the
//! registry key Explorer keeps it in (VirtualDesktopIDs, unchanged since Windows 10), read-only, and falls back to
//! the order windows are first seen in if it's missing. It never moves a window between desktops: Windows only
//! lets an app do that to its own windows (PLAN.md §2).
//!
//! Windows desktops span every monitor, where the Mac's are per display: on the board each desktop is listed under
//! every display with the same number (see `super::read`).

use super::rules;
use ::windows::Win32::Foundation::HWND;
use ::windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx};
use ::windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_BINARY, RegGetValueW};
use ::windows::Win32::UI::Shell::{IVirtualDesktopManager, VirtualDesktopManager};
use ::windows::core::w;

const KEY: ::windows::core::PCWSTR =
    w!(r"Software\Microsoft\Windows\CurrentVersion\Explorer\VirtualDesktops");

/// The desktop manager, for one snapshot or action on one thread.
pub struct Vdm(IVirtualDesktopManager);

impl Vdm {
    pub fn new() -> Option<Self> {
        // SAFETY: joining the multithreaded apartment is harmless if this thread already is in one, and the
        // manager is then usable either way (a thread already single-threaded gets RPC_E_CHANGED_MODE here and
        // keeps its apartment). MingDesk never leaves COM on these threads.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            CoCreateInstance(&VirtualDesktopManager, None, CLSCTX_ALL).ok().map(Vdm)
        }
    }

    /// None when Windows won't say (a window that's closing, or one the manager doesn't track).
    pub fn on_current(&self, h: HWND) -> Option<bool> {
        // SAFETY: a plain query on a window handle.
        unsafe { self.0.IsWindowOnCurrentVirtualDesktop(h) }.ok().map(|b| b.as_bool())
    }

    /// The desktop's GUID; None for a window shown on every desktop, or when Windows won't say.
    pub fn desktop(&self, h: HWND) -> Option<u128> {
        // SAFETY: a plain query on a window handle.
        let g = unsafe { self.0.GetWindowDesktopId(h) }.ok()?.to_u128();
        (g != 0).then_some(g)
    }
}

fn registry_binary(value: ::windows::core::PCWSTR) -> Option<Vec<u8>> {
    let mut len = 0u32;
    // SAFETY: first call asks for the size; the second fills a buffer of that size.
    unsafe {
        RegGetValueW(HKEY_CURRENT_USER, KEY, value, RRF_RT_REG_BINARY, None, None, Some(&mut len))
            .ok()
            .ok()?;
        let mut buf = vec![0u8; len as usize];
        RegGetValueW(
            HKEY_CURRENT_USER,
            KEY,
            value,
            RRF_RT_REG_BINARY,
            None,
            Some(buf.as_mut_ptr() as *mut _),
            Some(&mut len),
        )
        .ok()
        .ok()?;
        buf.truncate(len as usize);
        Some(buf)
    }
}

/// Task View's desktops in order, from the registry; empty if it isn't there.
pub fn registry_order() -> Vec<u128> {
    registry_binary(w!("VirtualDesktopIDs")).map_or(Vec::new(), |b| rules::guid_list(&b))
}

/// The desktop showing now, from the registry (Windows 11 keeps it here; Windows 10 per session elsewhere).
pub fn registry_current() -> Option<u128> {
    let b = registry_binary(w!("CurrentVirtualDesktop"))?;
    let bytes: &[u8; 16] = b.get(..16)?.try_into().ok()?;
    Some(rules::guid_from_bytes(bytes)).filter(|g| *g != 0)
}

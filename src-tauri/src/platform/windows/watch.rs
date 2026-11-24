//! Telling MingDesk when the desk changes (PLAN.md §6, M1), on one thread with its own message loop:
//! - WinEvent hooks (out of context, so nothing is injected into other apps): windows created, destroyed, shown,
//!   hidden, retitled, moved or resized, minimized and restored, the foreground changing, and windows cloaked or
//!   uncloaked, which is what switching virtual desktops looks like from outside.
//! - A hidden window of MingDesk's own for WM_DISPLAYCHANGE (monitors added, removed, rearranged, resolution) and
//!   WM_SETTINGCHANGE (the taskbar moving changes work areas). Both are broadcast only to top-level windows, so it
//!   is an ordinary top-level window that is never shown, not a message-only one.
//!
//! The sink gets every event; the refresh thread in lib.rs lets a burst settle before taking one snapshot
//! (live.rs SETTLE), as on the Mac. Anything missed is caught by its backstop timer while the board shows.

use super::list::class_of;
use crate::platform::{Change, Sink};
use ::windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use ::windows::Win32::System::LibraryLoader::GetModuleHandleW;
use ::windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook};
use ::windows::Win32::UI::WindowsAndMessaging::{
    CHILDID_SELF, CreateWindowExW, DefWindowProcW, DispatchMessageW, EVENT_OBJECT_CLOAKED,
    EVENT_OBJECT_CREATE, EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE, EVENT_OBJECT_LOCATIONCHANGE,
    EVENT_OBJECT_NAMECHANGE, EVENT_OBJECT_UNCLOAKED, EVENT_SYSTEM_FOREGROUND,
    EVENT_SYSTEM_MINIMIZEEND, EVENT_SYSTEM_MINIMIZESTART, GA_ROOT, GetAncestor, GetMessageW, MSG,
    OBJID_WINDOW, RegisterClassW, TranslateMessage, WINDOW_EX_STYLE, WINEVENT_OUTOFCONTEXT,
    WINEVENT_SKIPOWNPROCESS, WM_DISPLAYCHANGE, WM_SETTINGCHANGE, WNDCLASSW, WS_OVERLAPPED,
};
use ::windows::core::w;
use std::sync::OnceLock;

/// The one sink, for the C callbacks (which carry no Rust context).
static SINK: OnceLock<Sink> = OnceLock::new();

fn report(change: Change) {
    if let Some(sink) = SINK.get() {
        sink(change);
    }
}

pub fn start(sink: Sink) {
    if SINK.set(sink).is_err() {
        return; // already watching
    }
    let _ = std::thread::Builder::new()
        .name("mingdesk-win-events".into())
        .spawn(run);
}

/// Which WinEvents are about a top-level window, and so worth a refresh. Location changes also fire for carets,
/// cursors and scrollbars, and create/show/hide for every child control: only the window object of a top-level
/// window counts.
fn about_a_window(event: u32, hwnd: HWND, id_object: i32, id_child: i32) -> bool {
    if event == EVENT_SYSTEM_FOREGROUND
        || event == EVENT_SYSTEM_MINIMIZESTART
        || event == EVENT_SYSTEM_MINIMIZEEND
    {
        return true;
    }
    if id_object != OBJID_WINDOW.0 || id_child != CHILDID_SELF as i32 || hwnd.is_invalid() {
        return false;
    }
    if event == EVENT_OBJECT_DESTROY {
        // Gone already, so GetAncestor can't say; a destroyed child is rare enough to refresh for.
        return true;
    }
    // SAFETY: GetAncestor accepts any handle.
    let root = unsafe { GetAncestor(hwnd, GA_ROOT) };
    root == hwnd && class_of(hwnd) != "MingDeskWatch"
}

unsafe extern "system" fn win_event(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    id_object: i32,
    id_child: i32,
    _thread: u32,
    _time: u32,
) {
    if about_a_window(event, hwnd, id_object, id_child) {
        report(Change::Windows);
    }
}

unsafe extern "system" fn watch_proc(h: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_DISPLAYCHANGE || msg == WM_SETTINGCHANGE {
        crate::log::event(&format!("display or setting change (message {msg:#x})"));
        report(Change::Displays);
    }
    // SAFETY: the default handling for everything else.
    unsafe { DefWindowProcW(h, msg, w, l) }
}

fn run() {
    // SAFETY: the hooks and the window belong to this thread, which keeps pumping messages for as long as
    // MingDesk runs; out-of-context hooks are delivered through this loop.
    unsafe {
        let flags = WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS;
        let ranges = [
            (EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND),
            (EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZEEND),
            (EVENT_OBJECT_CREATE, EVENT_OBJECT_HIDE),
            (EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_NAMECHANGE),
            (EVENT_OBJECT_CLOAKED, EVENT_OBJECT_UNCLOAKED),
        ];
        for (min, max) in ranges {
            if SetWinEventHook(min, max, None, Some(win_event), 0, 0, flags).is_invalid() {
                crate::log::event(&format!("couldn't hook window events {min:#x}..{max:#x}"));
            }
        }

        let instance = GetModuleHandleW(None).unwrap_or_default();
        let class = WNDCLASSW {
            lpfnWndProc: Some(watch_proc),
            hInstance: instance.into(),
            lpszClassName: w!("MingDeskWatch"),
            ..Default::default()
        };
        RegisterClassW(&class);
        if CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("MingDeskWatch"),
            w!("MingDesk display watcher"),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance.into()),
            None,
        )
        .is_err()
        {
            crate::log::event("couldn't create the display watcher window");
        }

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

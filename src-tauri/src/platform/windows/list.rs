//! Top-level windows and the apps they belong to.
//!
//! EnumWindows walks top-level windows front to back, which is the stacking order. Each one is read without
//! sending it a message (GetWindowText reads another process's title from the window manager), so a hung app
//! can't stall a snapshot.

use super::desktops::Vdm;
use super::rules::{self, Candidate};
use crate::platform::Rect;
use ::windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, MAX_PATH, RECT};
use ::windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
use ::windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
use ::windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use ::windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, EnumWindows, GW_OWNER, GWL_EXSTYLE, GWL_STYLE, GetClassNameW, GetWindow,
    GetWindowLongPtrW, GetWindowPlacement, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, IsIconic, IsWindowVisible, WINDOWPLACEMENT, WS_CAPTION, WS_EX_APPWINDOW,
    WS_EX_TOOLWINDOW,
};
use ::windows::core::{BOOL, PCWSTR, PWSTR};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

/// A window's handle as a plain number: HWNDs fit in 32 bits on 64-bit Windows (by design, so 32-bit processes
/// can share them), and the board's window ids are u32.
pub fn id_of(h: HWND) -> u32 {
    h.0 as usize as u32
}

pub fn hwnd(id: u32) -> HWND {
    HWND(id as usize as *mut _)
}

/// One listed window, as read from Windows.
#[derive(Debug, Clone)]
pub struct Raw {
    pub id: u32,
    /// The app's process: for a Store app, the app inside ApplicationFrameHost.
    pub pid: u32,
    pub class: String,
    pub title: String,
    /// The visible frame; for a minimized window, still in workspace coordinates (see `rules::workspace_to_screen`).
    pub frame: Rect,
    pub minimized: bool,
    pub has_caption: bool,
    pub on_current_desktop: bool,
    pub desktop: Option<u128>,
    /// Position in EnumWindows, 0 = front-most.
    pub z: u32,
}

unsafe extern "system" fn collect(h: HWND, data: LPARAM) -> BOOL {
    // SAFETY: `data` is the Vec passed by the caller, alive for the whole enumeration.
    let all = unsafe { &mut *(data.0 as *mut Vec<HWND>) };
    all.push(h);
    true.into()
}

/// Every top-level window, front to back.
pub fn top_level() -> Vec<HWND> {
    let mut all: Vec<HWND> = Vec::new();
    // SAFETY: the callback only pushes into `all`, which outlives the call.
    let _ = unsafe { EnumWindows(Some(collect), LPARAM(&mut all as *mut _ as isize)) };
    all
}

pub fn class_of(h: HWND) -> String {
    let mut buf = [0u16; 256];
    // SAFETY: the buffer's length is passed with it.
    let n = unsafe { GetClassNameW(h, &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

pub fn title_of(h: HWND) -> String {
    // SAFETY: plain reads; the buffer's length is passed with it.
    unsafe {
        let len = GetWindowTextLengthW(h);
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; len as usize + 1];
        let n = GetWindowTextW(h, &mut buf);
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    }
}

pub fn pid_of(h: HWND) -> u32 {
    let mut pid = 0u32;
    // SAFETY: writes the process id into `pid`.
    unsafe { GetWindowThreadProcessId(h, Some(&mut pid)) };
    pid
}

fn cloaked(h: HWND) -> bool {
    let mut value = 0u32;
    // SAFETY: DWMWA_CLOAKED fills a DWORD.
    unsafe {
        DwmGetWindowAttribute(h, DWMWA_CLOAKED, &mut value as *mut _ as *mut _, size_of::<u32>() as u32)
            .is_ok()
            && value != 0
    }
}

/// The frame people see: DWM's extended frame bounds leave out the invisible resize borders that GetWindowRect
/// includes (about 7 px on each side but the top on Windows 10 and 11).
pub fn frame_of(h: HWND) -> Option<Rect> {
    let mut r = RECT::default();
    // SAFETY: DWMWA_EXTENDED_FRAME_BOUNDS fills a RECT.
    let dwm = unsafe {
        DwmGetWindowAttribute(
            h,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut r as *mut _ as *mut _,
            size_of::<RECT>() as u32,
        )
    };
    // SAFETY: fills a RECT.
    if dwm.is_err() && unsafe { GetWindowRect(h, &mut r) }.is_err() {
        return None;
    }
    Some(rules::rect(r.left, r.top, r.right, r.bottom))
}

/// Where a minimized window goes back to, in workspace coordinates.
fn restored_frame(h: HWND) -> Option<Rect> {
    let mut p = WINDOWPLACEMENT {
        length: size_of::<WINDOWPLACEMENT>() as u32,
        ..Default::default()
    };
    // SAFETY: `length` is set as the call requires.
    unsafe { GetWindowPlacement(h, &mut p) }.ok()?;
    let r = p.rcNormalPosition;
    Some(rules::rect(r.left, r.top, r.right, r.bottom))
}

struct CoreWindow {
    frame_pid: u32,
    found: Option<u32>,
}

unsafe extern "system" fn find_core_window(h: HWND, data: LPARAM) -> BOOL {
    // SAFETY: `data` is the CoreWindow passed by `store_app_pid`, alive for the whole enumeration.
    let search = unsafe { &mut *(data.0 as *mut CoreWindow) };
    if class_of(h) == "Windows.UI.Core.CoreWindow" {
        let pid = pid_of(h);
        if pid != search.frame_pid {
            search.found = Some(pid);
            return false.into();
        }
    }
    true.into()
}

/// A Store (UWP) app's window belongs to ApplicationFrameHost.exe, which draws the frame; the app itself runs
/// in the CoreWindow inside it. While the app is suspended the CoreWindow may be detached; then this says None
/// and the window is reported under ApplicationFrameHost, as Windows' own Task Manager does.
fn store_app_pid(h: HWND, frame_pid: u32) -> Option<u32> {
    let mut search = CoreWindow {
        frame_pid,
        found: None,
    };
    // SAFETY: the callback only writes into `search`, which outlives the call.
    let _ = unsafe { EnumChildWindows(Some(h), Some(find_core_window), LPARAM(&mut search as *mut _ as isize)) };
    search.found
}

/// Reads one top-level window and decides whether it's listed (`rules::counts`).
pub fn describe(h: HWND, z: u32, vdm: Option<&Vdm>) -> Option<Raw> {
    // SAFETY: plain reads of the window's styles and state.
    let (visible, style, ex_style, owned, minimized) = unsafe {
        (
            IsWindowVisible(h).as_bool(),
            GetWindowLongPtrW(h, GWL_STYLE) as u32,
            GetWindowLongPtrW(h, GWL_EXSTYLE) as u32,
            GetWindow(h, GW_OWNER).is_ok_and(|o| !o.is_invalid()),
            IsIconic(h).as_bool(),
        )
    };
    if !visible || owned || ex_style & WS_EX_TOOLWINDOW.0 != 0 {
        return None; // the cheap checks first: most top-level windows end here
    }
    let class = class_of(h);
    let title = title_of(h);
    let is_cloaked = cloaked(h);
    let on_current = vdm.and_then(|v| v.on_current(h)).unwrap_or(true);
    let desktop = vdm.and_then(|v| v.desktop(h));
    let candidate = Candidate {
        class: &class,
        title: &title,
        visible,
        owned,
        tool_window: false,
        app_window: ex_style & WS_EX_APPWINDOW.0 != 0,
        cloaked: is_cloaked,
        on_other_desktop: !on_current && desktop.is_some(),
    };
    if !rules::counts(&candidate) {
        return None;
    }
    let frame = if minimized { restored_frame(h)? } else { frame_of(h)? };
    let frame_pid = pid_of(h);
    let pid = if class == "ApplicationFrameWindow" {
        store_app_pid(h, frame_pid).unwrap_or(frame_pid)
    } else {
        frame_pid
    };
    Some(Raw {
        id: id_of(h),
        pid,
        class,
        title,
        frame,
        minimized,
        has_caption: style & WS_CAPTION.0 == WS_CAPTION.0,
        on_current_desktop: on_current,
        desktop,
        z,
    })
}

/// The executable a process runs, or None when Windows won't say (protected and some elevated processes).
pub fn exe_path(pid: u32) -> Option<String> {
    // SAFETY: the handle is closed before returning; the buffer's length is passed with it.
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; MAX_PATH as usize * 4];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(process);
        ok.ok()?;
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// An executable's FileDescription ("Notepad", "Google Chrome", "Windows Explorer"), as Task Manager shows it.
fn file_description(path: &str) -> Option<String> {
    let wpath = to_wide(path);
    // SAFETY: the version block is read into a buffer of the size Windows asked for, and every pointer
    // VerQueryValueW returns points inside it.
    unsafe {
        let size = GetFileVersionInfoSizeW(PCWSTR(wpath.as_ptr()), None);
        if size == 0 {
            return None;
        }
        let mut block = vec![0u8; size as usize];
        GetFileVersionInfoW(PCWSTR(wpath.as_ptr()), None, size, block.as_mut_ptr() as *mut _).ok()?;
        let mut langs: Vec<String> = Vec::new();
        let (mut ptr, mut len) = (std::ptr::null_mut(), 0u32);
        let key = to_wide(r"\VarFileInfo\Translation");
        if VerQueryValueW(block.as_ptr() as *const _, PCWSTR(key.as_ptr()), &mut ptr, &mut len).as_bool() {
            let pairs = std::slice::from_raw_parts(ptr as *const u16, len as usize / 2);
            for pair in pairs.chunks_exact(2) {
                langs.push(format!("{:04x}{:04x}", pair[0], pair[1]));
            }
        }
        langs.extend(["040904b0".into(), "040904e4".into(), "04090000".into()]);
        for lang in langs {
            let key = to_wide(&format!(r"\StringFileInfo\{lang}\FileDescription"));
            let (mut ptr, mut len) = (std::ptr::null_mut(), 0u32);
            if VerQueryValueW(block.as_ptr() as *const _, PCWSTR(key.as_ptr()), &mut ptr, &mut len).as_bool()
                && len > 1
            {
                let text = std::slice::from_raw_parts(ptr as *const u16, len as usize);
                let end = text.iter().position(|c| *c == 0).unwrap_or(text.len());
                let s = String::from_utf16_lossy(&text[..end]).trim().to_string();
                if !s.is_empty() {
                    return Some(s);
                }
            }
        }
        None
    }
}

/// An app's name: its FileDescription, else the exe's name without ".exe". Cached per executable.
pub fn app_name(path: &str) -> String {
    static NAMES: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    let names = NAMES.get_or_init(Default::default);
    if let Some(n) = names.lock().unwrap().get(path) {
        return n.clone();
    }
    let name = file_description(path).unwrap_or_else(|| {
        Path::new(path)
            .file_stem()
            .map_or_else(|| path.to_string(), |s| s.to_string_lossy().into_owned())
    });
    names.lock().unwrap().insert(path.to_string(), name.clone());
    name
}

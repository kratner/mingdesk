//! Focus, minimize, close and bring forward.
//!
//! Focus is the hard one: Windows lets a process set the foreground window only if it's in the foreground
//! already or received the last input event (the "foreground lock", SetForegroundWindow's documented rules). A
//! click on the board qualifies; a click on the tray menu usually does; a switcher popping up from a hotkey may
//! not. So `focus` tries the plain call, then the documented ways around it: send an input event of its own (a
//! zero mouse move, which satisfies "received the last input event" without pressing anything), then attach to
//! the foreground thread's input queue for the call (AttachThreadInput).

use super::list::hwnd;
use ::windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use ::windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use ::windows::Win32::UI::Input::KeyboardAndMouse::{INPUT, INPUT_MOUSE, SendInput};
use ::windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, HWND_TOP, IsIconic, IsWindow,
    PostMessageW, SW_MINIMIZE, SW_RESTORE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SetForegroundWindow, SetWindowPos, ShowWindowAsync, WM_CLOSE,
};
use std::time::{Duration, Instant};

const GONE: &str = "That window has closed.";

fn alive(id: u32) -> Result<HWND, String> {
    let h = hwnd(id);
    // SAFETY: IsWindow accepts any value and says whether it's a window.
    if unsafe { IsWindow(Some(h)) }.as_bool() {
        Ok(h)
    } else {
        Err(GONE.into())
    }
}

fn in_front(h: HWND) -> bool {
    // SAFETY: a plain query.
    unsafe { GetForegroundWindow() == h }
}

/// Waits briefly for Windows to report `h` in front (activation crosses processes, so it isn't instant).
fn came_forward(h: HWND) -> bool {
    let until = Instant::now() + Duration::from_millis(250);
    loop {
        if in_front(h) {
            return true;
        }
        if Instant::now() >= until {
            return false;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
}

fn try_set_foreground(h: HWND) -> bool {
    // SAFETY: plain calls on a window handle checked alive by the caller.
    unsafe { SetForegroundWindow(h).as_bool() && came_forward(h) }
}

/// Counts as the last input event, so the foreground lock lets MingDesk hand over focus. Moves nothing.
fn nudge_input() {
    let input = INPUT {
        r#type: INPUT_MOUSE,
        ..Default::default()
    };
    // SAFETY: one zeroed mouse input: no movement, no buttons.
    unsafe { SendInput(&[input], size_of::<INPUT>() as i32) };
}

/// Joins the foreground window's input queue for the call, as Windows' own task switchers effectively do.
fn set_foreground_attached(h: HWND) -> bool {
    // SAFETY: the threads are detached again before returning, whatever happens in between.
    unsafe {
        let fg = GetForegroundWindow();
        let fg_thread = GetWindowThreadProcessId(fg, None);
        let me = GetCurrentThreadId();
        let attached =
            fg_thread != 0 && fg_thread != me && AttachThreadInput(me, fg_thread, true).as_bool();
        let _ = BringWindowToTop(h);
        let ok = SetForegroundWindow(h).as_bool();
        if attached {
            let _ = AttachThreadInput(me, fg_thread, false);
        }
        ok && came_forward(h)
    }
}

pub fn focus(id: u32) -> Result<(), String> {
    let h = alive(id)?;
    // SAFETY: plain calls on a live window; the Async form never waits on a hung app.
    unsafe {
        if IsIconic(h).as_bool() {
            let _ = ShowWindowAsync(h, SW_RESTORE);
        }
    }
    if in_front(h) || try_set_foreground(h) {
        return Ok(());
    }
    nudge_input();
    if try_set_foreground(h) || set_foreground_attached(h) {
        return Ok(());
    }
    Err("Windows didn't let MingDesk bring that window to the front.".into())
}

pub fn minimize(id: u32) -> Result<(), String> {
    let h = alive(id)?;
    // SAFETY: asynchronous, so a hung app can't hold MingDesk up.
    unsafe {
        let _ = ShowWindowAsync(h, SW_MINIMIZE);
    }
    Ok(())
}

pub fn close(id: u32) -> Result<(), String> {
    let h = alive(id)?;
    // SAFETY: posting returns at once; the app handles WM_CLOSE as its close button would (and may ask to save).
    unsafe { PostMessageW(Some(h), WM_CLOSE, WPARAM(0), LPARAM(0)) }.map_err(|e| e.to_string())
}

/// Raises every window in `back_to_front` (an app's windows on the desktop showing now, back-most first), so
/// they keep their order among themselves, then focuses the front-most.
pub fn bring_forward(back_to_front: &[u32]) -> Result<(), String> {
    let Some(front) = back_to_front.last() else {
        return Err("That app has no windows on this desktop.".into());
    };
    for id in back_to_front {
        let h = hwnd(*id);
        // SAFETY: moves only the stacking order; never activates, never waits on the app.
        unsafe {
            let _ = SetWindowPos(
                h,
                Some(HWND_TOP),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }
    focus(*front)
}

//! Telling MingDesk when the desk changes (PLAN.md §6, M1), from three sources:
//! - Accessibility observers, one per regular app, on a thread of their own with its own run loop: windows
//!   opening, closing, moving, resizing, retitling, minimizing, and focus moving.
//! - NSWorkspace notifications on the main thread: apps launching, quitting, hiding, coming forward, and the
//!   desktop (Mission Control space) changing.
//! - The CoreGraphics display-reconfiguration callback, also on the main thread: displays added, removed,
//!   rearranged.
//!
//! Anything missed is caught by the refresh thread's backstop timer while the board shows (live.rs).

use super::ax::Element;
use super::ffi::*;
use crate::platform::{Change, Sink};
use block2::RcBlock;
use core_foundation::base::{CFType, TCFType};
use core_foundation::runloop::{
    CFRunLoop, CFRunLoopRunResult, CFRunLoopSource, kCFRunLoopDefaultMode,
};
use core_foundation::string::{CFString, CFStringRef};
use objc2::msg_send;
use objc2::rc::autoreleasepool;
use objc2::runtime::{AnyClass, AnyObject};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

/// The one sink, for the C callbacks (which carry no Rust context).
static SINK: OnceLock<Sink> = OnceLock::new();
/// Asks the observer thread to look at running apps and windows again (launch, quit, desktop switch).
static RESYNC: OnceLock<std::sync::Mutex<Sender<()>>> = OnceLock::new();

fn report(change: Change) {
    if let Some(sink) = SINK.get() {
        sink(change);
    }
}

fn resync() {
    if let Some(tx) = RESYNC.get() {
        let _ = tx.lock().unwrap().send(());
    }
}

/// Run-loop turns (half a second each) between syncs nobody asked for.
const RESYNC_EVERY: u32 = 20;

/// Registered on each app: new windows, focus, the app hiding or coming forward.
const APP_NOTIFICATIONS: &[&str] = &[
    "AXWindowCreated",
    "AXFocusedWindowChanged",
    "AXMainWindowChanged",
    "AXApplicationHidden",
    "AXApplicationShown",
    "AXApplicationActivated",
];
/// Registered on each window: everything that changes what the board shows about it.
const WINDOW_NOTIFICATIONS: &[&str] = &[
    "AXUIElementDestroyed",
    "AXTitleChanged",
    "AXWindowMoved",
    "AXWindowResized",
    "AXWindowMiniaturized",
    "AXWindowDeminiaturized",
];

/// Must be called on the main thread (the display callback and workspace notifications arrive there).
pub fn start(sink: Sink) {
    if SINK.set(sink).is_err() {
        return; // already watching
    }
    let (tx, rx) = mpsc::channel();
    let _ = RESYNC.set(std::sync::Mutex::new(tx));
    // SAFETY: the callback is a plain function that lives for the whole program.
    unsafe { CGDisplayRegisterReconfigurationCallback(display_changed, std::ptr::null_mut()) };
    watch_workspace();
    let _ = std::thread::Builder::new()
        .name("mingdesk-ax-observers".into())
        .spawn(move || observe(rx));
}

extern "C" fn display_changed(
    _display: CGDirectDisplayID,
    flags: u32,
    _user: *mut std::ffi::c_void,
) {
    // Each change comes as a "begin" call and then the change itself; the begin call has nothing new yet.
    if flags & kCGDisplayBeginConfigurationFlag == 0 {
        report(Change::Displays);
    }
}

fn watch_workspace() {
    let Some(class) = AnyClass::get(c"NSWorkspace") else {
        return;
    };
    autoreleasepool(|_| unsafe {
        // SAFETY: documented NSWorkspace/NSNotificationCenter API; the center keeps the observers (and blocks)
        // for as long as MingDesk runs, which is what we want.
        let workspace: *mut AnyObject = msg_send![class, sharedWorkspace];
        let center: *mut AnyObject = msg_send![workspace, notificationCenter];
        for (name, apps_changed) in [
            ("NSWorkspaceActiveSpaceDidChangeNotification", true),
            ("NSWorkspaceDidLaunchApplicationNotification", true),
            ("NSWorkspaceDidTerminateApplicationNotification", true),
            ("NSWorkspaceDidHideApplicationNotification", false),
            ("NSWorkspaceDidUnhideApplicationNotification", false),
            ("NSWorkspaceDidActivateApplicationNotification", false),
        ] {
            let block = RcBlock::new(move |_note: *mut AnyObject| {
                if apps_changed {
                    resync();
                }
                report(Change::Windows);
            });
            let name = CFString::new(name);
            let _: *mut AnyObject = msg_send![
                center,
                addObserverForName: name.as_concrete_TypeRef() as *mut AnyObject,
                object: std::ptr::null_mut::<AnyObject>(),
                queue: std::ptr::null_mut::<AnyObject>(),
                usingBlock: &*block
            ];
        }
    });
}

extern "C" fn ax_changed(
    observer: AXObserverRef,
    element: AXUIElementRef,
    notification: CFStringRef,
    _refcon: *mut std::ffi::c_void,
) {
    // SAFETY: the notification name is a CFString owned by the caller for the call's duration.
    let name = unsafe { CFString::wrap_under_get_rule(notification) }.to_string();
    if name == "AXWindowCreated" {
        // Watch the new window itself too, so its moves and closing are heard.
        add_notifications(observer, element, WINDOW_NOTIFICATIONS);
    }
    report(Change::Windows);
}

fn add_notifications(observer: AXObserverRef, element: AXUIElementRef, names: &[&str]) {
    for name in names {
        let name = CFString::new(name);
        // SAFETY: both refs are alive for the call; errors (e.g. "already registered") are harmless.
        unsafe {
            AXObserverAddNotification(
                observer,
                element,
                name.as_concrete_TypeRef(),
                std::ptr::null_mut(),
            )
        };
    }
}

struct Watched {
    observer: CFType,
    source: CFRunLoopSource,
    app: Element,
}

/// Regular apps (with a Dock icon) now running, by process id.
fn regular_pids() -> HashSet<i32> {
    let mut pids = HashSet::new();
    let Some(class) = AnyClass::get(c"NSWorkspace") else {
        return pids;
    };
    autoreleasepool(|_| unsafe {
        // SAFETY: documented NSWorkspace API.
        let workspace: *mut AnyObject = msg_send![class, sharedWorkspace];
        let apps: *mut AnyObject = msg_send![workspace, runningApplications];
        let count: usize = msg_send![apps, count];
        for i in 0..count {
            let app: *mut AnyObject = msg_send![apps, objectAtIndex: i];
            let policy: isize = msg_send![app, activationPolicy];
            if policy == 0 {
                pids.insert(msg_send![app, processIdentifier]);
            }
        }
    });
    pids.remove(&(std::process::id() as i32));
    pids
}

fn observe(resync: Receiver<()>) {
    let run_loop = CFRunLoop::get_current();
    // SAFETY: a constant exported by CoreFoundation.
    let mode = unsafe { kCFRunLoopDefaultMode };
    let mut watched: HashMap<i32, Watched> = HashMap::new();
    let mut sync = true;
    let mut ticks = 0u32;
    loop {
        if sync {
            let running = regular_pids();
            watched.retain(|pid, w| {
                let keep = running.contains(pid);
                if !keep {
                    run_loop.remove_source(&w.source, mode);
                }
                keep
            });
            for pid in running {
                if let Some(w) = watched.get(&pid) {
                    // Windows on a desktop just switched to weren't in the app's list before: watch them now.
                    for window in w.app.windows() {
                        add_notifications(
                            w.observer.as_CFTypeRef(),
                            window.as_ptr(),
                            WINDOW_NOTIFICATIONS,
                        );
                    }
                } else if let Some(w) = watch_app(pid) {
                    run_loop.add_source(&w.source, mode);
                    watched.insert(pid, w);
                }
            }
        }
        if let CFRunLoopRunResult::Finished =
            CFRunLoop::run_in_mode(mode, Duration::from_millis(500), false)
        {
            // No sources yet (no apps watched): don't spin.
            std::thread::sleep(Duration::from_millis(500));
        }
        // A launch, quit or desktop switch asks for a sync; every RESYNC_EVERY one happens anyway, in case a
        // notification was missed.
        ticks += 1;
        sync = resync.try_iter().count() > 0 || ticks % RESYNC_EVERY == 0;
    }
}

fn watch_app(pid: i32) -> Option<Watched> {
    let mut observer: AXObserverRef = std::ptr::null();
    // SAFETY: on success `observer` is owned by us (create rule).
    if unsafe { AXObserverCreate(pid, ax_changed, &mut observer) } != kAXErrorSuccess
        || observer.is_null()
    {
        return None;
    }
    let observer = unsafe { CFType::wrap_under_create_rule(observer) };
    // SAFETY: the source belongs to the observer (get rule); the wrapper retains it.
    let source = unsafe {
        CFRunLoopSource::wrap_under_get_rule(
            AXObserverGetRunLoopSource(observer.as_CFTypeRef()) as _
        )
    };
    let app = Element::app(pid);
    add_notifications(observer.as_CFTypeRef(), app.as_ptr(), APP_NOTIFICATIONS);
    for window in app.windows() {
        add_notifications(
            observer.as_CFTypeRef(),
            window.as_ptr(),
            WINDOW_NOTIFICATIONS,
        );
    }
    Some(Watched {
        observer,
        source,
        app,
    })
}

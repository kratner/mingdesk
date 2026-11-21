//! The C functions MingDesk calls on the Mac, declared by hand as M0 did. Private ones (leading underscore or
//! CGS) are marked; PLAN.md §7.12 says why they're used and what happens if one goes away.

#![allow(non_snake_case, non_upper_case_globals)]

use core_foundation::array::CFArrayRef;
use core_foundation::data::CFDataRef;
use core_foundation::string::CFStringRef;
use std::ffi::c_void;

pub type CFTypeRef = *const c_void;
pub type AXUIElementRef = *const c_void;
pub type AXObserverRef = *const c_void;
pub type AXObserverCallback = extern "C" fn(
    observer: AXObserverRef,
    element: AXUIElementRef,
    notification: CFStringRef,
    refcon: *mut c_void,
);
pub type CGDisplayReconfigurationCallBack =
    extern "C" fn(display: CGDirectDisplayID, flags: u32, user: *mut c_void);
pub type AXError = i32;
pub type CGWindowID = u32;
pub type CGDirectDisplayID = u32;
pub type CGSConnectionID = i32;

pub const kAXErrorSuccess: AXError = 0;
/// First call of a display change, before anything has moved.
pub const kCGDisplayBeginConfigurationFlag: u32 = 1 << 0;
pub const kAXValueCGPointType: u32 = 1;
pub const kAXValueCGSizeType: u32 = 2;

/// Every window, on screen or not, except the desktop picture and icons.
pub const kCGWindowListOptionAll: u32 = 0;
pub const kCGWindowListExcludeDesktopElements: u32 = 1 << 4;
pub const kCGNullWindowID: CGWindowID = 0;

/// NSApplicationActivationOptions.
pub const NSApplicationActivateAllWindows: usize = 1 << 0;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CGPoint {
    pub x: f64,
    pub y: f64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CGSize {
    pub width: f64,
    pub height: f64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CGRect {
    pub origin: CGPoint,
    pub size: CGSize,
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    pub fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    pub fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    pub fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> AXError;
    pub fn AXUIElementPerformAction(element: AXUIElementRef, action: CFStringRef) -> AXError;
    pub fn AXUIElementSetMessagingTimeout(element: AXUIElementRef, seconds: f32) -> AXError;
    pub fn AXValueGetValue(value: CFTypeRef, kind: u32, out: *mut c_void) -> bool;
    pub fn AXObserverCreate(
        pid: i32,
        callback: AXObserverCallback,
        observer: *mut AXObserverRef,
    ) -> AXError;
    pub fn AXObserverAddNotification(
        observer: AXObserverRef,
        element: AXUIElementRef,
        notification: CFStringRef,
        refcon: *mut c_void,
    ) -> AXError;
    pub fn AXObserverGetRunLoopSource(observer: AXObserverRef) -> CFTypeRef;

    /// Private: the CGWindowID behind an Accessibility window.
    pub fn _AXUIElementGetWindow(element: AXUIElementRef, out: *mut CGWindowID) -> AXError;
    /// Private: an Accessibility element from a 20-byte token (pid, 0, "coco", element id). Reaches windows on
    /// other desktops, which an app's AXWindows leaves out.
    pub fn _AXUIElementCreateWithRemoteToken(token: CFDataRef) -> AXUIElementRef;
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    pub fn CGWindowListCopyWindowInfo(option: u32, relative_to: CGWindowID) -> CFArrayRef;
    pub fn CGGetActiveDisplayList(
        max: u32,
        displays: *mut CGDirectDisplayID,
        count: *mut u32,
    ) -> i32;
    pub fn CGDisplayBounds(display: CGDirectDisplayID) -> CGRect;
    pub fn CGDisplayIsMain(display: CGDirectDisplayID) -> u32;
    /// The display this one mirrors, or 0 if it mirrors none.
    pub fn CGDisplayMirrorsDisplay(display: CGDirectDisplayID) -> CGDirectDisplayID;
    pub fn CGDisplayRegisterReconfigurationCallback(
        callback: CGDisplayReconfigurationCallBack,
        user: *mut c_void,
    ) -> i32;

    /// Private (SkyLight, re-exported by CoreGraphics): this process's window-server connection.
    pub fn CGSMainConnectionID() -> CGSConnectionID;
    /// Private: each display's desktops, full-screen spaces and current space.
    pub fn CGSCopyManagedDisplaySpaces(connection: CGSConnectionID) -> CFArrayRef;
    /// Private: the spaces the given windows are on. Mask 7 = every kind of space.
    pub fn CGSCopySpacesForWindows(
        connection: CGSConnectionID,
        mask: i32,
        windows: CFArrayRef,
    ) -> CFArrayRef;
}

#[link(name = "ColorSync", kind = "framework")]
unsafe extern "C" {
    /// The display's UUID, which outlives its CGDirectDisplayID across unplugging and sleep. Create rule.
    pub fn CGDisplayCreateUUIDFromDisplayID(display: CGDirectDisplayID) -> CFTypeRef;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    pub fn CFUUIDCreateString(allocator: CFTypeRef, uuid: CFTypeRef) -> CFStringRef;
}

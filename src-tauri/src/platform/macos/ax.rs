//! Accessibility: window titles, minimized and full-screen state, and the actions (raise, minimize, close).
//! Needs the Accessibility permission (M0's onboarding).

use super::ffi::*;
use crate::platform::Rect;
use core_foundation::array::CFArray;
use core_foundation::base::{CFType, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::data::CFData;
use core_foundation::string::CFString;
use std::collections::HashSet;
use std::ffi::c_void;
use std::time::{Duration, Instant};

/// How long MingDesk waits for an app that doesn't answer before moving on, in seconds.
const APP_TIMEOUT: f32 = 0.25;

/// An Accessibility element MingDesk owns (released when dropped).
#[derive(Clone)]
pub struct Element(CFType);

impl Element {
    /// SAFETY: `ptr` must be a non-null AXUIElementRef we own (create or copy rule).
    unsafe fn own(ptr: AXUIElementRef) -> Self {
        Self(unsafe { CFType::wrap_under_create_rule(ptr) })
    }

    fn ptr(&self) -> AXUIElementRef {
        self.0.as_CFTypeRef()
    }

    pub fn app(pid: i32) -> Self {
        // SAFETY: create rule, never null for a pid.
        let app = unsafe { Self::own(AXUIElementCreateApplication(pid)) };
        unsafe { AXUIElementSetMessagingTimeout(app.ptr(), APP_TIMEOUT) };
        app
    }

    fn attr(&self, name: &str) -> Option<CFType> {
        let mut value: CFTypeRef = std::ptr::null();
        let key = CFString::new(name);
        // SAFETY: on success `value` is a retained CF object (copy rule) that we take over.
        let err = unsafe {
            AXUIElementCopyAttributeValue(self.ptr(), key.as_concrete_TypeRef(), &mut value)
        };
        (err == kAXErrorSuccess && !value.is_null())
            .then(|| unsafe { CFType::wrap_under_create_rule(value) })
    }

    pub fn string(&self, name: &str) -> Option<String> {
        self.attr(name)
            .and_then(|v| v.downcast::<CFString>())
            .map(|s| s.to_string())
    }

    pub fn boolean(&self, name: &str) -> Option<bool> {
        self.attr(name)
            .and_then(|v| v.downcast::<CFBoolean>())
            .map(bool::from)
    }

    pub fn element(&self, name: &str) -> Option<Element> {
        // An element attribute is an AXUIElementRef already wrapped (and owned) as a CFType.
        self.attr(name).map(Element)
    }

    fn value<T: Default>(&self, name: &str, kind: u32) -> Option<T> {
        let v = self.attr(name)?;
        let mut out = T::default();
        // SAFETY: `kind` matches T (CGPoint or CGSize); AXValueGetValue writes one T.
        unsafe { AXValueGetValue(v.as_CFTypeRef(), kind, &mut out as *mut T as *mut c_void) }
            .then_some(out)
    }

    pub fn frame(&self) -> Option<Rect> {
        let p: CGPoint = self.value("AXPosition", kAXValueCGPointType)?;
        let s: CGSize = self.value("AXSize", kAXValueCGSizeType)?;
        Some(Rect::new(p.x, p.y, s.width, s.height))
    }

    /// The CoreGraphics window number behind this window (private call, PLAN.md §7.12).
    pub fn window_id(&self) -> Option<u32> {
        let mut id = 0u32;
        // SAFETY: writes one u32.
        (unsafe { _AXUIElementGetWindow(self.ptr(), &mut id) } == kAXErrorSuccess && id != 0)
            .then_some(id)
    }

    pub fn windows(&self) -> Vec<Element> {
        let Some(list) = self.attr("AXWindows") else {
            return Vec::new();
        };
        if list.type_of() != CFArray::<CFType>::type_id() {
            return Vec::new();
        }
        // SAFETY: checked it's a CFArray; each item is an AXUIElementRef.
        let list: CFArray<CFType> =
            unsafe { CFArray::wrap_under_get_rule(list.as_CFTypeRef() as _) };
        list.iter().map(|w| Element(w.clone())).collect()
    }

    pub fn set_bool(&self, name: &str, value: bool) -> Result<(), String> {
        let key = CFString::new(name);
        let v = if value {
            CFBoolean::true_value()
        } else {
            CFBoolean::false_value()
        };
        // SAFETY: plain call; both CF objects live across it.
        match unsafe {
            AXUIElementSetAttributeValue(self.ptr(), key.as_concrete_TypeRef(), v.as_CFTypeRef())
        } {
            err if err == kAXErrorSuccess => Ok(()),
            err => Err(format!(
                "the app refused {name} (Accessibility error {err})"
            )),
        }
    }

    pub fn perform(&self, action: &str) -> Result<(), String> {
        let key = CFString::new(action);
        // SAFETY: plain call.
        match unsafe { AXUIElementPerformAction(self.ptr(), key.as_concrete_TypeRef()) } {
            err if err == kAXErrorSuccess => Ok(()),
            err => Err(format!(
                "the app refused {action} (Accessibility error {err})"
            )),
        }
    }
}

impl PartialEq for Element {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0 // CFEqual
    }
}

/// What MingDesk reads from one Accessibility window.
pub struct AxWindow {
    pub element: Element,
    pub title: String,
    pub frame: Option<Rect>,
    pub minimized: bool,
    pub full_screen: bool,
    /// A real window (standard window or dialog), not a sheet, drawer, popover or tooltip.
    pub standard: bool,
}

pub fn describe(element: Element) -> AxWindow {
    let subrole = element.string("AXSubrole").unwrap_or_default();
    AxWindow {
        title: element.string("AXTitle").unwrap_or_default(),
        frame: element.frame(),
        minimized: element.boolean("AXMinimized").unwrap_or(false),
        full_screen: element.boolean("AXFullScreen").unwrap_or(false),
        standard: matches!(
            subrole.as_str(),
            "AXStandardWindow" | "AXDialog" | "AXFloatingWindow"
        ),
        element,
    }
}

/// Windows of `pid` that the app's own window list leaves out (those on other desktops), found by asking for
/// elements by number (private, PLAN.md §7.12), as AltTab does. Stops when every id in `wanted` is found or the
/// time is up.
pub fn remote_windows(pid: i32, wanted: &HashSet<u32>, budget: Duration) -> Vec<(u32, Element)> {
    let started = Instant::now();
    let mut found = Vec::new();
    let mut token = [0u8; 20];
    token[0..4].copy_from_slice(&pid.to_le_bytes());
    token[8..12].copy_from_slice(&0x636f_636f_i32.to_le_bytes()); // "coco"
    for element_id in 0u64..1000 {
        if found.len() == wanted.len() || started.elapsed() > budget {
            break;
        }
        token[12..20].copy_from_slice(&element_id.to_le_bytes());
        let data = CFData::from_buffer(&token);
        // SAFETY: the token is the 20 bytes the call expects; result is owned (create rule) or null.
        let raw = unsafe { _AXUIElementCreateWithRemoteToken(data.as_concrete_TypeRef()) };
        if raw.is_null() {
            continue;
        }
        let element = unsafe { Element::own(raw) };
        unsafe { AXUIElementSetMessagingTimeout(element.ptr(), APP_TIMEOUT) };
        if let Some(id) = element.window_id()
            && wanted.contains(&id)
            && !found.iter().any(|(f, _)| *f == id)
        {
            found.push((id, element));
        }
    }
    found
}

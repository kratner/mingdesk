//! The Mac side of MingDesk being a Dock app (PLAN.md §7.13): choosing MingDesk with ⌘-Tab, like clicking its
//! Dock icon, shows the board, so switching to MingDesk never lands on an app with nothing to show.

use block2::RcBlock;
use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use objc2::msg_send;
use objc2::rc::autoreleasepool;
use objc2::runtime::{AnyClass, AnyObject};

/// Calls `f` each time MingDesk becomes the active app (⌘-Tab, a click on one of its windows, launch).
pub fn on_activate(f: impl Fn() + 'static) {
    let Some(class) = AnyClass::get(c"NSNotificationCenter") else {
        return;
    };
    autoreleasepool(|_| unsafe {
        // SAFETY: documented NSNotificationCenter API; the center keeps the observer (and its block) for as
        // long as MingDesk runs.
        let center: *mut AnyObject = msg_send![class, defaultCenter];
        let block = RcBlock::new(move |_note: *mut AnyObject| f());
        let name = CFString::from_static_string("NSApplicationDidBecomeActiveNotification");
        let _: *mut AnyObject = msg_send![
            center,
            addObserverForName: name.as_concrete_TypeRef() as *mut AnyObject,
            object: std::ptr::null_mut::<AnyObject>(),
            queue: std::ptr::null_mut::<AnyObject>(),
            usingBlock: &*block
        ];
    });
}

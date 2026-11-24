//! Apps' icons for the board, as 64 px PNGs, cached per bundle id: the icon Finder shows for the app
//! (NSWorkspace iconForFile:), drawn into a 64×64 bitmap and saved as PNG. Works back to Mojave.
//!
//! Like the other AppKit reads here (appkit.rs), it runs off the main thread; drawing into a bitmap context of
//! our own is safe there.

use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use objc2::encode::{Encode, Encoding};
use objc2::msg_send;
use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::{AnyClass, AnyObject};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

const SIZE: f64 = 64.0;
/// NSCompositingOperationCopy.
const COPY: usize = 1;
/// NSBitmapImageFileTypePNG.
const PNG: usize = 4;

#[repr(C)]
#[derive(Clone, Copy)]
struct Point {
    x: f64,
    y: f64,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Size {
    w: f64,
    h: f64,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Rect {
    origin: Point,
    size: Size,
}

// SAFETY: the layouts match CGPoint, CGSize and CGRect (CGFloat is f64 on every Mac MingDesk runs on).
unsafe impl Encode for Point {
    const ENCODING: Encoding = Encoding::Struct("CGPoint", &[f64::ENCODING, f64::ENCODING]);
}
unsafe impl Encode for Size {
    const ENCODING: Encoding = Encoding::Struct("CGSize", &[f64::ENCODING, f64::ENCODING]);
}
unsafe impl Encode for Rect {
    const ENCODING: Encoding = Encoding::Struct("CGRect", &[Point::ENCODING, Size::ENCODING]);
}

pub fn icon(bundle_id: &str) -> Option<Vec<u8>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<Vec<u8>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(hit) = cache.lock().unwrap().get(bundle_id) {
        return hit.clone();
    }
    let png = autoreleasepool(|_| draw(bundle_id));
    cache.lock().unwrap().insert(bundle_id.to_string(), png.clone());
    png
}

fn draw(bundle_id: &str) -> Option<Vec<u8>> {
    let workspace_class = AnyClass::get(c"NSWorkspace")?;
    let rep_class = AnyClass::get(c"NSBitmapImageRep")?;
    let context_class = AnyClass::get(c"NSGraphicsContext")?;
    let dictionary_class = AnyClass::get(c"NSDictionary")?;
    let id = CFString::new(bundle_id);
    let rgb = CFString::from_static_string("NSCalibratedRGBColorSpace");
    // SAFETY: documented AppKit API, all available on 10.14. Objects are autoreleased (the caller's pool) except
    // the bitmap, which `Retained` releases.
    unsafe {
        let workspace: *mut AnyObject = msg_send![workspace_class, sharedWorkspace];
        let url: *mut AnyObject = msg_send![
            workspace,
            URLForApplicationWithBundleIdentifier: id.as_concrete_TypeRef() as *mut AnyObject
        ];
        if url.is_null() {
            return None;
        }
        let path: *mut AnyObject = msg_send![url, path];
        if path.is_null() {
            return None;
        }
        let image: *mut AnyObject = msg_send![workspace, iconForFile: path];
        if image.is_null() {
            return None;
        }

        let allocated: *mut AnyObject = msg_send![rep_class, alloc];
        let rep: *mut AnyObject = msg_send![
            allocated,
            initWithBitmapDataPlanes: std::ptr::null_mut::<*mut u8>(),
            pixelsWide: SIZE as isize,
            pixelsHigh: SIZE as isize,
            bitsPerSample: 8isize,
            samplesPerPixel: 4isize,
            hasAlpha: true,
            isPlanar: false,
            colorSpaceName: rgb.as_concrete_TypeRef() as *mut AnyObject,
            bytesPerRow: 0isize,
            bitsPerPixel: 0isize
        ];
        let rep: Retained<AnyObject> = Retained::from_raw(rep)?;
        let context: *mut AnyObject = msg_send![context_class, graphicsContextWithBitmapImageRep: &*rep];
        if context.is_null() {
            return None;
        }
        let () = msg_send![context_class, saveGraphicsState];
        let () = msg_send![context_class, setCurrentContext: context];
        let whole = Rect {
            origin: Point { x: 0.0, y: 0.0 },
            size: Size { w: SIZE, h: SIZE },
        };
        let zero = Rect {
            origin: Point { x: 0.0, y: 0.0 },
            size: Size { w: 0.0, h: 0.0 },
        };
        let () = msg_send![image, drawInRect: whole, fromRect: zero, operation: COPY, fraction: 1.0f64];
        let () = msg_send![context_class, restoreGraphicsState];

        let no_properties: *mut AnyObject = msg_send![dictionary_class, dictionary];
        let data: *mut AnyObject = msg_send![&*rep, representationUsingType: PNG, properties: no_properties];
        if data.is_null() {
            return None;
        }
        let bytes: *const u8 = msg_send![data, bytes];
        let length: usize = msg_send![data, length];
        (!bytes.is_null() && length > 0).then(|| std::slice::from_raw_parts(bytes, length).to_vec())
    }
}

#[cfg(test)]
mod tests {
    /// Finder's icon comes back as a 64×64 PNG; a bundle id nobody has gives none.
    #[test]
    fn finder_has_an_icon_and_a_made_up_app_has_none() {
        let png = super::icon("com.apple.finder").expect("Finder's icon");
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        // The IHDR chunk's width and height, big-endian, right after the signature and chunk header.
        assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 64);
        assert_eq!(u32::from_be_bytes(png[20..24].try_into().unwrap()), 64);
        assert!(super::icon("live.fourounces.no-such-app").is_none());
    }
}

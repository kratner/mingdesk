//! macOS: the Accessibility permission (System Settings → Privacy & Security → Accessibility).
//!
//! macOS ties the permission to the app's code signature, so it sticks across rebuilds only when every build is
//! signed with the same certificate (scripts/dev-cert.sh makes one for this Mac). Under `tauri dev` the binary
//! isn't an app bundle and macOS charges the permission to whatever launched it (VS Code, Terminal), so check the
//! onboarding with the bundled app (`npm run app`).

use super::{Permission, Platform};
use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
use core_foundation::string::{CFString, CFStringRef};

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> bool;
    static kAXTrustedCheckOptionPrompt: CFStringRef;
}

const ACCESSIBILITY_SETTINGS: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";

pub struct MacPlatform;

impl Platform for MacPlatform {
    fn name(&self) -> &'static str {
        "macOS"
    }

    fn permission(&self) -> Permission {
        // SAFETY: no arguments; safe to call from any thread.
        if unsafe { AXIsProcessTrusted() } { Permission::Granted } else { Permission::Missing }
    }

    fn request_permission(&self) {
        // SAFETY: the key is a constant exported by ApplicationServices; the dictionary lives across the call.
        unsafe {
            let key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
            let options = CFDictionary::from_CFType_pairs(&[(key, CFBoolean::true_value())]);
            AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef());
        }
    }

    fn open_permission_settings(&self) -> Result<(), String> {
        std::process::Command::new("open")
            .arg(ACCESSIBILITY_SETTINGS)
            .status()
            .map_err(|e| e.to_string())
            .and_then(|s| if s.success() { Ok(()) } else { Err(format!("open exited with {s}")) })
    }
}

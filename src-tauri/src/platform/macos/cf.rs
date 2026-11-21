//! Small helpers for reading CoreFoundation values the window server and Accessibility hand back.

use core_foundation::array::CFArray;
use core_foundation::base::{CFType, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;

pub type Dict = CFDictionary<CFString, CFType>;

pub fn get(dict: &Dict, key: &str) -> Option<CFType> {
    dict.find(CFString::new(key)).map(|v| v.clone())
}

pub fn int(value: &CFType) -> Option<i64> {
    value.downcast::<CFNumber>()?.to_i64()
}

pub fn float(value: &CFType) -> Option<f64> {
    value.downcast::<CFNumber>()?.to_f64()
}

pub fn string(value: &CFType) -> Option<String> {
    value.downcast::<CFString>().map(|s| s.to_string())
}

pub fn boolean(value: &CFType) -> Option<bool> {
    value.downcast::<CFBoolean>().map(bool::from)
}

pub fn dict(value: &CFType) -> Option<Dict> {
    if value.type_of() != Dict::type_id() {
        return None;
    }
    // SAFETY: checked above that it is a CFDictionary; the get rule retains it for the new wrapper.
    Some(unsafe { Dict::wrap_under_get_rule(value.as_CFTypeRef() as _) })
}

pub fn array(value: &CFType) -> Option<CFArray<CFType>> {
    if value.type_of() != CFArray::<CFType>::type_id() {
        return None;
    }
    // SAFETY: checked above that it is a CFArray; the get rule retains it for the new wrapper.
    Some(unsafe { CFArray::wrap_under_get_rule(value.as_CFTypeRef() as _) })
}

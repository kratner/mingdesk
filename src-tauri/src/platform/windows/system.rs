//! What Export desk says about this PC: the Windows version and the PC's model, from the registry (no WMI, no
//! child processes).

use ::windows::Win32::System::Registry::{HKEY, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegGetValueW};
use ::windows::Win32::System::SystemInformation::{FIRMWARE_TABLE_PROVIDER, GetSystemFirmwareTable};
use ::windows::core::{HSTRING, PCWSTR};

const CURRENT_VERSION: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
const BIOS: &str = r"HARDWARE\DESCRIPTION\System\BIOS";

fn text(root: HKEY, key: &str, value: &str) -> Option<String> {
    let (key, value) = (HSTRING::from(key), HSTRING::from(value));
    let mut len = 0u32;
    // SAFETY: first call asks for the size in bytes; the second fills a buffer of that size.
    unsafe {
        RegGetValueW(root, PCWSTR(key.as_ptr()), PCWSTR(value.as_ptr()), RRF_RT_REG_SZ, None, None, Some(&mut len))
            .ok()
            .ok()?;
        let mut buf = vec![0u16; len as usize / 2 + 1];
        RegGetValueW(
            root,
            PCWSTR(key.as_ptr()),
            PCWSTR(value.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut _),
            Some(&mut len),
        )
        .ok()
        .ok()?;
        let end = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
        let s = String::from_utf16_lossy(&buf[..end]).trim().to_string();
        (!s.is_empty()).then_some(s)
    }
}

fn number(root: HKEY, key: &str, value: &str) -> Option<u32> {
    let (key, value) = (HSTRING::from(key), HSTRING::from(value));
    let mut n = 0u32;
    let mut len = size_of::<u32>() as u32;
    // SAFETY: a DWORD read into `n`.
    unsafe {
        RegGetValueW(
            root,
            PCWSTR(key.as_ptr()),
            PCWSTR(value.as_ptr()),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut n as *mut _ as *mut _),
            Some(&mut len),
        )
        .ok()
        .ok()?;
    }
    Some(n)
}

/// "Windows 11 Home 25H2 (build 26300.1234)". The registry's ProductName still says "Windows 10" on Windows 11,
/// so the name follows the build number (22000 and up is 11).
pub fn os_version() -> Option<String> {
    let product = text(HKEY_LOCAL_MACHINE, CURRENT_VERSION, "ProductName")?;
    let build = text(HKEY_LOCAL_MACHINE, CURRENT_VERSION, "CurrentBuild");
    let build_no: u32 = build.as_deref().and_then(|b| b.parse().ok()).unwrap_or(0);
    let product = if build_no >= 22000 {
        product.replacen("Windows 10", "Windows 11", 1)
    } else {
        product
    };
    let release = text(HKEY_LOCAL_MACHINE, CURRENT_VERSION, "DisplayVersion");
    let ubr = number(HKEY_LOCAL_MACHINE, CURRENT_VERSION, "UBR");
    let mut out = product;
    if let Some(r) = release {
        out = format!("{out} {r}");
    }
    if let Some(b) = build {
        out = match ubr {
            Some(u) => format!("{out} (build {b}.{u})"),
            None => format!("{out} (build {b})"),
        };
    }
    Some(out)
}

/// "Microsoft Corporation Surface Pro 9": the firmware's SMBIOS system strings (what Task Manager and WMI show),
/// or the motherboard's on a self-built PC whose system strings are placeholders. The registry copy of these
/// strings is tried last: it's empty on some PCs (Keith's, 2026-11-23).
pub fn model() -> Option<String> {
    smbios_table()
        .and_then(|t| smbios_model(&t))
        .or_else(|| join(text(HKEY_LOCAL_MACHINE, BIOS, "SystemManufacturer"), text(HKEY_LOCAL_MACHINE, BIOS, "SystemProductName")))
}

fn join(maker: Option<String>, product: Option<String>) -> Option<String> {
    match (maker, product) {
        (Some(m), Some(p)) if p.starts_with(&m) => Some(p),
        (Some(m), Some(p)) => Some(format!("{m} {p}")),
        (m, p) => m.or(p),
    }
}

/// The raw SMBIOS table, as GetSystemFirmwareTable('RSMB') returns it (an 8-byte header, then the structures).
fn smbios_table() -> Option<Vec<u8>> {
    const RSMB: FIRMWARE_TABLE_PROVIDER = FIRMWARE_TABLE_PROVIDER(u32::from_be_bytes(*b"RSMB"));
    // SAFETY: the first call asks for the size; the second fills a buffer of that size.
    unsafe {
        let size = GetSystemFirmwareTable(RSMB, 0, None);
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        let n = GetSystemFirmwareTable(RSMB, 0, Some(&mut buf));
        (n == size).then_some(buf)
    }
}

/// Strings firmware vendors leave in when nobody filled them in.
fn placeholder(s: &str) -> bool {
    let l = s.to_ascii_lowercase();
    l.is_empty()
        || l.contains("to be filled")
        || l.contains("default string")
        || l == "system product name"
        || l == "system manufacturer"
        || l == "not applicable"
        || l == "o.e.m."
}

/// The model from an SMBIOS table: the System Information structure (type 1), else Baseboard (type 2).
pub fn smbios_model(raw: &[u8]) -> Option<String> {
    let mut system = (None, None);
    let mut board = (None, None);
    let mut at = 8; // past RawSMBIOSData's header
    while at + 4 <= raw.len() {
        let kind = raw[at];
        let len = raw[at + 1] as usize;
        if len < 4 || at + len > raw.len() {
            break;
        }
        let formatted = &raw[at..at + len];
        // The strings follow the formatted part, each NUL-terminated; an empty string ends the set. A structure
        // with no strings is followed by two NULs.
        let mut strings: Vec<String> = Vec::new();
        let mut p = at + len;
        if raw.get(p..p + 2)? == [0, 0] {
            p += 2;
        } else {
            loop {
                let end = p + raw.get(p..)?.iter().position(|b| *b == 0)?;
                if end == p {
                    p += 1;
                    break;
                }
                strings.push(String::from_utf8_lossy(&raw[p..end]).trim().to_string());
                p = end + 1;
            }
        }
        let string = |index: usize| -> Option<String> {
            let i = *formatted.get(index)? as usize;
            strings.get(i.checked_sub(1)?).filter(|s| !placeholder(s)).cloned()
        };
        match kind {
            1 => system = (string(4), string(5)),
            2 => board = (string(4), string(5)),
            127 => break, // end of table
            _ => {}
        }
        at = p;
    }
    if system.1.is_some() {
        join(system.0, system.1)
    } else {
        join(board.0, board.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An SMBIOS structure: type, the formatted part after the 4-byte header, then its strings.
    fn structure(kind: u8, after_header: &[u8], strings: &[&str]) -> Vec<u8> {
        let mut s = vec![kind, (4 + after_header.len()) as u8, 0, 0];
        s.extend_from_slice(after_header);
        for t in strings {
            s.extend_from_slice(t.as_bytes());
            s.push(0);
        }
        if strings.is_empty() {
            s.push(0);
        }
        s.push(0);
        s
    }

    fn table(structures: &[Vec<u8>]) -> Vec<u8> {
        let mut t = vec![0u8; 8];
        for s in structures {
            t.extend_from_slice(s);
        }
        t.extend(structure(127, &[], &[]));
        t
    }

    #[test]
    fn a_laptop_names_itself_in_its_system_strings() {
        let t = table(&[
            structure(0, &[1, 2], &["Microsoft Corporation", "1.2.3"]), // BIOS
            structure(1, &[1, 2, 0], &["Microsoft Corporation", "Surface Pro 9"]),
            structure(2, &[1, 2], &["Microsoft Corporation", "Surface Pro 9 board"]),
        ]);
        assert_eq!(smbios_model(&t).as_deref(), Some("Microsoft Corporation Surface Pro 9"));
    }

    #[test]
    fn a_self_built_pc_falls_back_to_its_motherboard() {
        let t = table(&[
            structure(1, &[1, 2, 0], &["To Be Filled By O.E.M.", "To Be Filled By O.E.M."]),
            structure(2, &[1, 2], &["GreatWall", "B760M-G"]),
        ]);
        assert_eq!(smbios_model(&t).as_deref(), Some("GreatWall B760M-G"));
    }

    #[test]
    fn a_product_named_after_its_maker_isnt_said_twice_and_junk_is_ignored() {
        let t = table(&[structure(1, &[1, 2], &["LENOVO", "LENOVO ThinkPad X1"])]);
        assert_eq!(smbios_model(&t).as_deref(), Some("LENOVO ThinkPad X1"));
        assert_eq!(smbios_model(&[0u8; 8]), None);
        assert_eq!(smbios_model(&[1, 2, 3]), None);
        let mut cut = table(&[structure(1, &[1, 2], &["Dell Inc.", "XPS 13"])]);
        cut.truncate(16);
        assert_eq!(smbios_model(&cut), None, "a truncated table");
    }
}

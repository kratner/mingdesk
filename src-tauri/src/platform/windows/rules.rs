//! The Windows platform's decisions, kept apart from the OS calls so they're unit-tested on any Windows machine
//! without opening a window: which top-level windows count, coordinate conversion, display names and ids, and how
//! virtual desktops are numbered.

use crate::platform::Rect;

/// The shell's own top-level windows: the desktop behind everything and the taskbars. Never listed.
pub const SHELL_CLASSES: &[&str] = &[
    "Progman",
    "WorkerW",
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
];

/// What MingDesk reads about a top-level window before deciding whether it's one people use.
#[derive(Debug, Clone, Default)]
pub struct Candidate<'a> {
    pub class: &'a str,
    pub title: &'a str,
    /// IsWindowVisible.
    pub visible: bool,
    /// Has an owner window (dialogs, palettes, tooltips).
    pub owned: bool,
    /// WS_EX_TOOLWINDOW: floating toolbars, notification popups; not in the taskbar or Alt+Tab.
    pub tool_window: bool,
    /// WS_EX_APPWINDOW: the app asks for a taskbar button, so it's an app window even untitled.
    pub app_window: bool,
    /// DWMWA_CLOAKED is set: DWM isn't drawing it.
    pub cloaked: bool,
    /// The virtual-desktop manager places it on a desktop other than the one showing.
    pub on_other_desktop: bool,
}

/// Whether a top-level window is one MingDesk lists: roughly what the taskbar and Alt+Tab show.
///
/// Cloaked windows are dropped (suspended Store apps, the shell's hidden panels) except one on another virtual
/// desktop: Windows cloaks those too, and the board lists them dimmed, as it lists windows on another Mission
/// Control desktop on the Mac.
pub fn counts(c: &Candidate) -> bool {
    c.visible
        && !c.owned
        && !c.tool_window
        && (!c.title.trim().is_empty() || c.app_window)
        && !SHELL_CLASSES.contains(&c.class)
        && (!c.cloaked || c.on_other_desktop)
}

/// A Win32 RECT (left, top, right, bottom) as a MingDesk rectangle.
pub fn rect(left: i32, top: i32, right: i32, bottom: i32) -> Rect {
    Rect::new(
        f64::from(left),
        f64::from(top),
        f64::from(right - left),
        f64::from(bottom - top),
    )
}

/// A minimized window's restored frame. GetWindowPlacement gives it in *workspace* coordinates: relative to the
/// work area (the monitor minus the taskbar), not the screen. Shifting by the work area's offset inside its
/// monitor turns it back into screen coordinates.
pub fn workspace_to_screen(normal: Rect, monitor: &Rect, work: &Rect) -> Rect {
    Rect::new(
        normal.x + (work.x - monitor.x),
        normal.y + (work.y - monitor.y),
        normal.w,
        normal.h,
    )
}

/// The invisible resize borders around a window: how far GetWindowRect reaches past what DWM draws, per side
/// (left, top, right, bottom). Read while a window is visible, so its restored frame can be trimmed the same way
/// once it's minimized (GetWindowPlacement includes them, and DWM can't measure a minimized window).
pub type Insets = (f64, f64, f64, f64);

pub fn insets(window_rect: &Rect, drawn: &Rect) -> Insets {
    (
        drawn.x - window_rect.x,
        drawn.y - window_rect.y,
        window_rect.right() - drawn.right(),
        window_rect.bottom() - drawn.bottom(),
    )
}

/// `r` without its invisible borders. Insets outside 0..=32 px are taken as a misreading and ignored.
pub fn trim(r: Rect, (l, t, rt, b): Insets) -> Rect {
    let ok = |v: f64| (0.0..=32.0).contains(&v);
    if !(ok(l) && ok(t) && ok(rt) && ok(b)) || r.w <= l + rt || r.h <= t + b {
        return r;
    }
    Rect::new(r.x + l, r.y + t, r.w - l - rt, r.h - t - b)
}

/// A Store app's package folder from its exe path: `C:\Program Files\WindowsApps\<package full name>\…`.
pub fn store_package(exe: &str) -> Option<(&str, &str)> {
    let lower = exe.to_ascii_lowercase();
    let at = lower.find(r"\windowsapps\")? + r"\windowsapps\".len();
    let name_len = exe[at..].find('\\')?;
    Some((&exe[..at + name_len], &exe[at..at + name_len]))
}

/// The app's name from its AppxManifest.xml: `<Properties><DisplayName>`. Often a resource reference
/// ("ms-resource:AppName"), which the caller resolves against the package.
pub fn manifest_display_name(xml: &str) -> Option<&str> {
    let props = &xml[xml.find("<Properties")?..];
    let props = &props[..props.find("</Properties>").unwrap_or(props.len())];
    let start = props.find("<DisplayName>")? + "<DisplayName>".len();
    let end = start + props[start..].find("</DisplayName>")?;
    let name = props[start..end].trim();
    (!name.is_empty()).then_some(name)
}

/// What SHLoadIndirectString needs to resolve a manifest's "ms-resource:…" name: `@{<package full name>?<uri>}`,
/// where a bare key lives under the package's `resources` map ("AppName" → ms-resource://<name>/resources/AppName),
/// a path starting with `/` is under the package, and one starting with `//` is complete already.
pub fn indirect_string(full_name: &str, resource: &str) -> String {
    let package = full_name.split('_').next().unwrap_or(full_name);
    let uri = if resource.starts_with("//") {
        format!("ms-resource:{resource}")
    } else if resource.starts_with('/') {
        format!("ms-resource://{package}{resource}")
    } else {
        format!("ms-resource://{package}/resources/{resource}")
    };
    format!("@{{{full_name}?{uri}}}")
}

/// Full screen on Windows means no title bar and covering the whole monitor (games, F11 in a browser, video
/// players). A maximized window keeps its title bar and stops at the taskbar, so it doesn't count.
pub fn is_full_screen(frame: &Rect, monitor: &Rect, has_caption: bool) -> bool {
    const SLACK: f64 = 1.0;
    !has_caption
        && frame.x <= monitor.x + SLACK
        && frame.y <= monitor.y + SLACK
        && frame.right() >= monitor.right() - SLACK
        && frame.bottom() >= monitor.bottom() - SLACK
}

/// A display's id: its monitor device path ("\\?\DISPLAY#DEL41B4#5&...&UID4353#{e6f07b5f-...}"), which stays
/// the same across reboots and re-plugging into the same port. Mirrored monitors share one desktop rectangle
/// (one HMONITOR); they get the first path in sorted order so the id doesn't change with enumeration order. Only
/// when Windows gives no path at all does it fall back to the GDI name ("\\.\DISPLAY1"), which isn't stable.
pub fn display_id(paths: &[&str], gdi: &str) -> String {
    paths
        .iter()
        .copied()
        .filter(|p| !p.is_empty())
        .min()
        .map_or_else(|| gdi.to_string(), str::to_string)
}

/// What Settings → Display calls a monitor: its friendly name ("DELL U2720Q"); a laptop's own panel, which has
/// none, "Built-in display"; otherwise "Display N" from the GDI name.
pub fn display_name(friendly: &str, internal: bool, gdi: &str) -> String {
    let friendly = friendly.trim();
    if !friendly.is_empty() {
        return friendly.to_string();
    }
    if internal {
        return "Built-in display".to_string();
    }
    let number: String = gdi
        .chars()
        .rev()
        .take_while(char::is_ascii_digit)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    if number.is_empty() {
        "Display".to_string()
    } else {
        format!("Display {number}")
    }
}

/// A GUID as Windows stores it in 16 bytes (Data1, Data2, Data3 little-endian, then Data4 as is), as one number in
/// the layout of the `windows` crate's `GUID::to_u128`.
pub fn guid_from_bytes(b: &[u8; 16]) -> u128 {
    let data1 = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
    let data2 = u16::from_le_bytes([b[4], b[5]]);
    let data3 = u16::from_le_bytes([b[6], b[7]]);
    let data4 = u64::from_be_bytes([b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]]);
    (u128::from(data1) << 96) | (u128::from(data2) << 80) | (u128::from(data3) << 64) | u128::from(data4)
}

/// The registry's list of virtual desktops (VirtualDesktopIDs): GUIDs back to back, in Task View's order.
pub fn guid_list(bytes: &[u8]) -> Vec<u128> {
    bytes
        .chunks_exact(16)
        .map(|c| guid_from_bytes(c.try_into().expect("16 bytes")))
        .collect()
}

/// A desktop's id on the board (`Desktop::id`, `WindowInfo::desktop`): its GUID folded to 64 bits. The same
/// desktop always gets the same id.
pub fn desktop_key(guid: u128) -> u64 {
    ((guid >> 64) as u64) ^ (guid as u64)
}

/// The desktops in the order the board numbers them (Desktop 1, 2, …): Task View's order from the registry, then
/// any desktop a window is on that the registry didn't list, in the order first seen. The registry list is
/// undocumented; when it's missing, first-seen order is the best Windows offers publicly.
pub fn number_desktops(registry: &[u128], seen: &[u128]) -> Vec<u128> {
    let mut out: Vec<u128> = Vec::new();
    for g in registry.iter().chain(seen.iter()) {
        if *g != 0 && !out.contains(g) {
            out.push(*g);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app<'a>(class: &'a str, title: &'a str) -> Candidate<'a> {
        Candidate {
            class,
            title,
            visible: true,
            ..Default::default()
        }
    }

    #[test]
    fn an_ordinary_titled_window_counts() {
        assert!(counts(&app("Notepad", "notes.txt - Notepad")));
    }

    #[test]
    fn hidden_owned_tool_and_untitled_windows_dont() {
        let base = app("Notepad", "notes.txt - Notepad");
        assert!(!counts(&Candidate { visible: false, ..base.clone() }));
        assert!(!counts(&Candidate { owned: true, ..base.clone() }), "a dialog");
        assert!(!counts(&Candidate { tool_window: true, ..base.clone() }));
        assert!(!counts(&app("SomeHelper", "  ")), "untitled");
        assert!(
            counts(&Candidate { app_window: true, ..app("SomeApp", "") }),
            "untitled but asks for a taskbar button"
        );
    }

    #[test]
    fn the_shell_is_never_listed() {
        for class in SHELL_CLASSES {
            assert!(!counts(&app(class, "Program Manager")), "{class}");
        }
    }

    #[test]
    fn cloaked_windows_count_only_when_on_another_desktop() {
        let base = app("ApplicationFrameWindow", "Calculator");
        assert!(!counts(&Candidate { cloaked: true, ..base.clone() }), "a suspended Store app");
        assert!(counts(&Candidate {
            cloaked: true,
            on_other_desktop: true,
            ..base
        }));
    }

    #[test]
    fn win32_rects_become_x_y_w_h() {
        assert_eq!(rect(-1920, 0, 0, 1080), Rect::new(-1920.0, 0.0, 1920.0, 1080.0));
    }

    #[test]
    fn workspace_coordinates_shift_by_the_taskbar() {
        let monitor = Rect::new(0.0, 0.0, 2560.0, 1440.0);
        // Taskbar on the left, 60 px wide: the work area starts at x=60.
        let work = Rect::new(60.0, 0.0, 2500.0, 1440.0);
        assert_eq!(
            workspace_to_screen(Rect::new(100.0, 50.0, 800.0, 600.0), &monitor, &work),
            Rect::new(160.0, 50.0, 800.0, 600.0)
        );
        // Bottom taskbar (the default) changes nothing.
        let work = Rect::new(0.0, 0.0, 2560.0, 1392.0);
        assert_eq!(
            workspace_to_screen(Rect::new(100.0, 50.0, 800.0, 600.0), &monitor, &work),
            Rect::new(100.0, 50.0, 800.0, 600.0)
        );
        // A monitor left of and above the primary (negative coordinates), taskbar on top.
        let monitor = Rect::new(-1080.0, -400.0, 1080.0, 1920.0);
        let work = Rect::new(-1080.0, -352.0, 1080.0, 1872.0);
        assert_eq!(
            workspace_to_screen(Rect::new(-1000.0, -300.0, 500.0, 500.0), &monitor, &work),
            Rect::new(-1000.0, -252.0, 500.0, 500.0)
        );
    }

    #[test]
    fn invisible_borders_are_measured_and_trimmed() {
        // Keith's PC, 2026-11-23: a File Explorer window drawn at (1940, 329, 1534×907) whose placement said
        // (1933, 329, 1548×914): 7 px left, right and bottom, none at the top.
        let outer = Rect::new(1933.0, 329.0, 1548.0, 914.0);
        let drawn = Rect::new(1940.0, 329.0, 1534.0, 907.0);
        let i = insets(&outer, &drawn);
        assert_eq!(i, (7.0, 0.0, 7.0, 7.0));
        assert_eq!(trim(outer, i), drawn);
        assert_eq!(trim(outer, (-5.0, 0.0, 7.0, 7.0)), outer, "nonsense insets are ignored");
        assert_eq!(trim(Rect::new(0.0, 0.0, 10.0, 10.0), (7.0, 0.0, 7.0, 7.0)), Rect::new(0.0, 0.0, 10.0, 10.0));
    }

    #[test]
    fn store_apps_are_found_by_their_package_folder() {
        let exe = r"C:\Program Files\WindowsApps\5319275A.WhatsAppDesktop_2.2644.101.0_x64__cv1g1gvanyjgm\WhatsApp.Root.exe";
        let (folder, full) = store_package(exe).unwrap();
        assert_eq!(full, "5319275A.WhatsAppDesktop_2.2644.101.0_x64__cv1g1gvanyjgm");
        assert!(folder.ends_with(full));
        assert_eq!(store_package(r"C:\Program Files\Google\Chrome\Application\chrome.exe"), None);
    }

    #[test]
    fn manifest_names_come_from_properties() {
        let xml = r#"<Package><Properties><DisplayName>WhatsApp</DisplayName><PublisherDisplayName>WhatsApp Inc.</PublisherDisplayName></Properties>
            <Applications><Application><uap:VisualElements DisplayName="ms-resource:Other"/></Application></Applications></Package>"#;
        assert_eq!(manifest_display_name(xml), Some("WhatsApp"));
        let xml = "<Properties>\n  <DisplayName>ms-resource:AppDisplayName</DisplayName>\n</Properties>";
        assert_eq!(manifest_display_name(xml), Some("ms-resource:AppDisplayName"));
        assert_eq!(manifest_display_name("<Package/>"), None);
    }

    #[test]
    fn manifest_resources_resolve_against_their_package() {
        let full = "Microsoft.WindowsCalculator_11.2502.2.0_x64__8wekyb3d8bbwe";
        assert_eq!(
            indirect_string(full, "AppStoreName"),
            "@{Microsoft.WindowsCalculator_11.2502.2.0_x64__8wekyb3d8bbwe?ms-resource://Microsoft.WindowsCalculator/resources/AppStoreName}"
        );
        assert!(indirect_string(full, "/Resources/AppName").ends_with("?ms-resource://Microsoft.WindowsCalculator/Resources/AppName}"));
        assert!(indirect_string(full, "//Other/x").ends_with("?ms-resource://Other/x}"));
    }

    #[test]
    fn full_screen_means_captionless_and_covering_the_monitor() {
        let monitor = Rect::new(-1920.0, 0.0, 1920.0, 1080.0);
        assert!(is_full_screen(&monitor, &monitor, false));
        assert!(!is_full_screen(&monitor, &monitor, true), "maximized with a title bar");
        assert!(!is_full_screen(
            &Rect::new(-1920.0, 0.0, 1920.0, 1032.0),
            &monitor,
            false
        ));
    }

    #[test]
    fn display_ids_prefer_the_device_path_and_ignore_enumeration_order() {
        let a = r"\\?\DISPLAY#DEL41B4#5&1&0&UID4353#{e6f07b5f-ee97-4a90-b076-33f57bf4eaa7}";
        let b = r"\\?\DISPLAY#GSM5B7F#5&1&0&UID4357#{e6f07b5f-ee97-4a90-b076-33f57bf4eaa7}";
        assert_eq!(display_id(&[a], r"\\.\DISPLAY1"), a);
        assert_eq!(display_id(&[b, a], r"\\.\DISPLAY1"), display_id(&[a, b], r"\\.\DISPLAY1"), "mirrored");
        assert_eq!(display_id(&[""], r"\\.\DISPLAY3"), r"\\.\DISPLAY3");
        assert_eq!(display_id(&[], r"\\.\DISPLAY3"), r"\\.\DISPLAY3");
    }

    #[test]
    fn display_names() {
        assert_eq!(display_name("DELL U2720Q", false, r"\\.\DISPLAY1"), "DELL U2720Q");
        assert_eq!(display_name("", true, r"\\.\DISPLAY1"), "Built-in display");
        assert_eq!(display_name(" ", false, r"\\.\DISPLAY12"), "Display 12");
        assert_eq!(display_name("", false, "odd"), "Display");
    }

    #[test]
    fn guids_read_from_registry_bytes() {
        // {6B8A1A40-5E3A-4F0B-9B8C-0D1E2F304152} as Windows stores it.
        let bytes: [u8; 16] = [
            0x40, 0x1A, 0x8A, 0x6B, 0x3A, 0x5E, 0x0B, 0x4F, 0x9B, 0x8C, 0x0D, 0x1E, 0x2F, 0x30, 0x41, 0x52,
        ];
        assert_eq!(guid_from_bytes(&bytes), 0x6B8A1A40_5E3A_4F0B_9B8C_0D1E2F304152);
        let mut two = bytes.to_vec();
        two.extend_from_slice(&[0xFF; 16]);
        two.extend_from_slice(&[1, 2, 3]); // a stray tail is ignored
        assert_eq!(guid_list(&two).len(), 2);
    }

    #[test]
    fn desktops_number_in_task_view_order_then_as_seen() {
        let (a, b, c, d) = (0xA, 0xB, 0xC, 0xD);
        assert_eq!(number_desktops(&[b, a, c], &[a, c]), vec![b, a, c]);
        assert_eq!(number_desktops(&[a, b], &[d, b]), vec![a, b, d], "a desktop the registry missed");
        assert_eq!(number_desktops(&[], &[c, a, c, 0]), vec![c, a], "no registry: first seen, no zero GUID");
    }

    #[test]
    fn desktop_keys_are_stable_and_distinct() {
        let a = 0x6B8A1A40_5E3A_4F0B_9B8C_0D1E2F304152;
        let b = 0x6B8A1A40_5E3A_4F0B_9B8C_0D1E2F304153;
        assert_eq!(desktop_key(a), desktop_key(a));
        assert_ne!(desktop_key(a), desktop_key(b));
    }
}

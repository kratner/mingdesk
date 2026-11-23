//! Displays: one per monitor rectangle on the desktop (EnumDisplayMonitors), named and identified through the
//! display configuration API (QueryDisplayConfig + DisplayConfigGetDeviceInfo).
//!
//! Coordinates are physical pixels with the origin at the primary monitor's top-left (PLAN.md §9): what Win32
//! reports to a per-monitor DPI-aware thread, which every caller here is (see `super::dpi`).

use super::rules;
use crate::platform::{Display, Rect};
use ::windows::Win32::Devices::Display::{
    DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
    DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DISPLAYPORT_EMBEDDED,
    DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INTERNAL, DISPLAYCONFIG_OUTPUT_TECHNOLOGY_LVDS,
    DISPLAYCONFIG_OUTPUT_TECHNOLOGY_UDI_EMBEDDED, DISPLAYCONFIG_PATH_INFO,
    DISPLAYCONFIG_SOURCE_DEVICE_NAME, DISPLAYCONFIG_TARGET_DEVICE_NAME, DisplayConfigGetDeviceInfo,
    GetDisplayConfigBufferSizes, QDC_ONLY_ACTIVE_PATHS, QueryDisplayConfig,
};
use ::windows::Win32::Foundation::{ERROR_SUCCESS, LPARAM, RECT};
use ::windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
};
use ::windows::core::BOOL;
use std::collections::HashMap;

/// MONITORINFOF_PRIMARY.
const PRIMARY: u32 = 1;

/// A connected display, with the work area (the display minus the taskbar) that minimized windows' restored
/// frames are measured from.
#[derive(Debug, Clone)]
pub struct Monitor {
    pub display: Display,
    pub work: Rect,
}

fn wide(s: &[u16]) -> String {
    let end = s.iter().position(|c| *c == 0).unwrap_or(s.len());
    String::from_utf16_lossy(&s[..end])
}

/// What the display configuration says about each active output, keyed by GDI name ("\\.\DISPLAY1"): the
/// monitor's friendly name, device path, and whether it's a laptop's own panel. A mirrored set lists one GDI
/// name with several monitors.
#[derive(Debug, Default)]
struct Output {
    friendly: String,
    paths: Vec<String>,
    internal: bool,
}

fn outputs() -> HashMap<String, Output> {
    let mut out: HashMap<String, Output> = HashMap::new();
    let (mut n_paths, mut n_modes) = (0u32, 0u32);
    // SAFETY: the counts are written by the call; the buffers are sized from them and outlive the query.
    unsafe {
        if GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut n_paths, &mut n_modes)
            != ERROR_SUCCESS
        {
            return out;
        }
        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); n_paths as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); n_modes as usize];
        if QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut n_paths,
            paths.as_mut_ptr(),
            &mut n_modes,
            modes.as_mut_ptr(),
            None,
        ) != ERROR_SUCCESS
        {
            return out;
        }
        for p in &paths[..n_paths as usize] {
            let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME::default();
            source.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME;
            source.header.size = size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32;
            source.header.adapterId = p.sourceInfo.adapterId;
            source.header.id = p.sourceInfo.id;
            if DisplayConfigGetDeviceInfo(&mut source.header) != 0 {
                continue;
            }
            let mut target = DISPLAYCONFIG_TARGET_DEVICE_NAME::default();
            target.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME;
            target.header.size = size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32;
            target.header.adapterId = p.targetInfo.adapterId;
            target.header.id = p.targetInfo.id;
            let named = DisplayConfigGetDeviceInfo(&mut target.header) == 0;
            let entry = out.entry(wide(&source.viewGdiDeviceName)).or_default();
            if named {
                if entry.friendly.is_empty() {
                    entry.friendly = wide(&target.monitorFriendlyDeviceName);
                }
                entry.paths.push(wide(&target.monitorDevicePath));
                let tech = target.outputTechnology;
                entry.internal |= tech == DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INTERNAL
                    || tech == DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DISPLAYPORT_EMBEDDED
                    || tech == DISPLAYCONFIG_OUTPUT_TECHNOLOGY_UDI_EMBEDDED
                    || tech == DISPLAYCONFIG_OUTPUT_TECHNOLOGY_LVDS;
            }
        }
    }
    out
}

unsafe extern "system" fn collect(h: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
    // SAFETY: `data` is the Vec passed by `list` below, alive for the whole enumeration.
    let handles = unsafe { &mut *(data.0 as *mut Vec<HMONITOR>) };
    handles.push(h);
    true.into()
}

/// Every connected display, primary first as Windows enumerates them.
pub fn list() -> Vec<Monitor> {
    let mut handles: Vec<HMONITOR> = Vec::new();
    // SAFETY: the callback only pushes into `handles`, which outlives the call.
    let _ = unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(collect),
            LPARAM(&mut handles as *mut _ as isize),
        )
    };
    let outputs = outputs();
    handles
        .into_iter()
        .filter_map(|h| {
            let mut info = MONITORINFOEXW::default();
            info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
            // SAFETY: cbSize says the buffer is the extended struct, which starts with MONITORINFO.
            let ok = unsafe { GetMonitorInfoW(h, &mut info as *mut _ as *mut MONITORINFO) };
            if !ok.as_bool() {
                return None;
            }
            let gdi = wide(&info.szDevice);
            let m = info.monitorInfo;
            let output = outputs.get(&gdi);
            let paths: Vec<&str> = output.map_or(Vec::new(), |o| o.paths.iter().map(String::as_str).collect());
            Some(Monitor {
                display: Display {
                    id: rules::display_id(&paths, &gdi),
                    name: rules::display_name(
                        output.map_or("", |o| o.friendly.as_str()),
                        output.is_some_and(|o| o.internal),
                        &gdi,
                    ),
                    frame: rules::rect(m.rcMonitor.left, m.rcMonitor.top, m.rcMonitor.right, m.rcMonitor.bottom),
                    main: m.dwFlags & PRIMARY != 0,
                },
                work: rules::rect(m.rcWork.left, m.rcWork.top, m.rcWork.right, m.rcWork.bottom),
            })
        })
        .collect()
}

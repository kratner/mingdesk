//! Windows: seeing and acting on windows (M1 on Windows, started 2026-11-23 ahead of M6; PLAN.md §2, §6).
//!
//! Win32 needs no permission to list, focus, minimize or close other apps' windows, so `permission` is
//! NotNeeded and the board skips onboarding. A snapshot joins four sources: the monitors (displays.rs), every
//! top-level window front to back with its app (list.rs), the virtual-desktop manager (desktops.rs) and the pure
//! rules that decide what's listed and convert coordinates (rules.rs, unit-tested).
//!
//! Coordinates: physical pixels, origin at the primary monitor's top-left (PLAN.md §9 says why). Every call that
//! reads coordinates runs per-monitor DPI-aware (`dpi`), so the numbers are physical whatever the process's own
//! awareness is: the app (Tauri makes it per-monitor aware), `cargo test`, or a probe.

mod actions;
mod desktops;
mod displays;
mod list;
mod rules;
mod system;
mod watch;

pub use system::{model, os_version};

use super::{Desktop, Permission, Platform, Sink, Snapshot, WindowInfo};
use ::windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext,
};
use ::windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
use std::collections::HashMap;

/// Where minimized windows start in the stacking order (they have no place in it), as on the Mac.
const STACK_REST: u32 = 100_000;

pub struct WinPlatform;

/// Makes this thread per-monitor DPI-aware until dropped, so every coordinate it reads is in physical pixels.
struct PhysicalPixels(DPI_AWARENESS_CONTEXT);

impl PhysicalPixels {
    fn enter() -> Self {
        // SAFETY: changes only this thread's awareness; the previous one is put back on drop.
        Self(unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) })
    }
}

impl Drop for PhysicalPixels {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            // SAFETY: restores what `enter` replaced.
            unsafe { SetThreadDpiAwarenessContext(self.0) };
        }
    }
}

/// The desk as Windows reports it now.
pub fn read() -> Snapshot {
    let _physical = PhysicalPixels::enter();
    let own = std::process::id();
    let monitors = displays::list();
    let vdm = desktops::Vdm::new();
    // SAFETY: a plain query.
    let foreground = list::id_of(unsafe { GetForegroundWindow() });

    let raws: Vec<list::Raw> = list::top_level()
        .into_iter()
        .enumerate()
        .filter_map(|(z, h)| list::describe(h, z as u32, vdm.as_ref()))
        .collect();

    // Desktops: numbered once, then listed under every display (a Windows desktop spans them all).
    let seen: Vec<u128> = raws.iter().filter_map(|r| r.desktop).collect();
    let ordered = rules::number_desktops(&desktops::registry_order(), &seen);
    let current = desktops::registry_current()
        .filter(|g| ordered.contains(g))
        .or_else(|| raws.iter().find(|r| r.on_current_desktop).and_then(|r| r.desktop));
    let desktop_list: Vec<Desktop> = if ordered.len() > 1 {
        monitors
            .iter()
            .flat_map(|m| {
                ordered.iter().enumerate().map(move |(i, g)| Desktop {
                    id: rules::desktop_key(*g),
                    display: m.display.id.clone(),
                    index: i as u32 + 1,
                    full_screen: false,
                    current: current == Some(*g),
                })
            })
            .collect()
    } else {
        Vec::new() // one desktop: nothing to tell apart
    };

    let mut apps: HashMap<u32, (Option<String>, String)> = HashMap::new();
    let windows = raws
        .into_iter()
        .map(|r| {
            let (app_id, app) = apps
                .entry(r.pid)
                .or_insert_with(|| {
                    if r.pid == own {
                        return (list::exe_path(own), "MingDesk".to_string());
                    }
                    let path = list::exe_path(r.pid);
                    let name = path.as_deref().map_or_else(|| r.class.clone(), list::app_name);
                    (path, name)
                })
                .clone();
            let frame = if r.minimized {
                // Restored frames are in workspace coordinates: measured from the work area of the monitor they
                // land on.
                monitors
                    .iter()
                    .max_by(|a, b| {
                        a.work.overlap(&r.frame).total_cmp(&b.work.overlap(&r.frame))
                    })
                    .map_or(r.frame, |m| rules::workspace_to_screen(r.frame, &m.display.frame, &m.work))
            } else {
                r.frame
            };
            let full_screen = !r.minimized
                && monitors
                    .iter()
                    .any(|m| rules::is_full_screen(&frame, &m.display.frame, r.has_caption));
            WindowInfo {
                id: r.id,
                pid: r.pid as i32,
                app,
                app_id,
                title: r.title,
                frame,
                minimized: r.minimized,
                full_screen,
                on_current_desktop: r.on_current_desktop,
                desktop: if ordered.len() > 1 { r.desktop.map(rules::desktop_key) } else { None },
                focused: r.id == foreground,
                order: if r.minimized { STACK_REST + r.z } else { r.z },
            }
        })
        .collect();

    Snapshot {
        displays: monitors.into_iter().map(|m| m.display).collect(),
        windows,
        desktops: desktop_list,
    }
}

impl Platform for WinPlatform {
    fn name(&self) -> &'static str {
        "Windows"
    }
    fn permission(&self) -> Permission {
        Permission::NotNeeded
    }
    fn request_permission(&self) {}
    fn open_permission_settings(&self) -> Result<(), String> {
        Ok(())
    }

    fn snapshot(&self) -> Result<Snapshot, String> {
        Ok(read())
    }

    fn focus(&self, window: u32) -> Result<(), String> {
        actions::focus(window)
    }

    fn minimize(&self, window: u32) -> Result<(), String> {
        actions::minimize(window)
    }

    fn close(&self, window: u32) -> Result<(), String> {
        actions::close(window)
    }

    fn bring_app_forward(&self, pid: i32) -> Result<(), String> {
        // The app's windows on the desktop showing now, back-most first so raising them in turn keeps their order.
        let mut mine: Vec<WindowInfo> = read()
            .windows
            .into_iter()
            .filter(|w| w.pid == pid && w.on_current_desktop && !w.minimized)
            .collect();
        mine.sort_by_key(|w| std::cmp::Reverse(w.order));
        actions::bring_forward(&mine.iter().map(|w| w.id).collect::<Vec<_>>())
    }

    fn watch(&self, sink: Sink) {
        watch::start(sink);
    }
}

#[cfg(test)]
mod probe {
    use super::*;
    use std::time::Instant;

    /// What MingDesk sees on this PC. Reads only; opens, focuses and changes nothing.
    /// `cargo test probe_this_pc -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn probe_this_pc() {
        let started = Instant::now();
        let snap = read();
        println!("snapshot took {:?}", started.elapsed());
        println!("os: {:?}\nmodel: {:?}", os_version(), model());
        println!("\nDisplays:");
        for d in &snap.displays {
            println!("  {} | {} | {:?} | main={}", d.name, d.id, d.frame, d.main);
        }
        println!("\nDesktops (listed under each display):");
        for x in snap.desktops.iter().filter(|x| Some(&x.display) == snap.displays.first().map(|d| &d.id)) {
            println!("  {} id={} current={}", x.index, x.id, x.current);
        }
        println!("\nWindows (front to back):");
        let mut rows = snap.windows.clone();
        rows.sort_by_key(|w| w.order);
        for w in rows {
            println!(
                "  {:>9} {:<22} {:<44} here={} min={} fs={} focus={} desk={:?} {:?} {}",
                w.id,
                w.app.chars().take(22).collect::<String>(),
                w.title.chars().take(44).collect::<String>(),
                w.on_current_desktop as u8,
                w.minimized as u8,
                w.full_screen as u8,
                w.focused as u8,
                w.desktop.map(|d| d % 10_000),
                w.frame,
                w.app_id.as_deref().unwrap_or("?"),
            );
        }
    }
}

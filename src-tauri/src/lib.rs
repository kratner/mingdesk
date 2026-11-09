//! MingDesk's core: the menu-bar icon, the board window and the commands the board calls.

mod platform;

use platform::{Permission, Platform};
use serde::Serialize;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, State, WindowEvent};

const BOARD: &str = "main";

type PlatformState = Box<dyn Platform>;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Status {
    permission: Permission,
    platform: &'static str,
    version: &'static str,
}

fn status_of(platform: &dyn Platform) -> Status {
    Status { permission: platform.permission(), platform: platform.name(), version: env!("CARGO_PKG_VERSION") }
}

/// The board's "Allow access" button: have the OS list MingDesk, then open the page where the user ticks it.
fn allow_access_on(platform: &dyn Platform) -> Result<Status, String> {
    if platform.permission() == Permission::Missing {
        platform.request_permission();
        platform.open_permission_settings()?;
    }
    Ok(status_of(platform))
}

#[tauri::command]
fn status(platform: State<'_, PlatformState>) -> Status {
    status_of(platform.as_ref())
}

#[tauri::command]
fn allow_access(platform: State<'_, PlatformState>) -> Result<Status, String> {
    allow_access_on(platform.as_ref())
}

#[tauri::command]
fn open_permission_settings(platform: State<'_, PlatformState>) -> Result<(), String> {
    platform.open_permission_settings()
}

fn show_board(app: &AppHandle) {
    if let Some(board) = app.get_webview_window(BOARD) {
        let _ = board.unminimize();
        let _ = board.show();
        let _ = board.set_focus();
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open MingDesk", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit MingDesk", true, Some("CmdOrCtrl+Q"))?;
    let menu = Menu::with_items(app, &[&open, &PredefinedMenuItem::separator(app)?, &quit])?;
    TrayIconBuilder::with_id("mingdesk")
        .icon(tauri::image::Image::from_bytes(include_bytes!("../icons/tray-template.png"))?)
        .icon_as_template(true)
        .tooltip("MingDesk")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_board(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage::<PlatformState>(platform::current())
        .invoke_handler(tauri::generate_handler![status, allow_access, open_permission_settings])
        .setup(|app| {
            // Menu bar only: no Dock icon, no app menu (PLAN.md §7, decided 2026-11-08).
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            build_tray(app.handle())?;
            // The board opens by itself only while there's something to do in it: allowing access.
            if !app.state::<PlatformState>().permission().usable() {
                show_board(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the board hides it; MingDesk keeps running in the menu bar until Quit.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == BOARD {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running MingDesk");
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform::fake::FakePlatform;

    #[test]
    fn allow_access_asks_the_os_and_opens_settings_when_missing() {
        let p = FakePlatform::new(Permission::Missing);
        let status = allow_access_on(&p).unwrap();
        assert_eq!(status.permission, Permission::Missing);
        assert_eq!(*p.requests.lock().unwrap(), 1);
        assert_eq!(*p.settings_opened.lock().unwrap(), 1);
    }

    #[test]
    fn allow_access_does_nothing_once_granted_or_not_needed() {
        for permission in [Permission::Granted, Permission::NotNeeded] {
            let p = FakePlatform::new(permission);
            assert_eq!(allow_access_on(&p).unwrap().permission, permission);
            assert_eq!(*p.requests.lock().unwrap(), 0);
            assert_eq!(*p.settings_opened.lock().unwrap(), 0);
        }
    }

    #[test]
    fn status_reports_the_grant_after_the_user_ticks_mingdesk() {
        let p = FakePlatform::new(Permission::Missing);
        assert!(!status_of(&p).permission.usable());
        *p.permission.lock().unwrap() = Permission::Granted;
        assert!(status_of(&p).permission.usable());
    }

    #[test]
    fn permission_serializes_as_the_board_expects() {
        assert_eq!(serde_json::to_string(&Permission::NotNeeded).unwrap(), "\"notNeeded\"");
        let json = serde_json::to_value(status_of(&FakePlatform::new(Permission::Granted))).unwrap();
        assert_eq!(json["permission"], "granted");
        assert_eq!(json["platform"], "fake");
    }

    #[test]
    fn this_machine_reports_a_permission_state() {
        // Real platform, no prompt: just proves the OS call links and returns.
        let _ = platform::current().permission();
    }
}

//! MingDesk's core: the menu-bar icon, the board window and the commands the board calls.

mod desk;
mod platform;

use platform::{Permission, Platform};
use serde::Serialize;
use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, State, WindowEvent};

const BOARD: &str = "main";

type PlatformState = Box<dyn Platform>;
/// Displays and setups seen so far (PLAN.md §9). In memory for now; saved to disk with the live updates.
type KnownState = Mutex<desk::Known>;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Status {
    permission: Permission,
    platform: &'static str,
    version: &'static str,
}

fn status_of(platform: &dyn Platform) -> Status {
    Status {
        permission: platform.permission(),
        platform: platform.name(),
        version: env!("CARGO_PKG_VERSION"),
    }
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

/// The desk as it is now: displays (ghosts included), their windows, and off-screen windows.
fn desk_of(platform: &dyn Platform, known: &KnownState) -> Result<desk::Desk, String> {
    if !platform.permission().usable() {
        return Ok(desk::Desk::default());
    }
    let snapshot = platform.snapshot()?;
    let mut known = known.lock().map_err(|e| e.to_string())?;
    known.remember(&snapshot.displays);
    Ok(desk::build(&snapshot, &known))
}

#[tauri::command]
fn desk(
    platform: State<'_, PlatformState>,
    known: State<'_, KnownState>,
) -> Result<desk::Desk, String> {
    desk_of(platform.as_ref(), &known)
}

/// The switcher's search over the desk as it is now: window ids, best match first.
#[tauri::command]
fn search(
    platform: State<'_, PlatformState>,
    known: State<'_, KnownState>,
    query: String,
) -> Result<Vec<u32>, String> {
    Ok(desk::search(&desk_of(platform.as_ref(), &known)?, &query))
}

#[tauri::command]
fn act(platform: State<'_, PlatformState>, action: desk::Action) -> Result<(), String> {
    desk::perform(platform.as_ref(), &action)
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
        .icon(tauri::image::Image::from_bytes(include_bytes!(
            "../icons/tray-template.png"
        ))?)
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
        .manage::<KnownState>(Mutex::new(desk::Known::default()))
        .invoke_handler(tauri::generate_handler![
            status,
            allow_access,
            open_permission_settings,
            desk,
            search,
            act
        ])
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
            if let WindowEvent::CloseRequested { api, .. } = event
                && window.label() == BOARD
            {
                api.prevent_close();
                let _ = window.hide();
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
        assert_eq!(
            serde_json::to_string(&Permission::NotNeeded).unwrap(),
            "\"notNeeded\""
        );
        let json =
            serde_json::to_value(status_of(&FakePlatform::new(Permission::Granted))).unwrap();
        assert_eq!(json["permission"], "granted");
        assert_eq!(json["platform"], "fake");
    }

    #[test]
    fn the_desk_is_empty_until_access_is_allowed_and_remembers_displays_after() {
        use crate::platform::Rect;
        use platform::fake::display;
        let p = FakePlatform::with(
            vec![display("laptop", Rect::new(0.0, 0.0, 1512.0, 982.0), true)],
            vec![],
        );
        let known = KnownState::default();
        *p.permission.lock().unwrap() = Permission::Missing;
        assert!(desk_of(&p, &known).unwrap().displays.is_empty());
        assert!(
            known.lock().unwrap().displays.is_empty(),
            "nothing read without access"
        );
        *p.permission.lock().unwrap() = Permission::Granted;
        assert_eq!(desk_of(&p, &known).unwrap().displays.len(), 1);
        assert_eq!(known.lock().unwrap().displays.len(), 1);
    }

    #[test]
    fn this_machine_reports_a_permission_state() {
        // Real platform, no prompt: just proves the OS call links and returns.
        let _ = platform::current().permission();
    }
}

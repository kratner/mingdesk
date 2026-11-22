//! MingDesk's core: the menu-bar icon, the board window and the commands the board calls.

mod desk;
mod live;
mod log;
#[cfg(target_os = "macos")]
mod mac_app;
mod platform;

use platform::{Change, Permission, Platform};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};

const BOARD: &str = "main";

type PlatformState = Box<dyn Platform>;
/// Displays and setups seen so far (PLAN.md §9), saved to known.json in MingDesk's settings folder.
type KnownState = Mutex<desk::Known>;
/// Nudges the refresh thread, e.g. when the board is shown.
type NudgeState = Mutex<Option<Sender<Change>>>;

/// The event the board listens to for a changed desk.
const DESK_EVENT: &str = "desk";

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

/// "Export desk": the desk as it is now and the Mac it came from, saved as JSON in Downloads and shown in
/// Finder. For trying board designs on a real desk in the browser preview (`?snapshot=`, src/desk/api.ts).
/// The file holds real window titles: keep it out of the repo (`snapshots/` is git-ignored).
#[tauri::command]
fn export_desk(
    platform: State<'_, PlatformState>,
    known: State<'_, KnownState>,
    stamp: String,
) -> Result<String, String> {
    let desk = desk_of(platform.as_ref(), &known)?;
    let file = serde_json::json!({
        "kind": "mingdesk-snapshot",
        "format": 1,
        "exported": stamp,
        "version": env!("CARGO_PKG_VERSION"),
        "platform": platform.name(),
        "os": command_output("sw_vers", &["-productVersion"]),
        "model": command_output("sysctl", &["-n", "hw.model"]),
        "desk": desk,
    });
    let home = std::env::var_os("HOME").ok_or("No home folder to save into.")?;
    let name: String = stamp
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' })
        .collect();
    let path = PathBuf::from(home)
        .join("Downloads")
        .join(format!("MingDesk desk {name}.json"));
    let text = serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("Couldn't save {}: {e}", path.display()))?;
    log::event(&format!("exported the desk to {}", path.display()));
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg("-R").arg(&path).spawn();
    Ok(path.display().to_string())
}

/// A short command's trimmed output (for the export's OS version and Mac model), or None.
fn command_output(program: &str, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new(program).args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
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
fn act(
    app: AppHandle,
    platform: State<'_, PlatformState>,
    action: desk::Action,
) -> Result<(), String> {
    use desk::Action::*;
    // MingDesk's own board is listed too; act on it through Tauri, not Accessibility.
    let own = std::process::id() as i32;
    let on_own = match &action {
        BringAppForward { pid } => *pid == own,
        Focus { window } | Minimize { window } | Close { window } => platform
            .snapshot()
            .is_ok_and(|s| s.windows.iter().any(|w| w.id == *window && w.pid == own)),
    };
    if !on_own {
        return desk::perform(platform.as_ref(), &action);
    }
    let board = app
        .get_webview_window(BOARD)
        .ok_or("The board is missing.")?;
    match action {
        Focus { .. } | BringAppForward { .. } => show_board(&app),
        Minimize { .. } => board.minimize().map_err(|e| e.to_string())?,
        // Closing the board hides it, as its own close button does.
        Close { .. } => board.hide().map_err(|e| e.to_string())?,
    }
    Ok(())
}

/// Saves the remembered displays and setups after the board changed them, and refreshes the desk.
fn after_setup_change(app: &AppHandle, known: &desk::Known) -> Result<(), String> {
    if let Some(path) = known_path(app) {
        live::save(&path, known)?;
    }
    if let Some(tx) = app.state::<NudgeState>().lock().unwrap().as_ref() {
        let _ = tx.send(Change::Displays);
    }
    Ok(())
}

/// "This is a setup of its own": the displays connected now become a setup, and their ghosts go.
#[tauri::command]
fn adopt_setup(
    app: AppHandle,
    platform: State<'_, PlatformState>,
    known: State<'_, KnownState>,
) -> Result<(), String> {
    let displays = platform.snapshot()?.displays;
    let mut known = known.lock().map_err(|e| e.to_string())?;
    if known.adopt(&displays) {
        after_setup_change(&app, &known)?;
    }
    Ok(())
}

#[tauri::command]
fn rename_setup(
    app: AppHandle,
    known: State<'_, KnownState>,
    id: String,
    name: String,
) -> Result<(), String> {
    let mut known = known.lock().map_err(|e| e.to_string())?;
    if known.rename_setup(&id, &name) {
        after_setup_change(&app, &known)?;
    }
    Ok(())
}

#[tauri::command]
fn forget_setup(app: AppHandle, known: State<'_, KnownState>, id: String) -> Result<(), String> {
    let mut known = known.lock().map_err(|e| e.to_string())?;
    if known.forget_setup(&id) {
        after_setup_change(&app, &known)?;
    }
    Ok(())
}

#[tauri::command]
fn open_permission_settings(platform: State<'_, PlatformState>) -> Result<(), String> {
    platform.open_permission_settings()
}

fn show_board(app: &AppHandle) {
    // Closing the board hides the app too; bring the app back before its window.
    #[cfg(target_os = "macos")]
    let _ = app.show();
    if let Some(board) = app.get_webview_window(BOARD) {
        let unmin = board.unminimize();
        let shown = board.show();
        let focused = board.set_focus();
        log::event(&format!(
            "show_board unminimize={unmin:?} show={shown:?} focus={focused:?}"
        ));
    } else {
        log::event("show_board: no board window");
    }
    // Windows may have changed while it was hidden.
    if let Some(tx) = app.state::<NudgeState>().lock().unwrap().as_ref() {
        let _ = tx.send(Change::Windows);
    }
}

fn known_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join("known.json"))
}

fn showing(app: &AppHandle) -> bool {
    app.get_webview_window(BOARD)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false)
}

/// The refresh thread (live.rs has the rules): waits for a change or the backstop timer, lets a burst settle,
/// then takes a snapshot if one is wanted, saves remembered displays and tells the board.
fn start_live(app: &AppHandle) {
    let (tx, rx) = mpsc::channel::<Change>();
    let sink_tx = Mutex::new(tx.clone());
    app.state::<PlatformState>().watch(Arc::new(move |change| {
        let _ = sink_tx.lock().unwrap().send(change);
    }));
    *app.state::<NudgeState>().lock().unwrap() = Some(tx);
    let app = app.clone();
    std::thread::Builder::new()
        .name("mingdesk-live".into())
        .spawn(move || {
            let mut state = live::Live::default();
            loop {
                let mut tick = live::Tick::default();
                let mut note = |c: Change| match c {
                    Change::Windows => tick.windows = true,
                    Change::Displays => tick.displays = true,
                };
                match rx.recv_timeout(live::POLL) {
                    Ok(first) => {
                        note(first);
                        std::thread::sleep(live::SETTLE);
                        rx.try_iter().for_each(&mut note);
                    }
                    Err(RecvTimeoutError::Timeout) => tick.timer = true,
                    Err(RecvTimeoutError::Disconnected) => return,
                }
                tick.showing = showing(&app);
                let platform = app.state::<PlatformState>();
                let known_state = app.state::<KnownState>();
                let mut known = known_state.lock().unwrap();
                match state.step(tick, platform.as_ref(), &mut known) {
                    Ok(step) => {
                        if step.save_known
                            && let Some(path) = known_path(&app)
                            && let Err(e) = live::save(&path, &known)
                        {
                            eprintln!("MingDesk: couldn't save known displays: {e}");
                        }
                        drop(known);
                        if let Some(desk) = step.emit {
                            let _ = app.emit(DESK_EVENT, desk);
                        }
                    }
                    Err(e) => eprintln!("MingDesk: refresh failed: {e}"),
                }
            }
        })
        .expect("start the refresh thread");
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
            "open" => {
                log::event("menu-bar icon: Open MingDesk");
                show_board(app);
            }
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
        .manage::<NudgeState>(Mutex::new(None))
        .invoke_handler(tauri::generate_handler![
            status,
            allow_access,
            open_permission_settings,
            desk,
            search,
            act,
            adopt_setup,
            rename_setup,
            forget_setup,
            export_desk
        ])
        .setup(|app| {
            log::event(&format!("started, version {}", env!("CARGO_PKG_VERSION")));
            // A Dock app with the standard menus; the menu-bar icon stays too (PLAN.md §7.13, 2026-11-22).
            build_tray(app.handle())?;
            #[cfg(target_os = "macos")]
            {
                let handle = app.handle().clone();
                mac_app::on_activate(move || {
                    log::event("app became active");
                    show_board(&handle);
                });
            }
            if let Some(path) = known_path(app.handle()) {
                *app.state::<KnownState>().lock().unwrap() = live::load(&path);
            }
            start_live(app.handle());
            // Launching MingDesk opens the board, as any Mac app opens its window.
            show_board(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the board hides it; MingDesk keeps running (Dock, menu bar) until Quit.
            if let WindowEvent::CloseRequested { api, .. } = event
                && window.label() == BOARD
            {
                api.prevent_close();
                let _ = window.hide();
                // Also step back from being the active app, as ⌘H does: you land in the app you were using, and
                // ⌘-Tab to MingDesk is then a real switch, which opens the board (Keith's check 2026-11-22: with
                // MingDesk left active and windowless, ⌘-Tab to it did nothing).
                #[cfg(target_os = "macos")]
                let _ = window.app_handle().hide();
                log::event("board closed (board hidden, app hidden)");
            }
        })
        // About, Hide, Quit (⌘Q), Edit (copy and paste in the board's fields), Window. Settings… joins with the
        // hotkey setting (M1 step 5).
        .menu(tauri::menu::Menu::default)
        .build(tauri::generate_context!())
        .expect("error while starting MingDesk")
        .run(|app, event| {
            // A click on the Dock icon shows the board, whether or not it was open.
            if let tauri::RunEvent::Reopen { .. } = event {
                log::event("dock icon clicked (reopen)");
                show_board(app);
            }
        });
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

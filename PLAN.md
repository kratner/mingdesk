# MingDesk — Plan

A window manager in the MingTab mould: every app window on your computer on one board, saved into
collections and spaces, searched, and brought back the way it was. Mac first; Windows and Linux after.

Created 2026-11-08. **M0 built 2026-11-08** (see CHANGELOG.md); M1 next.

**Name:** MingDesk, from 明窗淨几 (*míng chuāng jìng jī*), "a bright window and a clean desk": the classical
picture of a tidy place to work. MingTab keeps the tab bar clear; MingDesk keeps the desk clear. A Four Ounces
tool (four-ounces-brand repo: tokens, type, product icon).

---

## 1. What it is

MingTab treats tabs as cards you can save, sort and reopen. MingDesk does the same one level up, for **windows**
of any app, and keeps tabs as a level beneath them where an app has tabs.

| MingTab | MingDesk |
|---|---|
| Open tabs pane (every window and tab group) | **Open windows pane**: every window, grouped by app and by display |
| Card = a saved tab (URL, title, note) | Card = a saved **window** (app, title, document or folder, size and place, display, tabs if any) |
| Collection = saved tabs, e.g. a tab group | Collection = a **working set** of windows, e.g. "Quarterly report": Excel file, two Finder folders, a browser window, a Terminal |
| Save window / Save all | **Save these windows** / **Save desk** (everything open, on every display) |
| Open a collection: tabs come back | **Open a collection**: apps launch, documents reopen, windows go back to their size and place |
| Search saved + open tabs, `/` | Search saved + open windows; a **global hotkey** from any app jumps to a window |
| Spaces, tags, stars, notes, Next, Stats | Same ideas, same look |

**Where it stands among existing Mac tools.** Window tools on the Mac mostly do one of three things: arrange
(Rectangle, Moom, Magnet), switch (AltTab, Witch, Raycast), or restore a fixed layout (Stay, Workspaces).
MingDesk's difference is MingTab's: a **library** of saved working sets you organise, search and reopen,
alongside a live view of what's open. Arranging comes later (M4) and stays simple.

**Tabs as a subset.** Browsers, Finder, Terminal, VS Code and others have tabs. MingDesk shows tabs where the app
tells it about them (section 3), and for Chromium browsers it borrows them from MingTab rather than
reimplementing tab saving.

---

## 2. Platforms: what each one allows

Operating systems don't hand out a tidy list of windows the way a browser hands an extension its tabs. What's
possible differs a lot by platform, and it decides the order of work.

### macOS (first)

- **See and control windows:** the Accessibility API (`AXUIElement`) lists each app's windows with title,
  position, size, minimized and full-screen state, and the document they show (`AXDocument`, a file URL, for most
  document apps). It can move, resize, raise, focus, minimize and close them. `CGWindowListCopyWindowInfo` adds
  every window's on-screen order and display without needing a permission per app.
- **Permission:** the user grants **Accessibility** once (System Settings → Privacy & Security). MingDesk checks
  with `AXIsProcessTrusted` and walks the user there on first run. Without it, MingDesk can list windows but not
  move them or read titles reliably. **Screen Recording** is not needed unless we add window thumbnails (later,
  optional).
- **Reopen:** launch the app (`NSWorkspace`), open its documents (`open` with the file), wait for windows, match
  each to the saved card (document first, then title, then order) and put it back.
- **Limits, stated plainly:**
  - **App state isn't restorable.** MingDesk reopens the document and the window, not unsaved edits, scroll
    position, a Terminal's history or an app's internal panels. Apps that restore their own windows on launch
    (most Apple apps) help.
  - **Mission Control Spaces (desktops):** there's no public API to move a window to another desktop or switch
    desktops. MingDesk records which display a window was on, not which desktop. Workarounds (simulated
    keystrokes) are fragile; left out unless asked.
  - **Full-screen windows** live on their own desktop and can be listed but not placed.
  - **Not in the Mac App Store:** the store's sandbox forbids controlling other apps' windows. Distribution is a
    signed, notarized download (section 5).

### Windows (second)

- Win32 (`EnumWindows`, `GetWindowText`, `SetWindowPlacement`, `ShowWindow`) gives everything MingDesk needs to
  list, place, focus, minimize and close top-level windows; no permission prompt. UI Automation exposes tabs in
  some apps (Explorer, Windows Terminal, Edge).
- Documents: the process's command line and the window title usually identify them; less reliable than the Mac's
  `AXDocument`.
- Virtual desktops: Windows only lets an app move its own windows between desktops through public API, so
  MingDesk records the display and leaves desktops alone, as on the Mac.

### Linux (third)

- **X11:** full control through EWMH (what `wmctrl` and `xdotool` use). Straightforward.
- **Wayland** (now the default on GNOME and KDE): by design, apps can't list or move other apps' windows. Each
  desktop needs its own route: a KWin script on KDE, a GNOME Shell extension, the sway or Hyprland IPC. MingDesk
  supports X11 first, then the desktops people actually ask for.

---

## 3. Tabs inside apps

| App | How MingDesk gets its tabs | When |
|---|---|---|
| Chrome, Edge, Brave, Vivaldi | From **MingTab**, over Chrome's native messaging: MingTab reports each window's tabs and can save or reopen them. MingDesk shows a browser window card with its tabs and hands browser restores to MingTab. | M5 |
| Safari | AppleScript (`tell application "Safari"`: windows, tabs, URLs); needs the user's Automation permission per app | M5 |
| Finder | AppleScript: each window's folder and its tabs | M5 |
| Terminal, iTerm2 | AppleScript: windows, tabs, working directories (not history) | M5 |
| VS Code, Cursor | The window title carries the workspace; reopen with `code <folder>` | M5 |
| Everything else | The window only | — |

---

## 4. How it's built

- **Tauri 2:** a small native app whose screens are a web page.
  - **UI:** React + TypeScript, styled with the Four Ounces tokens, laid out like MingTab's board (spaces, the
    collections grid, the open-windows pane). The look and the habits carry over.
  - **Core:** Rust. A `Platform` trait (`list_windows`, `focus`, `place`, `minimize`, `close`, `launch`,
    `open_document`, `watch` for changes) with one implementation per OS: `macos` first (Accessibility through
    the `objc2` and `core-foundation` crates), then `windows` (the `windows` crate), then `linux`.
  - **Store:** SQLite in the Rust core (`rusqlite`), so saving works from the menu bar and the hotkey without the
    board open. Same shape as MingTab's data: spaces → collections → cards, with UUIDs and `updatedAt`/`deletedAt`
    so sync can come later without a migration.
- **Always there, not in the way:** a menu-bar icon (tray on Windows/Linux) with Save desk, recent collections
  and Open board; the board is a normal window; a global hotkey (default ⌥Space, changeable) opens a quick
  switcher over everything.
- **Starts at login** (optional), **updates itself** from GitHub Releases (Tauri's updater, signed).
- **Card data** (a saved window): app id (bundle id / exe path), app name, title, document or folder, frame
  (x, y, w, h), display (id plus size, so a restore on a different monitor setup can scale or fall back to the
  main display), minimized/full-screen flags, tabs (optional list), note, tags.

---

## 5. Distribution and what it costs

- **Mac:** a signed and notarized `.dmg`. Signing needs the **Apple Developer Program, $99/year**, on Keith's
  personal Apple ID. Two reasons it's worth it before anyone else installs it: unsigned apps meet Gatekeeper
  warnings, and macOS ties the Accessibility permission to the app's signature, so an unsigned build loses the
  permission on every update. Development builds can run unsigned on Keith's Mac meanwhile.
- **Windows:** an installer from GitHub Releases; code signing optional at first (SmartScreen warns until it's
  signed or has reputation).
- **Linux:** AppImage / `.deb`.
- Repo: <https://github.com/kratner/mingdesk> (pushed over SSH; this machine's `gh` is the work account).

---

## 6. Milestones

Each milestone ends with tests that ran and what Keith checks by hand, as in MingTab.

| | Milestone | Done when |
|---|---|---|
| **M0** | **Skeleton.** Tauri app, menu-bar icon, empty board, Four Ounces icon. Accessibility onboarding: detects the permission, opens the right Settings pane, notices when it's granted. | Builds and runs on the Mac; onboarding round-trip checked by hand. |
| **M1** | **See your desk.** Open-windows pane: every window grouped by app and display, kept live. Click to focus; minimize, close, bring all of an app's windows forward. Quick switcher on the global hotkey with search. | Rust tests over a fake platform; on the real Mac, a script opens TextEdit/Preview/Finder windows and checks MingDesk lists, focuses and closes them. |
| **M2** | **Save and restore.** Save a window, an app's windows, a selection, or the whole desk as a collection. Open a collection: launch apps, reopen documents, place windows; report anything that couldn't come back. Cope with a changed monitor setup. | Restore test: save a scripted set of documents in TextEdit, Preview and Finder, quit them, open the collection, compare frames (±2 px) and documents. Two-display and one-display restores by hand. |
| **M3** | **Organise.** Spaces, drag and drop, tags, stars, notes, Next, Stats, select mode, undo, backup/restore to a file. Mostly MingTab's board, ported. | Unit tests on the model; UI tests in the webview (Playwright against the dev server, as MingTab does). |
| **M4** | **Arrange.** Snap the focused window to halves, thirds and quarters from the hotkey; save and reapply a layout per display setup (docked vs. laptop only). Kept modest: Rectangle does this well. | Frame math unit-tested per display layout; by hand on the laptop alone and docked. |
| **M5** | **Tabs.** MingTab bridge (native messaging: list a browser window's tabs, save them with the window, hand restores to MingTab); Safari, Finder and Terminal tabs through AppleScript; VS Code workspaces. | Bridge protocol unit-tested on both sides; restore of a browser window with its tabs by hand. |
| **M6** | **Windows.** The `windows` platform implementation, tray icon, installer. | The M1/M2 scripted tests, ported to Notepad/Explorer windows. |
| **M7** | **Linux.** X11 first; then KDE, GNOME, sway or Hyprland as asked. | Same tests on an X11 session. |
| later | Sync (MingTab's Cloudflare Worker gains a MingDesk table set), window thumbnails (Screen Recording permission), per-app restore helpers. | |

---

## 7. Decisions

Decided 2026-11-08:

1. **Apple Developer Program ($99/yr): later.** Builds are signed with a free self-signed certificate
   (`scripts/dev-cert.sh`), which keeps the Accessibility permission across builds on Keith's machines. Enrol
   before MingDesk is shared with the public (individual = "Keith Ratner"; organisation = "Four Ounces", which
   needs a D-U-N-S number first).
2. **Menu bar only**, no Dock icon; the board opens as a normal window. Built in M0.
3. **Hotkey ⌥Space** by default (M1); MingDesk detects a clash and asks.
4. **Separate app** that talks to MingTab, not a merged product.
5. **Licence: MIT**, the same as MingTab. Repo public.

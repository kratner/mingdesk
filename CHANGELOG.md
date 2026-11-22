# Changelog

## 0.2.1 (2026-11-22): M1 preview

The first download. (0.2.0 was tagged but never published: the release workflow's final check failed
without saying why. It now names any check that fails.) MingDesk now sees your desk; the ⌥Space switcher, which completes M1, comes next.

- **The desk, live.** Every window on every display and Mission Control desktop, including full-screen spaces,
  kept up to date as windows open, close, move and change, apps come and go, you switch desktops or plug in
  displays.
- **List view:** one section per display (side by side on a wide board): the desktop showing now, other desktops
  and full-screen spaces by name, minimized windows; or grouped by app, with Bring forward. Filter box, density
  slider (Compact / Normal / Large), folding groups. Click to focus; minimize and close on hover.
- **Desk view:** your displays to scale with their windows in stacking order; pinch to zoom, two-finger scroll
  to pan, double-click a display to zoom to it. Windows on other desktops, minimized or hidden behind others are
  chips on each display, collapsing into a scrollable list when there are many.
- **Display setups** (e.g. Home and Office): remembered, renamed, forgotten; a display missing from the current
  setup shows as a ghost.
- **A Dock app:** the Dock icon and ⌘-Tab open the board; standard menus with Quit; the menu-bar icon stays.
- Universal (Apple silicon and Intel), macOS 10.14 Mojave or later; signed with the "MingDesk Dev" certificate.

## 0.1.0 (2026-11-08): M0, skeleton

- Tauri 2 app (Rust core, React board) with the Four Ounces tokens, fonts and a new product icon (two overlapping
  windows on plum).
- Menu bar only: menu-bar icon with Open MingDesk and Quit; no Dock icon; closing the board keeps MingDesk running.
- Accessibility onboarding on the Mac: the board opens by itself while the permission is missing, **Allow access**
  lists MingDesk and opens System Settings at Accessibility, and the board notices the grant without a restart.
- `Platform` trait (PLAN.md §4) with a macOS implementation and a fake for tests; Windows and Linux report that no
  permission is needed.
- Free self-signed "MingDesk Dev" signing (`scripts/dev-cert.sh`, `npm run app`), so the permission survives rebuilds.
- MIT licence.

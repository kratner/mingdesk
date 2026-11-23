# Changelog

## 0.2.6 (2026-11-22): M1 preview

- Desk view: a Zoom slider in the toolbar. Zoomed in, the desk scrolls, with scrollbars, the wheel or two fingers.
- Pinch to zoom works on macOS Mojave too (its WebKit sends Safari's own pinch events).
- Slim, quiet scrollbars everywhere, gold while you hold one.
- The Desk canvas fits a small window (the 2012 MacBook Air's) without the page scrolling.

## 0.2.5 (2026-11-22): M1 preview

- A window titled after its app shows its name once ("Keychain Access", not "Keychain Access: Keychain Access").
- The setup line lists your displays left to right, the way they sit on the desk.

## 0.2.4 (2026-11-22): M1 preview

- On Mojave, the Density and Windows labels no longer touch their sliders.

## 0.2.3 (2026-11-22): M1 preview

- **Tidier on Mojave.** Mojave's WebKit has no spacing between items in a row, so labels ran together
  ("ApplicationsFinder", "Display 12560 × 1440"). The board now adds that spacing itself where it's missing.
  The List view's toolbar no longer lands on the setup bar when the list is long. The Density and Windows
  sliders have a visible track.
- A setup named after its only display no longer says the name twice ("Display 1 Display 1").
- With one display, the List view uses the full width.
- **Export desk** (in the footer) saves every open window and where it sits, with the macOS version and Mac
  model, as a JSON file in Downloads. It's for trying board designs against a real desk: put the file in
  `snapshots/` and open the browser preview with `?snapshot=<file>`. The file holds real window titles, so
  `snapshots/` stays out of git.

## 0.2.2 (2026-11-22): M1 preview

- **Runs on macOS Mojave again.** On a Mojave Mac with its original Safari (12), the board's window stayed
  black: the board was built for Safari 14 and Safari 12 couldn't read it. It's now built for Safari 12, and
  the two newer browser features it used (watching the canvas size, copying the sample desk) have fallbacks.
- If the board ever can't start, its window now says why instead of staying blank.
- **Desk view, easier to read.** Windows are drawn as outlines, so the ones behind show through. A window's
  title appears only where its top-left corner is clear, so titles no longer pile up. Hovering names the window
  under the pointer and outlines it; clicking a spot lists every window stacked there, front to back, then the
  rest of that display, and hovering a row outlines that window. A maximized window's outline no longer loses
  its right and bottom edges.
- **Windows slider** in the toolbar (Desk view): from outlines, which show where every window sits, to solid,
  which shows only what's in front, as before. Remembered on this Mac. The canvas's hint and Fit all now sit on
  the canvas itself, Fit all only once you've zoomed in.

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

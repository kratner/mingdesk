# Changelog

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

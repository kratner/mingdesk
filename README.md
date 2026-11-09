# MingDesk

Every app window on your computer on one board: save working sets of windows into collections, search them,
and bring them back the way they were. A Four Ounces tool, from the maker of MingTab. Mac first.

**Status: M0, the skeleton.** MingDesk runs in the menu bar, opens an empty board and walks you through the Mac's
Accessibility permission. Seeing your windows comes next (M1). The plan and milestones: [PLAN.md](PLAN.md);
what changed: [CHANGELOG.md](CHANGELOG.md).

## Build it

MingDesk is a [Tauri 2](https://tauri.app) app: a Rust core with a React board.

### Mac

1. Install the Xcode command-line tools (`xcode-select --install`), [Node.js](https://nodejs.org) and Rust
   (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`).
2. `npm install`
3. Once per Mac: `sh scripts/dev-cert.sh`. It adds a free, self-signed "MingDesk Dev" certificate to your login
   keychain, so every build has the same signature and macOS keeps the Accessibility permission between builds.
   The first build after it may ask to use the key from your keychain: enter your login password and click
   **Always Allow**.
4. `npm run app -- --open` builds `MingDesk.app` (in `src-tauri/target/release/bundle/macos/`) and opens it.
   MingDesk appears in the menu bar, not the Dock.

`npm run tauri dev` gives a live-reloading board for UI work, but check the Accessibility steps with the built
app: under `tauri dev` macOS charges the permission to the app that launched it (VS Code or Terminal).

### Windows

1. Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with
   **Desktop development with C++**, then Rust from [rustup.rs](https://rustup.rs), [Node.js](https://nodejs.org)
   and Git. WebView2 is already part of Windows 10 and 11.
2. `npm install`, then `npx tauri build --bundles nsis` (installer) or `npm run tauri dev`.

Windows needs no permission for window control; MingDesk's Windows window code is milestone M6.

### Tests

`npm test` runs the Rust tests (`cargo test`); `npm run build` type-checks and builds the board.

### Icons

`npm run icons` rebuilds every icon from `branding/mingdesk.svg`, copied from the Four Ounces kit
(kratner/four-ounces-brand, `logo/products/mingdesk.svg`). Change the glyph in the kit first, then copy it here.

## Licence

MIT: see [LICENSE](LICENSE). The bundled fonts keep their own OFL licences (`public/fonts/licenses/`).

---

<p align="center">
  <a href="https://fourounces.live/">A Four Ounces tool</a> · by Keith Ratner
</p>

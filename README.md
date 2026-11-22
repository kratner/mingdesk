# MingDesk

Every app window on your computer on one board: save working sets of windows into collections, search them,
and bring them back the way they were. A Four Ounces tool, from the maker of MingTab. Mac first.

**Status: M1 preview.** MingDesk shows every window on every display and desktop, live, as a list or as a picture
of your screens, and focuses, minimizes or closes them. The ⌥Space switcher finishes M1; saving and restoring
windows is M3. The plan and milestones: [PLAN.md](PLAN.md); what changed: [CHANGELOG.md](CHANGELOG.md).

## Download

Get the latest **MingDesk-…-universal.dmg** from [Releases](https://github.com/kratner/mingdesk/releases): one
download for Apple silicon and Intel Macs, macOS 10.14 Mojave or later. Each release's notes say how to install
it. MingDesk is signed with Four Ounces' own certificate rather than through Apple's paid programme, so the first
time you open a new version macOS asks you to allow it (**Open Anyway** in System Settings → Privacy & Security,
or Control-click → Open on macOS 14 and earlier).

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
   MingDesk opens its board, with an icon in the Dock and one in the menu bar.

`npm run tauri dev` gives a live-reloading board for UI work, but check the Accessibility steps with the built
app: under `tauri dev` macOS charges the permission to the app that launched it (VS Code or Terminal).

### Windows

1. Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with
   **Desktop development with C++**, then Rust from [rustup.rs](https://rustup.rs), [Node.js](https://nodejs.org)
   and Git. WebView2 is already part of Windows 10 and 11.
2. `npm install`, then `npx tauri build --bundles nsis` (installer) or `npm run tauri dev`.

Windows needs no permission for window control; MingDesk's Windows window code is milestone M6.

### Releases

Bump the version in `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`, give it a section in
CHANGELOG.md ("preview" in its heading makes a pre-release), commit, then tag `vX.Y.Z` and push the tag. The
Release workflow (`.github/workflows/release.yml`) tests, builds the universal app, signs it with the "MingDesk Dev"
certificate, makes the .dmg and publishes the release. Once per repo, give the workflow the certificate:
`scripts/set-release-secrets.sh` says how.

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

# Regie

[![CI](https://github.com/jdtobe/rake/actions/workflows/ci.yml/badge.svg)](https://github.com/jdtobe/rake/actions/workflows/ci.yml)

Regie is a macOS-first Quake-style drop-down terminal written in Rust. Press a hotkey and a terminal panel slides down from the top of the screen; press it again and it slides away. Previously named Rake; the GitHub repository has not yet been renamed.

## Status

**Early. Independent terminal tabs are implemented on macOS; native hands-on acceptance is pending.**

| Capability | State |
|---|---|
| Menubar icon with **About** and **Quit** | Working (macOS); template mark, tinted by the system |
| Left-click on the menubar icon | Toggles the panel; right-click opens the menu |
| **About** panel | Native panel with the colour app icon, version, copyright and credits |
| One running copy | A second launch brings the existing panel forward and exits |
| Global hotkey | Working |
| Drop-down window at the top of the screen | Working; opens on the display under the mouse and shows on every Space |
| Slide animation | Working; short hitch at the start of each slide in |
| Configuration | Hotkey, window size, hide on focus lost, animation time (config file only) |
| Settings menu in the menubar | Not started |
| Embedded terminal (shell + rendering) | Independent persistent login-shell sessions; active-tab WebGL with a DOM fallback |
| Terminal colours and font | Follows the macOS Terminal default profile; changes apply on restart |
| Tabs | Bottom/top strip, names, cwd inheritance, close/quit confirmation, optional layout restoration |
| Profiles | Later; terminal colours still follow the Terminal default profile |
| Linux / Windows | Not tested |

Developed and tested on macOS only.

## Install

Regie development builds use `Regie-v….dmg` containing **Regie.app**; previously published releases may still be named Rake. Open the image and drag the app onto **Applications**. The build is ad-hoc signed and not notarised, so the first launch needs Finder: right-click the app and choose **Open** (macOS remembers the choice). Releases are manual; there is no auto-update.

## Run it

```bash
cargo run
```

Regie opens and focuses a terminal backed by your default login shell. The panel appears on the display under the mouse and stays there until hidden; the next show chooses again. The shell remains alive when the hotkey hides the window. An icon also appears in the menubar (no Dock icon): left-click toggles the panel, right-click opens **About** and **Quit**.

Only one copy runs, including an old Rake copy. Launching again, from Finder or the binary, brings the existing panel forward and exits the new process, so quit the running copy before `cargo run`.

## Tabs

Each tab keeps its own shell, screen and scrollback alive. Click **+**, or use
**File → New Tab**. Double-click a tab name to set a manual title; clear it to
return to automatic shell/program titles (directory name when no title is supplied).
Manual titles ignore shell title updates until cleared. Reordering is wanted
follow-up work, not implemented yet.

| Shortcut | Action |
|---|---|
| Cmd+T | New tab in the current tab's local directory |
| Cmd+W | Close tab |
| Cmd+1–8 | Select numbered tab |
| Cmd+9 | Select last tab |
| Ctrl+Tab / Ctrl+Shift+Tab | Next / previous tab |
| Cmd+Q | Quit Regie |

Closing a tab or quitting asks before terminating running apps, including
background/stopped jobs; uncertain process inspection also asks. Cancellation
keeps sessions alive. In-app menu/keyboard quit routes share this check, but
macOS logout/shutdown, Force Quit and external signals cannot guarantee a prompt.
Cleanup targets the local session tree, not deliberately detached/reparented daemons.

Startup defaults to one new tab. Opt-in restoration recreates tab order, names,
selection and local directories with **fresh shells**, never SSH reconnections,
running commands or scrollback. Regie does not restore command history; the shell's
normal history files still work. Saved layout lives in
`~/Library/Application Support/Regie/tabs.json`. Missing directories fall back to
home with a notice.

## Configure

Optional. Create `~/.config/regie/config.toml`; if absent, Regie reads the old `~/.config/rake/config.toml` with a notice. No files are moved or overwritten. If neither exists, defaults apply. Changes need a restart. A bad canonical file stops the app rather than silently falling back.

```toml
# Global key that shows/hides the terminal. Default: "Ctrl+`"
hotkey = "Alt+Space"

# Window height as a percentage of the screen, 10 to 100. Default: 40
height_percent = 40

# Window width as a percentage of the screen, 10 to 100. The window is centered. Default: 100
width_percent = 100

# Hide the window when it loses keyboard focus. Default: false
hide_on_focus_lost = false

# Slide in/out time in milliseconds, 0 to 2000. 0 turns the animation off. Default: 300
animation_ms = 300

# Tab strip placement: bottom (default) or top.
tab_position = "bottom"
# New shell directory: current (default) or home. SSH remains local.
new_tab_directory = "current"
# Startup: new (default) or restore the previous layout with fresh shells.
startup_tabs = "new"
# Last tab closed: new (default), hide, or quit. Hidden-empty summons create a tab.
last_tab_action = "new"
# Regie controls only: system (default), light or dark. Terminal profile is separate.
appearance = "system"
```

The panel opens at the configured size and cannot be resized by dragging.

Hotkey strings use the [`global-hotkey`](https://docs.rs/global-hotkey) format, such as `Ctrl+Shift+K`, `Cmd+Backquote` or `F12`. Note that a global hotkey is captured system-wide, so the default `Ctrl+\`` will also stop working inside other apps that use it, such as editors.

## Develop

```bash
cargo test      # unit tests
cargo clippy --all-targets
cargo fmt
```

Requires a stable Rust toolchain.

Browser tests require Node 22 and the pinned Playwright WebKit build:

```bash
npm ci
npx playwright install webkit
npm test
```

See [tabs acceptance checks](docs/testing/tabs.md) for automated evidence and
remaining native hand checks. These tools are development-only, not shipped in the app.

Pull requests and pushes to **main** run the same shared checks (feature-branch
pushes do not also trigger duplicate runs): formatting, strict Clippy, all-target
Rust tests, doc tests, real WebKit tests and a built/mounted development DMG on
macOS; actionlint, ShellCheck and Python script tests on Ubuntu. This does not
claim Linux support for Regie. Failed browser checks retain traces, screenshots,
videos and the HTML report as workflow artifacts for seven days.

Rust and npm dependency audits run with those checks and weekly on Monday at
08:17 UTC; **Dependency security → Run workflow** also runs them manually.
Actions are pinned to commit SHAs; grouped Dependabot updates cover Cargo, Actions
and npm. Audits fail on vulnerabilities; informational Rust advisories remain
visible warnings. Current warnings are `proc-macro-error` (unmaintained) and
`glib` (`VariantStrIter` unsoundness), both in the upstream Linux GTK dependency
tree, not the macOS build. No advisory is suppressed and no dependency upgrade
is implied by this workflow change.

### Manual releases

Releases never run on push, tag or merge. In **Actions → Release → Run workflow**,
select **main**. Leave **dry_run** checked (the default) to run full checks and
produce a verified DMG workflow artifact **without creating a tag or release**.
Uncheck it only when you intend to publish. Both modes reject other branches.

The version is `vYY.M.N`: UTC year/month without leading month zero, with build
`N` starting at zero each month and incrementing past the highest canonical tag
(October 2026 starts at `v26.10.0`, November at `v26.11.0`). The workflow stamps
only its checkout's Cargo package and root lockfile entry. Cargo and both bundle
version fields are numeric; About and release tags display the `v` prefix. Source
stays at the development version. Release runs are serialized; tags point to the
exact checked commit. A failed publication after tag creation can leave a tag;
the next run uses the next build number rather than overwriting it.

Only the final publish job receives a write token; it does not check out or execute
repository code. It publishes **one arm64 DMG**, with `Regie.app` and an Applications
symlink, and includes its SHA-256 in generated release notes. Checks and builds
have read-only tokens. Images are verified for checksum integrity, bundle versions,
executable architecture, icon, ad-hoc signature and Applications link before
upload; no signing/notarisation or automatic updates are added.

To build and verify a development image locally on Apple Silicon macOS:

```bash
cargo build --release --locked
scripts/package.sh                  # prints dist/Regie-v0.1.0-arm64.dmg
scripts/verify-package.sh dist/Regie-v0.1.0-arm64.dmg 0.1.0
python3 -m unittest discover -s tests/ci -v  # Python 3.11+; includes real DMG tests
```

`package.sh` accepts a **numeric** version, not a `v`-prefixed tag. Package tests
use a temporary directory so they do not replace your local preview. Analysis
tools can be installed without changing global tools using
`sh .github/scripts/install-analysis-tools.sh <scratch-directory>`; it prints the
directory to add to PATH.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Security problems should follow
[SECURITY.md](SECURITY.md) rather than be filed as public issues.

## Licence

[MIT](LICENSE).

## Documentation

- [Roadmap](docs/roadmap.md): goals, planned feature set and milestones
- [Design](docs/design.md): architecture and complications
- [Principles](docs/principles.md): how we work (TDD, ADRs, commits)
- [Open questions](docs/open-questions.md): decisions not yet made
- [Architecture decision records](docs/adr/): why things are the way they are
- [AGENTS.md](AGENTS.md): instructions for AI coding agents

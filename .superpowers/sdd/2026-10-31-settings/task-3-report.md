# Task 3 report — live runtime application

## Status

**Partial implementation committed.** The runtime primitives for workspace behavior and live terminal/UI updates are present, but Task 3 is not fully complete: the main-thread settings controller integration and hotkey replacement/rollback adapter are intentionally not included in this commit. Settings edits therefore do not yet drive these methods from the running application.

## Changes

- Added `Workspace::apply_config`, which replaces its active config, so subsequent workspace actions use new-tab/last-tab/startup-related values. It emits typed UI deliveries when interface appearance or tab-bar placement changes and does not recreate tabs or sessions.
- Added typed `UiDelivery` variants for appearance, tab-bar position, and terminal theme. The theme payload uses the existing `TerminalTheme::config_json()` wire representation.
- Added `DropDown::apply_config` to update configured panel dimensions and animation duration, resize/refit the visible webview, and deliver the theme and current appearance/tab-bar settings. In-flight slides are restarted from the current position toward the new endpoint.
- Added terminal JS handlers to update all live xterm instances' colors and font options in place and refresh/refit the active terminal, without clearing the buffer or recreating terminals. Appearance and tab-bar position update their live document state.
- Added Rust tests for subsequent last-tab behavior and typed UI delivery without tab replacement, plus a Playwright WebKit test exercising theme changes with multiple live sessions.

## Verification

- `cargo test -- --test-threads=1` — passed: 218 unit tests, 1 integration test, doc tests.
- `cargo clippy --all-targets -- -D warnings` — passed.
- `cargo fmt -- --check` — passed.
- `PLAYWRIGHT_BROWSERS_PATH=/private/var/folders/n6/kfcbdjld3_sfl70ndpl5j6jh0000gn/T/opencode/regie-playwright npm test` — passed: 25 WebKit tests.
- Two parallel `cargo test` attempts had unrelated existing tests fail because temp-path tests collided (`AlreadyExists` / a config write conflict). The serial full test run passed. Do not treat the initial parallel test output as a code failure attributable to these changes.

## Concerns / remaining work

- `SettingsController` effects are not wired into the application event loop in this task. In particular, `DropDown::apply_config` and `Workspace::apply_config` are not called when settings are applied; this is the later integration work requested by the parent task.
- No `replace_hotkey(manager, old, new)` adapter or fake-adapter rollback tests were added. Hotkey registration rejection, save failure, restoration failure reporting, and retained shortcut evidence remain unimplemented here.
- Native AppKit appearance changes still require main-thread application wiring; the current typed delivery updates the embedded workspace appearance state.
- Startup-mode and new-directory behavior are retained through the updated `Workspace` config, but dedicated new tests for these cases were not added.
- No ADR or user documentation update was included; those are still required before the settings feature is considered complete.

## Commit

`feat: settings - apply configuration without restarting sessions` (this report is included in that commit).

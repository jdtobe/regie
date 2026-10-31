# Task 3 report — live runtime application

## Status

**Runtime-adapter work committed; controller/event-loop integration remains intentionally deferred.** The runtime methods, testable hotkey transaction helper, native appearance adapter, and headless coverage described below are present. Settings edits still do not drive these methods from the running application until the later integration task.

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
- The event-loop/controller integration still needs to call the hotkey, appearance, workspace, and dropdown adapters and surface errors to the settings UI.
- Native AppKit behavior is not exercised by the headless adapter test; real interactions remain a Task 6 hand-check.
- No ADR or user documentation update was included; those are still required before the settings feature is considered complete.

## Commit

`feat: settings - apply configuration without restarting sessions` (this report is included in that commit).

## Task 3 follow-up

### Additional implementation

- Added `HotkeyRegistry` and `replace_hotkey` transaction helper. It short-circuits registry changes for an unchanged shortcut; registers the replacement before unregistering the old key; skips persistence when registration is rejected; and on save failure attempts to restore the old key and remove the new one. Errors explicitly state whether restoration/cleanup succeeded or failed, avoiding any claim that an unverified shortcut remains active. `GlobalHotKeyManager` implements the adapter trait; invocation from settings remains part of the later app integration.
- Added fake-registry tests for registration rejection without saving, unchanged shortcut, successful rollback after save failure, and explicit failure reporting when old-key restoration itself fails.
- Added `NativeAppearance` mapping and `apply_appearance` / `apply_appearance_with` adapters. The macOS implementation applies system/light/dark with AppKit on the main thread; the injectable mapping test covers all three modes. App loop invocation remains integration work.
- Added workspace tests that a runtime config update changes new-tab directory and startup mode for the next start, while the existing active-session behavior remains intact.
- Added pure resize/animation tests for recalculated panel geometry and continuing a slide from its current position toward the resized endpoint.
- Expanded WebKit coverage for dynamic appearance/bar position, theme/font updates with multiple existing terminals and retained output, inactive-tab output, and late write callbacks after tab removal.

### Follow-up verification

- `cargo test -- --test-threads=1` — passed: 227 unit tests, 1 integration test, doc tests.
- `cargo clippy --all-targets -- -D warnings` — passed.
- `cargo fmt -- --check` — passed.
- `PLAYWRIGHT_BROWSERS_PATH=/private/var/folders/n6/kfcbdjld3_sfl70ndpl5j6jh0000gn/T/opencode/regie-playwright npm test` — passed: 27 WebKit tests.
- One intermediate Playwright run failed because the test queried `style.fontFamily` instead of computed xterm styles; the assertion was corrected to check computed style and the complete suite subsequently passed.

### Concrete native hand checks deferred to Task 6

1. On macOS, change interface appearance among System, Light, and Dark while Settings and the terminal are open; verify native controls update without panel/session recreation, including returning to system appearance.
2. Change panel width/height with the panel shown and with it hidden; verify centering, menubar inset, active-terminal refit/PTY resize, and unchanged shell identity/output.
3. Change size and animation duration mid-slide in both directions; verify no jump, correct new resting/hidden endpoints, and that zero-duration changes finish coherently.
4. Use a real conflicting global shortcut to verify rejection leaves the old shortcut working and is not persisted; then test a successful replacement and a real save failure/rollback.
5. Change terminal theme with multiple live tabs and output in both active and inactive tabs; verify palette and profile font update while buffer, scrollback, selection, and shell processes remain intact.

### Remaining integration boundary

The requested adapter logic and headless behavioral coverage are now implemented, but no app event-loop/controller wiring was added, as requested for later integration. Consequently a live settings edit still does not invoke `replace_hotkey`, `apply_appearance`, `Workspace::apply_config`, or `DropDown::apply_config` in the running app. The native checks above have not been performed and are explicitly deferred to Task 6.

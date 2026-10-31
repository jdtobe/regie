# Settings verification

The automated Rust and WebKit suites exercise settings storage, validation, recovery policy, runtime adapters, watcher behavior and the bundled document. A successful test run does not replace native application verification.

On 2026-10-31 the serial Rust suite passed 244 unit tests and one integration test; the WebKit suite passed 35 tests; Clippy passed with warnings denied. These results do not include the native acceptance checks below.

Native acceptance still to perform:

- [ ] Open Settings from the menubar and Cmd+,; reopen without duplicate windows.
- [ ] Change all settings, including a conflicting hotkey, and verify immediate or next-start behavior.
- [ ] Confirm panel focus and Settings coexist; switch to another application with hide-on-focus-loss enabled.
- [ ] Verify invalid startup, parseable field recovery, malformed TOML and external edits; check warnings and retained comments.
- [ ] Exercise reset, search, keyboard navigation, all appearance modes, existing tabs and saved layout.
- [ ] Quit the already running Regie instance, launch the preview bundle, and check its actual menus, terminal sessions and settings behavior.

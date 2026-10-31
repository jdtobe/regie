# Task 4: Config file observation — implementation report

Implemented `SettingsWatch` using `notify`'s native recommended filesystem backend. It watches the nearest existing parent for each canonical and legacy settings path, updates registrations when directories appear, coalesces filesystem event bursts, returns setup errors, and stops its worker on drop.

Watcher tests use bounded channel deadlines (no fixed test sleeps) and cover writes, atomic rename, delete/recreate, canonical creation when legacy exists, burst coalescing, and teardown.

## Verification

- `cargo test settings_watch::tests` — 5 passed.
- `cargo test` — 234 unit tests and 1 integration test passed.
- `cargo clippy --all-targets` — passed.
- `cargo fmt -- --check` — passed.
- TDD red observed first: watcher tests failed to compile because `SettingsWatch` was not implemented yet.

## Concern

The required wake callback carries no error value. Initial setup failures are returned by `new`; asynchronous notification or watch-refresh failures are logged and cause a wake so the controller rereads, but cannot be surfaced as structured watcher errors through this interface.

# Task 4: Config file observation — implementation report

Implemented `SettingsWatch` using `notify`'s native recommended filesystem backend. It watches the nearest existing parent for each canonical and legacy settings path, updates registrations when directories appear, coalesces filesystem event bursts, returns setup errors, and stops its worker on drop.

Watcher tests use bounded channel deadlines (no fixed test sleeps) and cover writes, atomic rename, delete/recreate, canonical creation when legacy exists, burst coalescing, and teardown.

## Verification

- `cargo test settings_watch::tests` — 5 passed.
- `cargo test` — 234 unit tests and 1 integration test passed.
- `cargo clippy --all-targets` — passed.
- `cargo fmt -- --check` — passed.
- TDD red observed first: watcher tests failed to compile because `SettingsWatch` was not implemented yet.

## Initial concern (resolved in follow-up)

The required wake callback carries no error value. Initial setup failures are returned by `new`; asynchronous notification or watch-refresh failures are logged and cause a wake so the controller rereads, but cannot be surfaced as structured watcher errors through this interface.

## Follow-up review

Changed the callback to receive `SettingsWatchEvent::{Changed, Failed(String)}`. Async notification-source errors and re-registration errors now produce actionable failure events rather than a healthy change event; the callback lets future controller wiring display those failures. Replaced the missing-parent filesystem-only check with an injected adapter test that observes registration of the newly created config parent before injecting a later config-file change. This avoids treating a leftover directory event as proof of re-registration.

Follow-up TDD observed red while adding the new callback/event and injected adapter expectations. Follow-up verification: `cargo test settings_watch::tests` — 8 passed; `cargo test` — 237 unit tests and 1 integration test passed; `cargo clippy --all-targets`, `cargo fmt -- --check`, and `git diff --check` passed.

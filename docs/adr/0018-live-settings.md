# 18. Live settings and recoverable configuration

Date: 2026-10-31 · Status: Accepted

## Context

Regie has settings in TOML but needs an accessible in-app interface. People also edit the file directly, including while the app is running. A malformed file must not destroy user comments or prevent recovery.

## Decision

The Settings window edits the existing TOML configuration. Preserve comments and unrelated content, detect concurrent edits where possible, and reload valid external changes. Apply changes to the running app where applicable; startup tab mode affects the next startup. An invalid or unreadable config remains untouched. At startup use defaults and offer repair in Settings; while running retain the last applied configuration. Parseable files support field-level correction, while malformed TOML requires manual repair. Canonical symlink writes are refused rather than replacing a link.

This supersedes ADR 0005's read-once and invalid-startup behavior; its accepted text is not changed. A filesystem compare-and-rename cannot provide a fully atomic transaction against uncoordinated external writers, so a narrow race remains after the final check.

## Consequences

Config watching, hotkey registration, UI and runtime state must handle failure explicitly. Live updates must not destroy terminal sessions. Native macOS focus, menus, alerts and watcher behavior need hands-on verification in addition to automated tests.

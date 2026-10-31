# Regie candidate preview acceptance

Date: 2026-10-30. Palette status: **candidate, not frozen**.

## Local artifact

In this session's linked worktree:

```text
/Users/jdtobe/.local/share/opencode/worktree/1940a0/rake-terminal/dist/Regie.app
/Users/jdtobe/.local/share/opencode/worktree/1940a0/rake-terminal/dist/Regie-v0.1.0-arm64.dmg
```

Development version remains 0.1.0. The DMG is ad-hoc signed, not notarised or
published. It has been mounted/verified for literal Regie metadata, arm64 binary,
signature, complete icon and Applications link. Existing Rake artifacts and the
installed/running app were not replaced. No feature branch push or release made.

**Before opening:** save work and quit old Rake yourself. The two names share
the old instance lock; launching Regie while Rake runs will summon the old app,
not preview Regie. No live terminal transfer occurs. Open the local app rather
than dragging it into Applications until you choose to replace the installation.

Canonical config: `~/.config/regie/config.toml`. Canonical layout:
`~/Library/Application Support/Regie/tabs.json`. Absent entries read the old Rake
paths, with notices; no automatic config move/write. Successful layout saves go
to Regie. Broken canonical entries do not fall back. For the old color policy,
set `terminal_theme="macos"` and restart. Otherwise Regie Dark is the new default,
including with an old config. Readable Terminal font is retained in both modes.

## Automated/local evidence

Environment: macOS 27.0.1 (26A434), Apple Silicon; Rust 1.99.1/Cargo 1.99.1;
Node 22.23.4, Playwright 1.56.1 pinned WebKit. Earlier development also ran under
Node 26; the final gate explicitly used the checksum-verified Node 22 in scratch.

- [x] 173 Rust unit tests and one production-document integration test; all targets
  and doc tests, strict locked Clippy and formatting.
- [x] 24 WebKit tests: real generated documents, all sixteen foreground/background
  ANSI slots in **available WebGL and forced DOM fallback**, palette independence
  through all UI appearances/live media, actual cursor and drag-selection pixels.
- [x] Reveal exact colors, label/focus contrast, keyboard outlines, busy/aria state,
  overflow, top/bottom, rename round trips, selection/clipboard and late events.
- [x] 14 Python tests including real mounted packages, independent bundle identity,
  ICNS extraction/dimensions, invalid-version/no-mutation and generic Regie stamping.
- [x] Legacy config/restore names/order/selection/cwd, canonical precedence, invalid
  UTF-8/TOML, broken/denied entries, cancel/failed-startup recovery, shared instance
  protocol in both acquisition orders, canonical save with live socket retained.
- [x] Pixel failures first for old icon geometry and missing theme APIs; renderer
  mutation reproducing missing named ANSI keys fails both renderers before fixture
  regeneration passes. A discovered socket close-before-write race has a delayed
  writer regression and bounded nonblocking request read.
- [x] Final independent review's important fallback finding reproduced using native
  keyed archives: no decodable profile colors now trigger Regie Dark and a notice,
  retaining the separately readable Menlo font. A readable partial profile keeps
  neutral missing-field defaults, without Regie selection overlays.
- [x] Native source assets inspected at 16/32/48/128/512/1024, 22/44 templates on
  light/dark preview surfaces. Native system tinting remains a hand check below.
- [x] actionlint and ShellCheck; npm audit: zero vulnerabilities. cargo-audit 0.22.2
  exits 0, with existing informational warnings RUSTSEC-2024-0370
  (`proc-macro-error` unmaintained) and RUSTSEC-2024-0429 (`glib` unsound iterator),
  from the upstream Linux GTK tree. Neither suppressed nor fixed by dependency churn.

## Real utility output, supplemental renderer check

Installed: tree 2.3.2, ag 2.2.0, Claude Code 2.1.314, OpenCode 2.2.3. `htop` was
not found. No installs, tool reconfiguration, authentication or model prompts.

In harmless scratch files containing a Rust source, directory and symlink:

| Command | Result |
|---|---|
| `/bin/ls -lG` | Exit 0; permissions/path/symlink output; piped output was uncolored |
| `tree -C -p` | Exit 0; actual colored directories/symlink plus permissions |
| `ag --unrestricted --color --numbers -C1 Regie .` | Exit 0; actual matches and context |
| `ag --unrestricted --color no-such-pattern .` | Exit 1; no matches, empty output |
| `ag --unrestricted --color Regie missing-file` | Exit 1; visible file error |

`--unrestricted` only bypasses ignored scratch fixture paths for these commands;
no saved ag preferences changed. Captured bytes were rendered and inspected in
the production xterm document: paths/tree, highlights/line numbers/context and
error text were legible. This is **not native-app workload acceptance** and the
synthetic bold/dim/inverse/underline sample is likewise supplemental.

## Pending native/daily-use checks

- [ ] Actual Finder/About icon, app/menu names and version; 1×/2× macOS menubar
  tinting in light/dark. Verify chevron/bar spacing at normal viewing distance.
- [ ] Second launch/reopen summons Regie after old Rake has been quit. Confirm
  old/new exclusion without losing unsaved shell work.
- [ ] Ordinary shell/colored file browsing, tree and ag in native Regie with light
  and dark controls; long lines, resize/refit, hide/show with output, per-tab isolation.
- [ ] Bold/dim/inverse/underline; focused/inactive selections, cursor over a glyph,
  keyboard focus, selected/inactive tabs and ended sessions in native WKWebView.
- [ ] htop meters/selected row/footer and focus states (missing tool; no install done).
- [ ] Claude Code and OpenCode user-driven prompts/output, code/diffs, tool status,
  approval prompts and selection; check their own truecolor themes without forcing
  recoloring. Do not submit paid prompts or grant permissions just for a smoke test.
- [ ] Cancel close/quit with real foreground/background/stopped jobs; optional
  restore with fresh shells. No commands, scrollback or SSH reconnection restored.

Force Quit, logout/shutdown and external signals remain outside guaranteed quit
confirmation. No claims of Linux/Windows application support. A light terminal
theme, Settings UI, website deployment and remote repository rename remain future.

## Final review notes

One minor deferred: bold/dim/inverse/underline samples do not yet have localized
pixel assertions for each attribute in both renderers. Their presence in a sample
is not claimed as complete attribute verification.

Execution decisions: the socket listener now consumes the existing `show` bytes
before closing to prevent BrokenPipe; bounded reads can delay malformed local
clients up to 500ms. The compatibility test permits up to 2s for a dropped lock to
become reclaimable while concurrent PTY pre-exec children drain inherited CLOEXEC
descriptors; this could mask sub-2s test release delays, not a persistent leaked lock.

Review deliberately left Finder/About/menubar appearance, native launch/reopen and
old/new process interaction, WKWebView focus/selection/cursor/resize/hide-show,
real-job cancellation/restoration, named native utility/agent workloads and final
palette acceptance to the pending user checklist. Cost: those native issues may
still surface during daily use. Unsupported Linux/Windows behavior was not judged
(no support claim); installation/notarisation/publication/repo rename/website were
not attempted (separate authorization needed; no shipping/install proof implied).

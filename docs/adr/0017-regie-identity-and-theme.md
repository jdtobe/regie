# 17. Regie identity, safe compatibility and independent terminal colors

Date: 2026-10-30
Status: Accepted

## Context

The application is renamed from Rake to **Regie**, without an accent, from French
*régie* (control room). The approved Reveal identity and Descending Cue mark need
durable production assets, while existing config/layout and running copies must
remain safe. The reviewed Regie Dark palette is promising, not final workload proof.

## Decision

- Product/bundle is Regie; Cargo executable/library is `regie`, native identifier
  `com.jonathantobe.regie`. No `rake` executable alias. Real GitHub URLs stay
  `jdtobe/rake` until separately renamed. No website deployment is implied.
- Commit SVG/PNG/ICNS Descending Cue assets with approved cue-only translations
  (menu 1.75 units, app 8.75); runtime decodes the same PNG masters. Reveal controls
  use system sans and native AppKit menus; no downloaded or bundled fonts.
- Canonical config is `~/.config/regie/config.toml`; layout is
  `~/Library/Application Support/Regie/tabs.json`. Only genuine absence permits a
  read-only fallback to the corresponding Rake path, with a notice. Present broken
  canonical entries do not silently restore legacy data. New layout saves target
  Regie, leaving old bytes alone; cancelled quit/failed startup preserve recovery.
- Keep the Rake lock/socket namespace so old/new builds cannot compete for the
  hotkey or sessions. Users quit old Rake before preview; no live PTY transfer.
- `terminal_theme="regie"` is the default even for legacy configs missing that
  key. `"macos"` opts into previous profile colors, without Regie overlays; missing
  profiles use Regie Dark with a notice. Font family/size remain profile-derived
  independently where readable, otherwise existing generic fallback without a
  forced size. Settings require restart.
- Emit xterm's named sixteen ANSI fields and cursor/selection values. UI
  `appearance` stays independent. Do not alter bold mapping, cursor blink, tool
  themes, shell configuration or arbitrary truecolor output for branding.

## Consequences

This supersedes ADR 7's name/path decision, ADR 10's old artwork, and ADR 9's
default-color policy, without editing those historical records or replacing the
profile reader. Regie Dark remains a candidate pending native and named-workload
acceptance. [Brand guide](../brand.md) and [acceptance record](../testing/regie.md)
separate automated evidence from pending hand checks. Release publication,
installation replacement and repository rename still need separate authorization.

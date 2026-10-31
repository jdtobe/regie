# Regie: identity, rename and candidate terminal theme

Date: 2026-10-30
Status: Approved by user on 2026-10-30, including the proposed setting and migration/compatibility decisions below. Terminal palette remains a candidate pending real-workload validation.

## Intent

Rename Rake to **Regie**, plain spelling without an accent. The name comes from
French *régie*, a control room. Make the application, its icons and its future
website feel like one quiet, useful tool rather than a generic terminal brand.
The application remains a macOS-first drop-down terminal with independent tabs.

The user approved proceeding with the full application rebrand and production
icons, leaving the actual website for later. They also want a bundled terminal
theme based on the small shell preview. That palette is a promising candidate,
not a frozen final theme: it needs validation in their real terminal workloads.

## Agreed visual direction

- **Descending Cue**, with the revised spacing: continuous side brackets, a
  down-chevron above a horizontal cursor-like bar, with the pair centered inside
  the brackets. Not the stemmed arrow/download-like variant.
- **Reveal** palette and character: sage, forest ink, warm paper, restrained depth
  in the app icon, and an editorial serif paired with system sans and monospace
  for future website typography. This is not a request to use serif text in
  terminal output or native controls.
- App icon: forest symbol on the sage tile. Menubar: the same identity simplified
  as a monochrome, transparent template that macOS tints for light/dark surfaces.
- Terminal theme and surrounding UI appearance remain independent. UI keeps
  `appearance = "system" | "light" | "dark"`; changing it must never change the
  terminal palette. Regie Dark is the proposed default terminal theme. A light
  terminal theme is not part of this change.

## Durable artwork specification

The selected local sketches live in ignored brainstorming scratch and must not
become a runtime dependency. Production masters and a concise brand reference
belong in the repository. The chosen geometry is recorded here so it can be
reconstructed even if that scratch is lost.

### Menubar reference, 22-unit canvas

Black with alpha, rounded caps and joins, stroke 2.1:

```text
brackets: M7 4 H4 V18 H7  M15 4 H18 V18 H15
cue:      M8.5 6 L11 8.5 L13.5 6  M8.5 12.5 H13.5
cue translation only: (0, 1.75)
```

The translation is the approved spacing correction. Keep the brackets in place;
do not regress to the high, crowded chevron. Produce 22 px and 44 px template
assets, then check the actual rendered menubar at 1×/2×. The builder may make small
pixel/optical refinements if needed, retaining the proportions and centered cue;
show any noticeable change to the user rather than silently redesigning it.

### App reference, 128-unit concept canvas

- Tile: x/y 8, width/height 112, radius 25. Subtle diagonal gradient from
  `#D5E4D1` to `#A2BCA9`, with a restrained light edge. No baked outer drop shadow
  that clips at smaller sizes.
- Symbol: `#2B4C3B`.
- Brackets, rounded 9-unit stroke:
  `M46 29 H30 V99 H46 M82 29 H98 V99 H82`.
- Cue, rounded 10.5-unit stroke:
  `M51.5 39 L64 51.5 L76.5 39 M51.5 71.5 H76.5`.
- Translate the cue only by `(0, 8.75)`.

Create a 1024 px vector master, PNG master and complete `.icns` sizes from 16 to
1024 px. Check 16/32/48/128/512/1024 sizes, icon transparency and Finder/About
rendering. Provide a standalone vector symbol and wordmark lockup suitable for
future web use, but no website build or deployment. Production menubar rendering
and committed SVG/PNG reference assets must describe the same chosen mark.

## Reveal UI and brand tokens

Core identity: sage `#BAD5BD`, forest `#1C2B25`, paper `#F4F0E7`.

Replace amber tab accents and unrelated charcoal UI tones with a coherent
Reveal-based light/dark control palette. Use dark forest accents on light
surfaces and light sage accents on dark surfaces; a pale sage line alone on paper
is not sufficient contrast. Preserve keyboard focus, hover, selected/busy/ended
states, rename input and notices. Colors must not be the sole state cue.
Keep existing layout, tab sizes/placement, shortcuts and native appearance
behavior; do not introduce custom-drawn native menus or a Settings window.

The brand reference should name semantic UI roles and their values, icon rules,
and typography roles. Use existing system fonts for application controls and
terminal font fallback; Georgia/system serif is the website study direction, not
a requirement to bundle a new font or add a network font dependency.

## Regie Dark, candidate default

Use these initial values from the reviewed real-xterm.js preview:

| Role | Value |
|---|---|
| Background | `#121615` |
| Foreground | `#DFE6DC` |
| Cursor | `#C6D8BE` |
| Cursor character | `#121615` |
| Selection background | `#344A3B` |
| Selection foreground | `#F2F4ED` |

| ANSI slot | Normal | Bright |
|---|---|---|
| Black | `#36483A` | `#819082` |
| Red | `#E08A83` | `#F1AAA1` |
| Green | `#A8C59A` | `#C1DBAF` |
| Yellow | `#D6BF86` | `#E9D4A0` |
| Blue | `#92B4CE` | `#B2D0E8` |
| Magenta | `#B6A1CC` | `#D0BCE3` |
| Cyan | `#8CBFBA` | `#ACD8D1` |
| White | `#D8DFD3` | `#F3F4EB` |

Measured preview pairs: foreground/background about 14.3:1, muted bright-black
text/background about 5.4:1, selection text/fill about 8.7:1. These are not a
claim that every ANSI combination or program is accessible. ANSI black is
intentionally dark; do not use it as the general-purpose muted foreground.
Actual bold/dim/inverse text, inactive selection, cursor visibility and WebGL/DOM
fallback need checks in the application's renderer, not just swatch math.

Do not silently change the renderer's bold-color mapping, cursor blink/style or
other interaction behavior merely to reproduce the preview. The preview used a
nonblinking block for inspection; production keeps existing cursor behavior
unless a readability defect requires a documented change. Do not turn dim text
into bright text, collapse the 16 colors into one sage family, or remap arbitrary
256-color/truecolor output.

### Proposed terminal-theme setting (builder call)

```toml
terminal_theme = "regie" # default; alternative: "macos"
```

This selects colors, not UI appearance. `"regie"` supplies all candidate palette,
cursor and selection values; `"macos"` follows the macOS Terminal default profile
as before, without overlaying Regie-specific colors. If that profile cannot be
decoded, use the deterministic Regie Dark color fallback and report the fallback.
Keep strict unknown-key/enum validation. Like existing settings, changes take
effect after restart; no live settings editor is required.

Preserve the current macOS Terminal-derived font family/size where readable,
independently of the color source; otherwise retain current font fallback
behavior. A palette is not authorization to change the user's shell prompt,
startup files, `LS_COLORS`, `ag` options or another program's configuration.

Under this proposal Regie Dark is the default even when an old config has no
`terminal_theme` key. Document this intentional default change and the explicit
`"macos"` opt-out rather than silently pretending the app still follows profile
colors automatically.

## User's acceptance workloads

The user named normal Linux-style utilities, file browsing, `ag`, `tree`, `htop`,
and especially Claude Code and OpenCode. Test the corresponding tools on macOS;
this does not expand the supported application platform to Linux.

- Directory/file distinctions, permissions, symlinks and paths; ordinary `ls`
  and `tree` output, including colored output if supported by the installed tool.
- `ag` matches, filenames, line numbers, context and no-match/error cases.
- `htop` meters, selected row, footer keys, focused and inactive states.
- Claude Code and OpenCode prompts, assistant output, code/diffs, tool status,
  permission/approval prompts and selection. Each may supply its own theme or
  truecolor output: verify legibility and integration, not forced recoloring.
- Usual ANSI stress cases: all 16 slots, normal/bold/dim/inverse/underline,
  selections, long lines, output while hidden, resize and per-tab isolation.

Do not install or reconfigure these user tools, change agent themes, authenticate,
submit prompts, grant permissions or spend provider tokens merely for a smoke
check without separate consent. Use installed tools and harmless local fixtures
where possible; record missing-tool/interactive checks as pending for the user.
Mock ANSI transcripts are supplemental design examples, not evidence of actual
Claude Code/OpenCode/htop compatibility. Keep a checked/pending acceptance record.
Provide a local rebranded build for daily-use feedback before declaring the
palette final.

## Application rename

Update the current product surface consistently:

- Cargo package/library/binary `regie`, Rust imports, compiled webview bridge and
  internal asset/template identifiers currently named for Rake.
- App, About/menu/window labels, credits and error messages: **Regie**.
- Bundle `Regie.app`, executable `regie`, icon `Regie.icns`, bundle identifier
  `com.jonathantobe.regie`, DMG `Regie-v<version>-arm64.dmg`.
- Package verification, scoped version stamping/tests, release/check workflows,
  artifact paths and current README/contributor/security instructions. Retain
  numeric Cargo/plist versions, v-prefixed About/tags, calendar versioning,
  manual-only release and the existing least-privilege/dry-run boundaries.
- Preserve MIT authorship/copyright. Record the rename, new default theme and
  migration in new ADR(s); never edit accepted ADRs. Historical design/plan
  documents retain historically accurate Rake names, with a current pointer if
  useful. Do not blanket-replace historical evidence.

Do not add a compatibility `rake` command: that name also belongs to Ruby's build
tool. The old application's launch does not become a command alias for Regie.

### Proposed data compatibility (builder calls)

Canonical config: `~/.config/regie/config.toml`.

- If present, read it exclusively. An unreadable/invalid file is an explicit
  startup error, not permission to fall back to another config.
- Only when absent, read `~/.config/rake/config.toml` as a legacy fallback with a
  clear notice naming the file actually used. Do not copy/move/delete or overwrite
  the user's file. If neither exists, use defaults. Do not merge two configs.

Canonical layout: `~/Library/Application Support/Regie/tabs.json`.

- Prefer the canonical layout. Only when it is absent, permit loading the legacy
  Rake layout for startup restoration. Preserve existing schema validation,
  failed-startup protection and fresh-local-shell-only semantics.
- Future successful saves go to the new Regie path using the existing atomic
  writer. Do not modify/delete the old layout. A present but bad new layout gets
  the existing warning/fallback behavior, not an undisclosed legacy rollback.
- No live session transfer between Rake and Regie; ask the user to quit the old
  application before running the preview. Existing restoration preferences apply.

Keep **one shared lock/socket namespace** with old Rake during the transition.
Specifically, reuse its `~/Library/Application Support/Rake/instance.lock` and
`instance.sock` paths as compatibility infrastructure, separate from Regie's new
layout directory. This avoids two competing copies/hotkeys and preserves the
old process's second-launch protocol. Do not bind a second socket over a live
old one or try to transfer its PTYs. A running old Rake may therefore be summoned
instead of the new build; make quitting the old copy explicit in preview docs.
This deliberate legacy-path exception is not a missed visible rename.

### External operations

Implement and review in a feature branch, not main. GitHub repository rename,
remote URL/local checkout-directory rename, enabling Pages, custom domains,
release dispatch/publication and replacing the user's installed application are
separate operations requiring explicit authorization. Until then, links to the
actual `jdtobe/rake` repository/releases remain valid; do not generate broken
`jdtobe/regie` links as if a remote rename had already happened. A future project
site can use GitHub Pages; no site is built or deployed in this milestone.

## Non-goals and anti-goals

No Settings window, light terminal theme, live theme switching, shell/editor/agent
reconfiguration, new fonts/dependencies just for styling, website implementation,
new tab capabilities, single-instance/process-lifecycle redesign, performance
polish unrelated to the rename, signing/notarisation or release publication.
Do not sacrifice menubar legibility for large-icon detail, lose old configuration
or restorable layout, break safe close/quit, force UI appearance onto terminal
colors, or report user workloads as checked without running them.

## Verification and handoff

TDD for rename/migration/theme behavior, plus actual asset/package inspection.
Exercise canonical/legacy path precedence, unreadable and invalid files, no
mutation of legacy data, shared old/new instance exclusion, layout preservation,
theme defaults/validation/serialization, font independence, strict IPC after bridge
rename and UI appearance not mutating terminal colors. Browser checks must use
the bundled app document. Inspect icons/alpha/dimensions, built bundle plist,
executable, signature, image layout and second-launch behavior.

Run fmt, strict all-target Clippy, all Rust/doc tests, WebKit tests, version/package
script tests, actionlint/ShellCheck, audits and verified development packaging.
Start from merged main (currently 740266e, including tao/wry patch updates), not
the old pre-merge branch snapshot. Preserve existing functionality and track any
actual blockers or upstream audit warnings honestly.

Handoff: reviewed design, implementation plan, logical tested commits, production
icon assets and brand reference, local preview/build instructions and acceptance
evidence. The candidate theme stays provisional pending the user's real workload
feedback. PR/release operations beyond this design are not implied by approving
the mockups.

## Design checkpoint

- [x] Name, refined icon and Reveal direction selected with the user.
- [x] Candidate terminal palette previewed in real xterm.js; user wants further
  real-workload validation before treating it as final.
- [x] User acceptance tools recorded above.
- [x] Fresh builder check found nothing important left before planning.
- [x] User approves this complete design, including the proposed setting and
  migration/compatibility decisions.
- [x] Write the implementation plan after design approval; no product code has
  been changed for this milestone yet.

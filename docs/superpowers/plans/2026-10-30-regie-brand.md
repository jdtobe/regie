# Regie Rebrand and Candidate Terminal Theme Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan inline, task-by-task. Preserve the user's lower-cost inline preference; do not start per-task implementer/reviewer agents. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver a reviewable local Regie build with the approved Descending Cue icons, Reveal controls, candidate Regie Dark default and safe Rake-data compatibility.

**Architecture:** Explicit application paths separate canonical config/layout from the legacy shared instance namespace. Rust resolves terminal colors and fonts independently; the existing bundled xterm document consumes its named color fields. Source SVGs generate committed native assets; current session, close/quit, output and window policies remain unchanged.

**Tech Stack:** Existing Rust/tao/wry/xterm.js, serde, png; Node 22 + pinned Playwright WebKit for renderer tests and offline SVG rasterization; Python standard library, macOS iconutil/sips/hdiutil/codesign for artifact checks. No new runtime dependencies.

**Spec:** `docs/superpowers/specs/2026-10-30-regie-brand-design.md` (approved).

## Global Constraints

- Product **Regie**, command/Cargo name `regie`, no accent. macOS-first; Linux/Windows app support is not claimed.
- Descending Cue with the approved spacing correction, not a stemmed down arrow. Use the exact spec geometry as the master; show any noticeable optical change to the user.
- Reveal: sage `#BAD5BD`, forest `#1C2B25`, paper `#F4F0E7`. Website serif styling is not terminal or native-control typography.
- `terminal_theme = "regie" | "macos"`, default `"regie"`, applies on restart. Regie Dark stays dark independently of `appearance = "system" | "light" | "dark"`.
- The approved spec's six core terminal colors and sixteen ANSI values are the initial candidate; do not silently choose another palette. Preserve font preferences and current cursor/renderer interaction behavior.
- Read old config/layout only if the canonical entry is absent. Never overwrite/move/delete legacy user files or merge configurations. Keep old/new app exclusion via the legacy Rake lock/socket namespace.
- No Settings window, light terminal theme, shell/prompt/editor/agent reconfiguration, new terminal features, actual website, remote repo/directory rename, Pages/domain setup, installation replacement, release dispatch or publication.
- Do not change accepted ADRs or blanket-replace historical design/evidence. Actual GitHub links remain `jdtobe/rake` until a separately authorized remote rename.
- Start from merged main `740266e` (tao/wry patches included); current branch `feat/regie-brand` contains only the approved design. Keep the user's main checkout, running app and generated previews intact.
- Each task: RED → GREEN → refactor, whole Rust suite + strict Clippy + fmt, applicable browser/script checks, one logical commit. Use the execution ledger; preserve prior brainstorming artifacts.

## Review Focus

- A present-but-invalid, unreadable, directory-valued or dangling-symlink canonical file must not silently load legacy data (Task 1).
- A failed restore or two old/new launches must not corrupt a layout or allow competing instances/hotkeys; lock ownership stays separate from layout writes (Tasks 1, 6).
- A partial/unreadable macOS profile must neither leak Regie selection/colors into `macos` mode nor discard a separately readable font (Task 3).
- Correct-looking JSON is not proof of correct ANSI rendering: test named color fields, both renderers, bold/dim/inverse, and UI changes with an unchanged terminal palette (Tasks 3, 5).
- Icons must remain legible at native scale and packaged app/executable/plist/icon/release names must agree; old/new assertions must not merely cancel each other's mistakes (Tasks 2, 4, 6).

## File boundaries and dependencies

1. `src/app_paths.rs` (new): canonical/legacy path values and preferred-file reading. `src/config.rs`, `src/tab_layout.rs`, `src/workspace.rs`, `src/main.rs`, `src/instance.rs`, `src/lib.rs`: integrate compatibility without duplicating precedence policy.
2. Cargo metadata/imports, current app copy, `assets/terminal/regie.js` (rename), `src/terminal_view.rs`, browser fixtures/tests, packaging/workflows and current docs: one coherent product rename.
3. `src/theme.rs`: palette, font data and pure resolution/serialization. `src/terminal_profile.rs`: small native profile/font adapters. `tests/terminal_document.rs` (new): actual production-document fixtures consumed by `tests/terminal/theme.spec.js` (new).
4. `assets/icons/`: approved vector masters and PNG/ICNS exports. `scripts/render-icons.js`, `tests/ci/test_icons.py` (new): development-only generation/inspection. `src/lib.rs`: decode the committed menu PNG rather than maintaining a second geometric drawing.
5. `assets/terminal/tabs.css`, existing strip tests: Reveal semantic controls. `docs/brand.md` (new), `assets/icons/README.md`: durable brand rules independent of ignored mockups.
6. `README.md`, `docs/roadmap.md`, new ADR 17, `docs/testing/regie.md` (new): truthful behavior, compatibility and candidate-theme evidence; local preview, not release publication.

Tasks 1→2 establish identity/paths. Task 3 uses Task 1 config and Task 2 bridge. Task 4 uses Task 2 asset/package names. Task 5 uses Tasks 3/4 for integrated checks. Task 6 verifies their combined result. These are one identity milestone, not independent product launches.

### Task 1: Separate canonical data paths from shared runtime compatibility

**Files:** Create `src/app_paths.rs`; modify `src/{lib,config,tab_layout,workspace,main,instance}.rs`. Tests stay beside their owning modules.

**Interfaces:**
- `AppPaths::new(home: &Path) -> AppPaths`; fields `config`, `legacy_config`, `layout`, `legacy_layout`, `runtime`: `PathBuf`.
- Exact values: `.config/regie/config.toml`, `.config/rake/config.toml`, `Library/Application Support/Regie/tabs.json`, `Library/Application Support/Rake/tabs.json`, `Library/Application Support/Rake` under the supplied home.
- `read_preferred(primary: &Path, legacy: &Path) -> Result<Option<LoadedFile>, String>`; `LoadedFile { path: PathBuf, bytes: Vec<u8>, legacy: bool }`. Only genuine absence permits fallback; errors name the offending path. No filesystem writes.
- `config::load_compatible(paths: &AppPaths) -> Result<LoadedConfig, String>`; `LoadedConfig { config: Config, notices: Vec<String> }`.
- `tab_layout::load_compatible(paths: &AppPaths) -> Result<LoadedLayout, String>`; `LoadedLayout { saved: Option<SavedLayout>, notices: Vec<String> }`. Keep explicit-path `load/save_atomic` and schema 1; share parsing rather than duplicating validation.
- `Workspace::new(config: Config, home: PathBuf, paths: AppPaths, output: SessionOutput) -> Self` replaces the state-directory argument. Instance `claim/listen/request_show` signatures remain unchanged; main passes `paths.runtime`, workspace writes only `paths.layout`.

- [ ] **Step 1: Write pure path and real-filesystem precedence tests.** Reuse the existing unique temporary-directory/cleanup patterns; do not touch real HOME. Pin paths independently:
  ```rust
  #[test]
  fn regie_paths_retain_the_legacy_runtime_namespace() {
      let p = AppPaths::new(Path::new("/Users/test"));
      assert_eq!(p.config, PathBuf::from("/Users/test/.config/regie/config.toml"));
      assert_eq!(p.layout, PathBuf::from("/Users/test/Library/Application Support/Regie/tabs.json"));
      assert_eq!(p.runtime, PathBuf::from("/Users/test/Library/Application Support/Rake"));
  }
  ```
  `read_preferred` fixture cases: canonical bytes `b"new"` beat legacy `b"old"`; absent canonical returns old bytes and `legacy=true`; both absent returns None; a canonical directory or dangling symlink returns Err naming canonical, even when legacy contains valid bytes. Also check invalid UTF-8 config, malformed TOML and denied read where supported: errors, never legacy fallback.
- [ ] **Step 2: Run red.** `cargo test app_paths::tests` and targeted config/layout tests. Expected: missing interfaces or incorrect current paths/precedence; retain stderr. Do not treat a typo/import unrelated to the new API as evidence.
- [ ] **Step 3: Implement preferred reads and integrate them.** Distinguish a missing directory entry from a broken link with `symlink_metadata`; do not use `Path::exists()` to swallow errors. Parse config from the selected bytes with strict validation; legacy notice includes its actual path. Replace main's old single-path config call, print notices, remove obsolete unused path APIs. Start/restore in Workspace loads compatible layout once; save uses only canonical layout. Startup read errors keep existing warning/fresh-tab behavior. Keep `preserve_saved` and cancellation semantics intact.
- [ ] **Step 4: Add integration regressions, observe red, then make green.** Config legacy `height_percent=63` must load as 63 without creating the new config. Workspace with restore and a manually named legacy tab must retain cwd/name/order/selection and save a new layout while old bytes stay identical. A corrupt canonical layout with valid legacy must warn and not restore legacy. Failed/partial spawns and cancelled quit preserve recoverable files. Old and new clients claiming `paths.runtime` must have exactly one winner; losing client's `request_show` reaches the holder. Test both claim orders, stale socket recovery and release after drop.
- [ ] **Step 5: Verify and commit.** Run `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`. Expected: all exit 0, including existing instance/layout/workspace tests. Commit `feat: compatibility - preserve Rake data while separating Regie paths`.

### Task 2: Rename the executable, document bridge and distribution coherently

**Files:** Modify `Cargo.toml`, root entry in `Cargo.lock`, `package.json`/root npm lock metadata (`regie-terminal-tests`), `src/{main,lib,app_ui,dropdown,terminal_view}.rs`, `assets/terminal/index.html`; rename `assets/terminal/rake.js`→`regie.js` and `assets/icons/Rake.icns`→`Regie.icns` (existing picture until Task 4). Update `tests/terminal/{fixture,tabs.spec,strip.spec}.js`, `scripts/{package,verify-package}.sh`, `tests/ci/{test_package,test_version}.py`, `.github/workflows/release.yml`, current `README.md`, `AGENTS.md`, `CONTRIBUTING.md`, `SECURITY.md` and current architecture/roadmap/open-question references if they describe the live product. Audit other owned runtime files for visible names; no vendored asset or historical-doc blanket replacement.

**Interfaces:** Cargo/library/binary `regie`; `APP_NAME="Regie"`; JS namespace `window.regie`, placeholders `__REGIE_CONFIG__`/`__REGIE_JS__`, bundled `regie.js`. IPC message shapes and session IDs are unchanged. Bundle `Regie.app`, executable `regie`, identifier `com.jonathantobe.regie`, icon `Regie.icns`; image `Regie-v<version>-arm64.dmg`. Retain existing public library helper signatures except the Task 1 path integration.

- [ ] **Step 1: Make consumer-visible expectations red.** About must return `Some("Regie")` and `v<package-version>`. Add a native Python metadata assertion inside `NativePackage` using `cargo metadata --locked --no-deps --format-version 1`: root package `regie` and binary target `regie` (do not require Cargo in Ubuntu's portable Python-only script job). Extend the native package test to independently mount the actual produced image and assert `Regie.app/Contents/MacOS/regie`, literal bundle identifier `com.jonathantobe.regie`, both version keys `0.1.0`, icon file `Regie`, `/Applications` link, and no `Rake.app`. Do not depend only on verifier success, because a wrong packager/verifier pair could agree.
- [ ] **Step 2: Run red.** `cargo test about_metadata_uses` and `python3 -m unittest discover -s tests/ci -v` after a baseline `cargo build --release --locked`. Expected: old name/metadata/native bundle fails the new literal assertions. Use temporary package fixtures, never the user's installed bundle.
- [ ] **Step 3: Rename and update consumers atomically.** Rename Cargo root metadata without upgrading dependencies. Route native labels through APP_NAME. Rename bridge constants, injection tokens and all JS callers/tests together. Rename the existing ICNS file so the package remains buildable; Task 4 replaces artwork. Update the release publisher's exact download filename and checksum note, leaving triggers, guard, SHA pins, permissions, target commit and dry-run gating unchanged. Keep real GitHub repository URLs as `jdtobe/rake`. No `rake` executable alias.
- [ ] **Step 4: Verify renderer and version behavior.** Existing browser input/output/selection/paste/rename/late-event tests must pass through `window.regie`; no source-text-only replacement test. Change the version fixture's root name to `regie` and retain dependency version `9.9.9`, valid stamp `v26.10.2`, invalid-lock no-mutation assertions. The stamper already discovers the root package generically: if these pass without a production edit, record that existing behavior instead of rewriting the script. Run `cargo build --release --locked`, Python native/version tests and `npm test`. Expected: all exit 0 and the verified image has the new identity.
- [ ] **Step 5: Full checks and commit.** `cargo test`, strict Clippy, fmt, actionlint and ShellCheck on scripts/workflows. Expected: exit 0; no unintended lockfile dependency changes or broken links to a not-yet-renamed repository. Commit `feat: Regie - rename the app executable bridge and distribution`.

### Task 3: Resolve Regie Dark, profile colors and fonts independently

**Files:** Modify `src/{config,theme,terminal_profile,main,terminal_view}.rs`, `assets/terminal/regie.js` only if needed to consume valid xterm options; create `tests/terminal_document.rs`, `tests/terminal/theme.spec.js`; modify `tests/terminal/fixture.js`, `package.json`/`package-lock.json` only for test-script metadata (no dependency updates).

**Interfaces:**
- `TerminalThemeSource::{Regie,Macos}`, serde snake_case, default Regie; config field `terminal_theme`.
- Keep existing flattened `TerminalTheme` fields; add `cursor_accent`, `selection_background`, `selection_foreground`: `Option<Rgb>`. `TerminalTheme::regie_dark() -> Self`; Default uses it. The six base values and ANSI array order are exactly the spec's table.
- `TerminalFont { family: Option<String>, size: Option<f32> }` and `terminal_profile::default_font() -> TerminalFont`, independently decoded. Keep `from_default_terminal() -> Option<TerminalTheme>` for color-profile mode, with its old neutral fallback rather than accidentally using new Regie defaults for missing profile fields.
- `theme::resolve(source: TerminalThemeSource, profile: Option<TerminalTheme>, font: TerminalFont) -> ThemeResolution { theme: TerminalTheme, notices: Vec<String> }`: Regie colors + common readable font; Macos colors unchanged + same font policy; missing profile in Macos mode uses Regie Dark with a clear notice. Use existing generic font fallback when no font can be read, no forced new size.
- `config_json()` emits actual xterm names: `black`…`white`, `brightBlack`…`brightWhite`, plus `cursorAccent`, `selectionBackground`, `selectionForeground`. **Do not emit the old non-xterm `ansi` array as the renderer interface.** The Rust array may remain internal. Absent profile values stay absent in Macos mode; no Regie selection overlay.
- `REGIE_TERMINAL_FIXTURE_DIR` in `tests/terminal_document.rs` enables writing six production `workspace_html` fixtures (`regie-{system,light,dark}.html`, `macos-{system,light,dark}.html`) with deterministic synthetic profile/font inputs, not the real user's settings. A meaningful integration test validates the resolved options while producing them. Add npm `pretest` to run this integration target with output `target/terminal-fixtures`; ordinary Cargo runs use disposable scratch or skip file output, not test assertions.
- `generated_workspace(page, source='regie', appearance='system')` in the JS fixture loads that real document and captures IPC like existing `workspace`; no alternate test-only terminal implementation or global exposure of production session internals.

- [ ] **Step 1: Write failing config and pure resolution tests.** Exact anchors:
  ```rust
  assert_eq!(Config::from_toml_str("").unwrap().terminal_theme, TerminalThemeSource::Regie);
  assert_eq!(Config::from_toml_str("terminal_theme='macos'").unwrap().terminal_theme, TerminalThemeSource::Macos);
  assert!(Config::from_toml_str("terminal_theme='system'").is_err());
  let wire: serde_json::Value = serde_json::from_str(&TerminalTheme::regie_dark().config_json()).unwrap();
  assert_eq!(wire["theme"]["background"], "#121615");
  assert_eq!(wire["theme"]["red"], "#e08a83");
  assert_eq!(wire["theme"]["brightWhite"], "#f3f4eb");
  assert_eq!(wire["theme"]["cursorAccent"], "#121615");
  assert_eq!(wire["theme"]["selectionBackground"], "#344a3b");
  assert_eq!(wire["theme"]["selectionForeground"], "#f2f4ed");
  assert!(wire["theme"].get("ansi").is_none());
  ```
  Also assert all sixteen literal values from the spec, not expectations computed by the production palette helper. `resolve_preserves_font_in_both_sources`: inject `Some("Menlo")`, `Some(14.7)` with a synthetic `#212121` profile; both modes keep that font, only Macos keeps `#212121`. `macos_partial_profile_has_no_regie_selection_or_cursor_accent`; `missing_macos_profile_warns_and_uses_regie_colors`; `regie_colors_survive_missing_color_profile_with_readable_font`.
- [ ] **Step 2: Run red.** Targeted config/theme tests. Expected: missing new interfaces/keys or old defaults/ANSI serialization; save output. No implementation before the failing expectations.
- [ ] **Step 3: Implement pure theme wiring and small native font decoding.** Extract the existing NSFont read into a small shared native helper; do not install fonts or mutate Terminal preferences. Ensure the old profile decoder does not call Regie Default for its own partial color fallback. Main resolves once, reports notices and passes the same resolved theme to the document. Keep renderer cursor blink, bold-to-bright policy, minimum-contrast policy and selection behavior unchanged unless a failing readability test justifies a recorded change.
- [ ] **Step 4: Generate actual production documents and write red WebKit tests.** `all_sixteen_ansi_slots_render`: send foreground full-block glyphs/background blocks with SGR 30–37/90–97 and 40–47/100–107; screenshot the terminal and decode the PNG in a browser canvas to assert literal RGB interiors, avoiding private xterm buffer fields or new image dependencies. Test WebGL when actually available and force the existing fallback path in a separate run; report an unavailable WebGL path rather than count it as passed. `ui_appearance_never_mutates_regie_palette`: use all three generated appearances and live system media changes; background `rgb(18,22,21)` and foreground `rgb(223,230,220)` remain, while strip appearance changes. `macos_fixture_does_not_inherit_regie_selection`: synthetic profile colors survive unchanged. Exercise real drag selection/cursor colors and sample bold/dim/inverse/underline; don't claim every application color pair meets contrast thresholds.
- [ ] **Step 5: Make renderer tests green and commit.** Run `cargo test`, strict Clippy, fmt, `npm test` (pretest exports real fixtures), and existing clipboard/resize/completion tests. Expected: exit 0; no network imports, no shell/profile writes. Commit `feat: terminal theme - add Regie Dark with independent profile and font selection`.

### Task 4: Produce native Descending Cue assets from durable vector masters

**Files:** Modify `assets/icons/{app,menubar}.svg`, PNG masters and `Regie.icns`; create `assets/icons/symbol.svg`, `wordmark.svg`, `scripts/render-icons.js`, `tests/ci/test_icons.py`; modify `src/lib.rs` and `assets/icons/README.md`. Use only existing png and Playwright dependencies.

**Interfaces:** `node scripts/render-icons.js` rasterizes source SVGs offline with pinned WebKit into transparent app PNG/iconset sizes and 22/44 px menu PNGs, invokes native `iconutil`, and cleans only its own temporary iconset. No application runtime rendering dependency. `build_icon_rgba() -> Vec<u8>`, `build_icon() -> Icon` stay public and decode the bundled `menubar-44.png`; share PNG decoding with About artwork instead of maintaining duplicate procedural geometry. `ICON_SIZE=44` remains.

- [ ] **Step 1: Write red pixel/asset checks.** In `src/lib.rs`, replace obsolete bar/pane geometry tests with rendered-mark assertions on the 22-unit grid (test helper already samples 2× alpha): opaque brackets near `(4,6)`/`(18,6)`, opaque chevron tip `(11,10.25)`, opaque cursor `(11,14.25)`, transparent gap `(11,12)` and top-center `(11,5)`. Verify black-only RGB + alpha, transparent outer corners and correct 44²×4 buffer length. About master is 1024² RGBA with transparent corners; glyph at `(64,60)` on the 128-unit concept grid is forest ink, not the old pane. Use interior pixels/tolerance only where antialiasing makes a boundary ambiguous.
- [ ] **Step 2: Run red and add native ICNS expectations.** `cargo test icon` must fail against old geometry. Python `test_icons.py` reads PNG IHDRs with standard-library struct parsing and checks 22/44/1024 dimensions and alpha-bearing output; on macOS extract `Regie.icns` with iconutil into a temporary iconset and assert standard 16/32/128/256/512 base and @2x members with their literal dimensions. Expected: old master/geometry fails relevant checks. Explicitly skip macOS-only checks outside macOS.
- [ ] **Step 3: Implement SVG masters and generation.** Use spec paths/translations exactly: menu cue `(0,1.75)`, app cue `(0,8.75)` on the 128-unit concept canvas, normalized to 1024; tile/gradient as specified. Generate at the target raster sizes rather than reusing screenshots of the mood board. Export `symbol.svg` with transparent background and a plain Regie lockup `wordmark.svg` that documents system-font dependence if lettering is not outlined. Use no external fonts/network fetch. Replace runtime drawing with PNG decode; remove old unneeded geometry functions. Do not stamp/change the product version.
- [ ] **Step 4: Inspect actual exports and rerun checks.** Run `node scripts/render-icons.js`, `cargo test icon`, `python3 -m unittest discover -s tests/ci -p test_icons.py -v`. Expected: pass; extracted 16→1024 representations are real, valid assets, not source-string assertions. Open generated PNGs at intended small/large sizes via file/image tools; compare light/dark actual-size template previews and the approved centered spacing. Note, and ask about, a noticeable optical departure rather than independently redesigning.
- [ ] **Step 5: Build/package and commit.** Full Rust checks and `cargo build --release --locked`; package/verify via Task 2 scripts in disposable test fixtures so existing previews are not overwritten. Expected: Regie icon present in built bundle and About image decodes; no shell processes/installed app changed. Commit `feat: identity - ship the refined Descending Cue app and menubar icons`.

### Task 5: Apply Reveal controls and document the brand system

**Files:** Modify `assets/terminal/tabs.css`, `tests/terminal/strip.spec.js` and theme fixture tests; create `docs/brand.md`; finish `assets/icons/README.md` production-asset guidance.

**Interfaces:** semantic CSS tokens `--bar-bg`, `--tab-fg`, `--tab-active`, `--tab-border`, `--tab-focus`, `--tab-accent`. Initial builder-selected values consistent with the approved identity:

| Token | Light | Dark |
|---|---|---|
| bar-bg | `#F4F0E7` | `#1C2B25` |
| tab-fg | `#42543F` | `#B2C5B5` |
| tab-active | `#FFFCF5` | `#24382D` |
| tab-border | `#CCD6C6` | `#354C3C` |
| tab-focus / tab-accent | `#3E6848` | `#BAD5BD` |

Native menus/dialogs continue using existing AppKit appearance rather than custom brand drawing. No serif in tab controls or the shell. Existing DOM/actions/layout remain.

- [ ] **Step 1: Write red real-DOM style and focus tests.** With production fixture `appearance=light`, tabstrip background is `rgb(244,240,231)` and active `::before` is `rgb(62,104,72)`; dark gives `rgb(28,43,37)` and `rgb(186,213,189)`. Compare computed foreground/background contrast: normal labels ≥4.5:1 and focus/active indicator ≥3:1 against adjacent surfaces. Keyboard-focus a real tab/New Tab/rename input and assert a visible nonzero outline using computed style. Keep selected role/aria state and busy dot, not only color.
- [ ] **Step 2: Run red.** `npm test -- --grep 'reveal|appearance|system_colour|focus'`. Expected: old gray/amber styles fail new values; existing palette-independence checks still pass. Do not redefine expected colors using the production CSS variables being tested.
- [ ] **Step 3: Implement tokens once.** Explicit dark and media-driven system dark use the same values; light is the base. Replace active-tab hardcoded amber with `var(--tab-accent)` and retain top/bottom orientation rules. Apply roles consistently to tabs, rename editor, notices and hover. Do not change terminal theme, tab widths/heights or native menu dispatch.
- [ ] **Step 4: Complete docs and browser verification.** Brand guide records exact icon spacing/exports, identity tokens vs terminal tokens, website serif/system sans/mono roles, template tinting rules and candidate terminal status. Link actual production assets, not ignored local server URLs. Run full `npm test`, including 30-tab overflow, top/bottom, rename round trips, selection/clipboard and live System appearance. Expected: all exit 0, including Task 3's fixed terminal colors.
- [ ] **Step 5: Full Rust checks and commit.** `cargo test`, strict Clippy, fmt. Expected: pass. Commit `feat: appearance - apply Reveal controls and record Regie brand rules`.

### Task 6: Verify the combined build, real workloads and honest documentation

**Files:** Modify `README.md`, `AGENTS.md`, `docs/{roadmap,design,open-questions}.md` where they describe current behavior; create `docs/adr/0017-regie-identity-and-theme.md`, `docs/testing/regie.md`; refine regression tests only for evidence-backed defects. Older accepted ADRs and historical plans remain unchanged.

**Interfaces:** Final user-visible settings and paths are Tasks 1/3's exact values. Distribution is Task 2's names and Task 4's assets. Acceptance evidence records command/tool version/macOS version, actual result, and explicit pending/blocked items; no real user config/shell/tool changes are implied.

- [ ] **Step 1: Add combined regressions before fixes.** In the existing workspace/config/instance harnesses: legacy `startup_tabs=restore` plus missing `terminal_theme` uses Regie Dark and restores manual/automatic names without changing old bytes; explicit new `terminal_theme='macos'` beats legacy defaults; cancelled quit and failed startup never erase recoverable layout. Simulate old/new clients in both lock-acquisition orders with the actual shared directory/protocol. New canonical data paths must not replace an owned live socket. Observe any new failure, then make the minimum fix; preserve upstream behavior already passing.
- [ ] **Step 2: Run the full local gate.** Run `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked --all-targets`, `cargo test --locked --doc`, `npm ci`, `npx playwright install webkit`, `npm test`, `cargo build --release --locked`, `python3 -m unittest discover -s tests/ci -v`, actionlint, ShellCheck, pinned cargo-audit and npm audit. Use Node 22. Analysis tools install only in this plan's ignored scratch. Expected: all exit 0; audit informational warnings are reported, never hidden or silently fixed with dependency churn. Record real counts and unavailable renderer/tool checks separately.
- [ ] **Step 3: Inspect the packaged application without touching the installed one.** Produce a development DMG in owned scratch or `dist/Regie-*` without deleting existing Rake artifacts. Verify the mounted image, plist versions/identifier, executable arm64/signature/icon and Applications link; do not publish. Keep source development version. With user cooperation or safe isolated native test fixtures, check About/Finder/menu icon, actual light/dark menubars, second launch/reopen and old/new exclusion. Do not terminate the user's running Rake or steal its global hotkey. If safe isolation is not possible, leave the native check pending with precise user instructions.
- [ ] **Step 4: Check named workloads conservatively.** First detect installed `tree`, `ag`, `htop`, `claude`, `opencode` versions without installing or authenticating. Use temporary harmless local files for ordinary file listings/tree/search and recorded read-only visual observations for htop. User checks Claude Code/OpenCode, including approval prompts, code/diffs, selection and own theme behavior; no test request may submit model prompts, change agent configuration or spend tokens without consent. Missing tools or inaccessible interactions stay unchecked. Repeat terminal readability in both UI appearances, selected/inactive tabs, hidden/show/resize and renderer fallback. Synthetic ANSI tests do not count as native app acceptance.
- [ ] **Step 5: Update current documentation and ADR.** Explain Regie name, provisional palette, settings/default change and restart requirement; font independence; canonical config/layout and legacy fallback precedence; deliberate old lock path; quit old Rake before preview; no live-session transfer; preserved safe-close/OS limitations. Existing GitHub links stay real. Describe future website as future and light terminal theme/Settings UI as deferred. ADR 17 records the changed default superseding ADR 9 for default-color policy without editing ADR 9. Include the native candidate-theme checklist and local artifact path in `docs/testing/regie.md`.
- [ ] **Step 6: Review, final regression gate and handoff.** Personally review the whole feature range against the approved spec, then obtain one focused fresh-context whole-branch review at the end, not per-task review cycles. Any important finding gets a reproducing red→green test and full green suite. Commit `feat: Regie - verify and document the rebranded preview`. Present the local preview path and pending user workload checks. Do not push/open a new PR, rename the remote repository, merge, install over the running copy or dispatch a release without separate authorization.

## Self-review

- Spec coverage: paths/data/old-new exclusion (1,6), product/bridge/package/workflows (2,6), default/macOS/font/ANSI/selection behavior (3), durable icons/export sizes (4), Reveal controls/brand reference (5), named real workloads/ADRs/handoff (6).
- Shared interfaces: AppPaths names are consistent in config, layout and Workspace; theme source uses exactly `regie`/`macos`; `window.regie` lands before generated documents/theme tests; Task 4 consumes `Regie.icns`, not the pre-rename filename.
- Review focus: canonical-entry error tests (1), restore/cancel/lock integration (1,6), profile/font/fallback tests (3), painted ANSI/fallback/UI isolation (3,5), native asset and independently inspected bundle tests (2,4,6).
- Existing ANSI-array serialization is not a valid xterm color interface; named keys and painted-color tests are deliberately part of Task 3, not an unrelated renderer rewrite.
- Scope/proportion: six integrated tasks, no new runtime library, no test-only production hooks or additional application binaries, no website/remote rename/release. Mockup SVG geometry and palette values are durable in the approved spec. All real-tool gaps remain explicit rather than blocking indefinitely on uninstalled software.

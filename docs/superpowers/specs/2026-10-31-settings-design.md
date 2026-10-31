# Settings UI design

Status: proposed for approval · 2026-10-31

## Intent and scope

Expose all 11 existing config settings in Regie so people need not edit TOML for ordinary customization. Keep hand editing supported. This is a macOS-first feature; do not claim other platforms work. Per-tab themes, additional settings such as font selection, and redesigning terminal sessions are out of scope. UI labels and grouping are independent of config keys so the interface can be simplified later without breaking existing files.

## Window and navigation

“Settings…” in the menubar icon menu and the standard macOS ⌘, shortcut open the same settings window; subsequent requests bring it forward. Settings is part of Regie: focusing it must not hide the terminal panel under hide-on-focus-loss. Normal focus-loss behavior resumes when appropriate after Settings closes.

Use the Reveal control identity and existing System/Light/Dark UI appearance, independently of the terminal theme. The approved rough direction is a search field and navigation on the left, with one scrollable list of settings on the right. Sidebar links jump to section headings; the highlighted link follows the section in view. Search filters that same list across all sections by setting labels and descriptions, allows editing directly, and offers a clear action; no matches shows a useful empty state. The mockup is directional, not a pixel-perfect contract. Controls and navigation must be keyboard accessible and labeled for assistive technology.

Suggested sections, subject to refinement:

- **General:** global hotkey, hide on focus loss.
- **Window:** width, height, animation duration.
- **Tabs:** bar position, new-tab directory, startup tabs, last-tab action.
- **Appearance:** interface appearance, terminal color source.

Use suitable controls: a hotkey recorder, switches, menus, and sliders with visible, precisely editable values. Respect existing validation ranges and permitted values. Each non-default setting has an accessible circular-arrow reset control; resetting one setting restores its default. A confirmed Reset All action restores settings defaults, not tabs, saved session layouts, shell history, or other user data.

## Application and persistence

Changes normally save and apply immediately; no Save button. Resize the visible panel and refit the active terminal as dimensions change. Apply terminal color changes to all existing tabs without restarting shells, clearing output or changing font selection. Appearance and tab-bar position update existing UI. Animation and focus behavior affect subsequent relevant actions. New-tab directory and last-tab behavior affect subsequent actions; startup mode affects the next startup, and is labeled accordingly. Existing tabs and sessions are not recreated to apply settings.

The TOML config remains the source of truth. UI writes preserve comments, formatting and unrelated content. Reread before updating a field, preserve unrelated external changes, and surface a same-field conflict instead of silently overwriting it. A failed save must be visible and must not be presented as successfully persisted. Changes to the file outside the app are detected and applied automatically when valid; the UI refreshes accordingly. Invalid or unreadable changes do not replace the last working active configuration.

Hotkey changes must be registered before replacing the previous working shortcut; an unavailable shortcut reports an error and is not saved by a UI edit. If a valid external edit cannot be applied, retain the working shortcut and clearly report that the file and active shortcut differ. Other runtime failures likewise must not be represented as successfully applied; explain the failure and preserve working state where possible.

## Invalid configuration and recovery

Never overwrite invalid user content with defaults. If a running app encounters an invalid or unreadable file, retain the last working configuration and show an actionable error. On launch with invalid config, start with safe defaults, open Settings automatically with a prominent explanation, and leave the file untouched. If Settings is closed while the issue remains, show a subtle menubar warning and a “Configuration needs attention” menu action that reopens Settings. Clear the warning only once the config is valid and successfully applied.

For syntactically parseable TOML, show valid settings and flag invalid fields; let users repair fields individually. Permit edits to other valid settings, preserving invalid content and comments; make it clear that those edits are saved but not active until the entire file validates. For malformed TOML, do not guess at field boundaries: show the parse error and offer to open the file for manual repair without rewriting it. Unknown keys remain errors, consistent with current validation; present them clearly without silently deleting them. If file access prevents writing, show an actionable error and leave it unchanged.

The existing canonical Regie config and legacy Rake fallback behavior must remain safe: reading the legacy file must not mutate it, and UI saves must not silently overwrite an invalid or inaccessible canonical file. Make clear which file needs attention.

## Verification and compatibility

Test each control against the existing parser, defaults and validation; live application to existing tabs and panel; hotkey conflict rollback; focus handling; singleton window and keyboard access; search, scroll navigation and reset actions; external edits and safe writes; invalid startup, parseable partial recovery, malformed TOML, inaccessible files and legacy fallback. Exercise Settings UI on macOS, including real hotkey registration and native window focus. Keep existing tests and session behavior intact.

This design changes the read-once and invalid-startup decisions in accepted ADR 0005. Record a new ADR when implemented; do not edit the accepted ADR. Update user documentation to distinguish implemented behavior from plans.

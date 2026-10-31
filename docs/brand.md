# Regie identity

**Regie**, without an accent, draws on French *régie*: a control room. The
identity is **Reveal**: quiet sage, forest and warm paper, with a descending cue.
The website is future work; this guide and its vector exports do not deploy one.

## Mark and exports

The **Descending Cue** has continuous brackets around a downward chevron and
cursor bar. Move the cue together, never the brackets: the approved translation
is `(0, 1.75)` in the 22-unit menu canvas and `(0, 8.75)` in the 128-unit app
concept, scaled 8× into the 1024 master. No separate optical redesign was made.

- [App SVG](../assets/icons/app.svg): forest `#2B4C3B` on a diagonal sage
  `#D5E4D1`→`#A2BCA9` tile, restrained light edge, transparent outer corners.
- [Template SVG](../assets/icons/menubar.svg): black with alpha, rounded 2.1-unit
  stroke, rendered at 22 and 44 pixels. macOS tints it; do not add a colored accent.
- [Standalone symbol](../assets/icons/symbol.svg) and
  [Regie lockup](../assets/icons/wordmark.svg). The lockup uses live Georgia/system
  serif text, not outlined lettering; appearance depends on installed fonts.
- [Generation and assets](../assets/icons/README.md): offline pinned WebKit;
  committed PNG masters and full 16–1024 [ICNS](../assets/icons/Regie.icns).

## Reveal controls

Identity anchors: sage `#BAD5BD`, forest `#1C2B25`, paper `#F4F0E7`.
Application controls use semantic tokens, not terminal colors:

| CSS role | Light | Dark |
|---|---|---|
| `--bar-bg` | `#F4F0E7` | `#1C2B25` |
| `--tab-fg` | `#42543F` | `#B2C5B5` |
| `--tab-active` | `#FFFCF5` | `#24382D` |
| `--tab-border` | `#CCD6C6` | `#354C3C` |
| `--tab-focus`, `--tab-accent` | `#3E6848` | `#BAD5BD` |

System dark and explicit dark share values. Keyboard outlines, a selected line
and surface, `aria-selected`, busy dots and ended text retain non-color state cues.
Labels meet 4.5:1 and focus/selected indicators 3:1 against their adjacent surfaces
in computed-style tests. Native menus and dialogs retain AppKit appearance.

## Typography

- Native controls and tabs: existing system sans; no bundled or downloaded fonts.
- Terminal: readable macOS Terminal font/size independently of color source, then
  existing `ui-monospace, Menlo, Monaco, monospace` fallback, without forced size.
- Future website/editorial study: Georgia/system serif for headings and wordmark,
  system sans for supporting copy, mono for shell examples. Not a terminal font.

## Candidate Regie Dark

`terminal_theme = "regie"` selects the candidate default, independent of
`appearance = "system" | "light" | "dark"`. This intentionally also changes
the default for legacy config files without a terminal-theme setting. Set
`terminal_theme = "macos"` to retain macOS Terminal profile colors. Missing profile
colors in that mode fall back to Regie Dark with a notice; readable font remains.

Base colors: background `#121615`, foreground `#DFE6DC`, cursor `#C6D8BE`, cursor
character `#121615`, selection `#344A3B` / `#F2F4ED`. The
[approved palette](superpowers/specs/2026-10-30-regie-brand-design.md#regie-dark-candidate-default)
records all sixteen independent ANSI slots. Truecolor/256-color programs are not
recolored. Cursor blink, bold/dim, selection and interaction policies are unchanged.

Pixel tests cover all 16 foreground/background colors in actual WebKit WebGL and
DOM fallback renderers, cursor, selection and UI appearance separation. This is
not proof of every application's contrast. Real `ag`, `tree`, `htop`, Claude Code
and OpenCode workload checks and native Finder/About/menubar checks remain
acceptance work before freezing the palette. Never alter shell/tool themes or
submit paid agent prompts merely to make a screenshot look right.

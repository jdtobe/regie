# Regie icons

**Descending Cue:** continuous brackets frame a down-chevron above a cursor bar.
The cue uses the approved downward spacing correction: 1.75 units in the 22-unit
template, 8.75 in the 128-unit app concept. Do not move the brackets with it.

| File | Purpose |
|---|---|
| `menubar.svg` | Black/alpha template master, 22-unit grid |
| `menubar-22.png`, `menubar-44.png` | Actual 1×/2× template assets |
| `app.svg`, `app-1024.png` | Sage tile, forest glyph, transparent outer corners |
| `Regie.icns` | Native 16–1024 representations |
| `symbol.svg` | Transparent standalone symbol |
| `wordmark.svg` | Plain Regie lockup, live Georgia/system-serif lettering, not outlined |

Regenerate offline on macOS with `node scripts/render-icons.js` after installing
the pinned development WebKit (`npm ci`, `npx playwright install webkit`). No
fonts or remote images are loaded. SVGs are durable sources; PNGs/ICNS are committed.
Runtime decodes `menubar-44.png` and `app-1024.png` with the same PNG helper.
macOS template tinting supplies black/white menubar appearance; no colored accent.

Tile gradient: `#D5E4D1`→`#A2BCA9`; symbol `#2B4C3B`. No baked outer shadow.
See the [approved geometry](../../docs/superpowers/specs/2026-10-30-regie-brand-design.md).

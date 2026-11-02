---
version: 1
slug: "src-window-rs"
primary_target: "src/window.rs"
related_targets: ["src/picker.rs"]
---

# Main window and screen picker

Mode: Operate. Native GTK3 desktop tool; code-first implementation.

## Direction contract

THESIS: Make pick, copy, and keep immediately accessible in one compact window.
OWN-WORLD: Honor the brief's GTK3 theme and controls. System sans for labels,
monospace for color values; sampled colors supply the expressive surface.
STORY: Pick a screen pixel, inspect or edit its values, copy a format, and save it.
FIRST VIEWPORT: Native header with the main Pick action, a large current swatch,
three aligned value/copy rows, save and adjust actions, Recent/Favorites tabs,
and an honest empty state. The magnifier shows discrete source pixels and a
contrasting selected-pixel frame, with keyboard precision.
FORM: The brief pins the GTK3 visual world. The seed cannot override native
controls or desktop theme; use the color-reference swatch tradition for density
and the magnified pixel grid for its signature. seed:2d3c37c2465982dc. The roll
service was unavailable; no external challenger boards were loaded.
FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance

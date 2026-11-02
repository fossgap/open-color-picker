---
name: Open Color Picker
description: A native GTK3 color reference with precise source-pixel picking.
colors:
  contrast-dark: "#000000"
  contrast-light: "#ffffff"
  picker-canvas: "rgb(8% 8% 9%)"
  magnifier-surface: "rgb(6% 6% 7% / 0.98)"
typography:
  color-entry:
    fontFamily: monospace
    fontSize: "14px"
  color-preview:
    fontFamily: monospace
    fontSize: "30px"
    fontWeight: 700
  magnifier-value:
    fontFamily: monospace
    fontSize: "17px"
    fontWeight: 700
  magnifier-coordinate:
    fontFamily: monospace
    fontSize: "11px"
    fontWeight: 700
spacing:
  palette-gap: "6px"
  compact: "8px"
  value-row: "10px"
  related: "12px"
  section: "16px"
  window: "24px"
components:
  main-window:
    width: "540px"
    height: "620px"
  content-column:
    padding: "{spacing.window}"
  current-swatch:
    height: "144px"
    typography: "{typography.color-preview}"
  color-value:
    typography: "{typography.color-entry}"
  palette-swatch:
    width: "44px"
    height: "36px"
  picker-canvas:
    backgroundColor: "{colors.picker-canvas}"
  picker-magnifier:
    width: "189px"
    height: "224px"
    backgroundColor: "{colors.magnifier-surface}"
    textColor: "{colors.contrast-light}"
  magnifier-pixel:
    size: "11px"
---

# Design System: Open Color Picker

## Overview

**Creative North Star: "The Desktop Color Reference"**

Open Color Picker is a compact Rust/GTK3 color reference for the Linux desktop. Native controls provide familiar actions and feedback; the selected color and discrete pixel magnifier supply the visual identity.

The desktop theme owns the surrounding interface. App-owned drawing stays factual: solid color samples, readable numeric values, and a precise indication of the selected source pixel.

**Key Characteristics:**

- GTK3 controls that follow the desktop's light or dark theme.
- Monospace color values beside ordinary system labels.
- Solid sampled-color surfaces and a discrete source-pixel magnifier.

Recorded from `src/main.rs`, `src/window.rs`, `src/picker.rs`, and `src/color.rs`, with light, dark, compact, populated, error, and picker captures in `.impeccable/review/`. The surface composition is documented in `.impeccable/surfaces/src-window-rs.md`.

## Colors

### Primary

The active GTK theme supplies the accent for suggested actions such as “Pick a color” and “Use color.” Sampled colors fill the current swatch and palette items; their values are runtime content.

### Neutral

The frontmatter records only Cairo's fixed neutrals: the dark picker surround, nearly opaque magnifier surface, and black/white contrast pair. Percentage RGB preserves the exact normalized Cairo channel values. Main-window backgrounds, text, borders, and errors have no application-owned fixed color.

The current swatch chooses black above a linear relative luminance of (0.179), otherwise white. The picker crosshair and center-cell outline combine a dark stroke (3px) with a light stroke (1px), so selection remains visible across captured colors.

**The Theme Ownership Rule.** Keep application chrome, action accents, error treatment, focus states, borders, and control shapes under GTK's theme. A selected or default sample is color data, not a brand token.

**The Source Pixel Rule.** Show and sample original captured pixels. Use nearest-neighbor scaling for the captured image and solid cells for magnification; never derive a selected value from an interpolated display pixel.

## Typography

The frontmatter's `color-entry` role is GTK CSS. The other three roles are Cairo text. System labels, window titles, instructions, status, and empty states inherit GTK typography; no fixed body family, body size, or display scale is imposed.

**The Native Type Rule.** Inherit the desktop's interface typography for labels, help, status, and header bars. Apply the recorded monospace roles only to numeric color information.

## Layout

Token measurements are application logical pixels. The main-window dimensions are startup requests, not locked dimensions; native decoration and theme requisitions can enlarge the outer window.

The content column uses the window gutter and section spacing. Action groups use compact spacing. Value rows use related column spacing and value-row spacing, with expanding entries requested at (24 characters). Status remains below the vertically scrolling content, aligned to its gutter. Error and status labels wrap with a maximum width request of (48 characters).

Recent and Favorites use homogeneous GTK FlowBoxes with palette-gap spacing and (1–8 items per row). Empty-state copy wraps at a maximum width request of (40 characters). Compact windows retain the same controls and scroll vertically; there are no custom viewport breakpoints.

The picker expands its image canvas and opens maximized from a default request of (1000 × 700px). The captured image is fitted without changing sampled source coordinates. Its magnifier contains (15 × 15 source pixels) using the recorded cell size and related spacing as its inner gutter; placement moves to avoid the canvas edges.

## Elevation & Depth

**The Native Depth Rule.** Let GTK and the window manager supply control and window elevation. The app's Cairo swatch and magnifier have no added shadow.

The magnifier is a flat overlay over the captured image. Separators and themed control boundaries organize the main window. There are no application-owned shadow or animation tokens.

## Shapes

Cairo samples, pixel cells, and the magnifier are rectangular with square corners. GTK owns the rounding and border treatment of buttons, entries, stack-switcher tabs, and dialogs; the app defines no fixed radius scale.

## Components

**Actions.** “Pick a color” and “Use color” use GTK's suggested-action style. Adjust, save/remove, cancel, and copy use standard themed buttons. Icons come from the system's symbolic icon theme. Tooltips and accessible names identify icon-only copy actions.

**Color values.** HEX, RGB, and HSL appear in aligned, editable monospace fields. Enter applies an edit. Invalid input retains the previous selected color, marks the field with GTK error styling, and displays a written format example. Copy actions report the copied value in the status line.

**Current color.** A solid swatch carries its centered HEX value using the color-preview role and luminance-selected foreground. Its accessible name includes the current value.

**Palettes.** Recent and Favorites are native stack-switcher pages. Each solid palette sample sits inside a themed button whose tooltip and accessible name include its HEX value. Save becomes “Remove favorite” when the current color is already saved. Empty pages explain how to populate them.

**Picker.** The header explicitly identifies a captured screen. Clicking a source pixel selects it; a visible readout reports HEX and source coordinates. The canvas takes initial focus: arrows move one source pixel, Shift+arrows move ten, and Enter confirms. Tab reaches the native buttons; their Enter/Space behavior remains native, and Escape cancels. The canvas accessible name and confirmation-button description update with the current value and coordinates.

**Documentation previews.** The sidecar contains static illustrations of the custom Cairo artwork using an observed sample color. Its HTML/CSS schema cannot encode GTK theme rendering, native focus/ATK behavior, or dynamic source pixels. Native controls remain specified by this document and the Rust source; no substitute control palette, tonal ramps, or web breakpoints are asserted.

## Do's and Don'ts

### Do:

- Do preserve GTK's suggested actions, symbolic icons, focus treatment, and light/dark theme behavior.
- Do pair sampled color with a readable value and an accessible name.
- Do preserve native button activation when focus leaves the picker canvas.
- Do keep errors and empty states written, visible, and actionable.

### Don't:

- Don't turn a sampled color or a screenshot's theme colors into fixed application chrome.
- Don't smooth source pixels in the picker or magnifier.
- Don't replace GTK controls with a custom web-style control system.

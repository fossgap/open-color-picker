<p align="center">
  <img src="data/io.github.opencolorpicker.OpenColorPicker.svg" width="128" height="128" alt="Open Color Picker icon">
</p>

<h1 align="center">Open Color Picker</h1>

<p align="center">
  <a href="https://github.com/fossgap/open-color-picker/actions/workflows/test-ci.yml"><img src="https://github.com/fossgap/open-color-picker/actions/workflows/test-ci.yml/badge.svg" alt="Test CI"></a>
  <a href="https://github.com/fossgap/open-color-picker/actions/workflows/release-ci.yml"><img src="https://github.com/fossgap/open-color-picker/actions/workflows/release-ci.yml/badge.svg" alt="Release CI"></a>
  <a href="https://github.com/fossgap/open-color-picker/releases"><img src="https://img.shields.io/github/v/release/fossgap/open-color-picker" alt="Latest release"></a>
  <a href="https://github.com/fossgap/open-color-picker/releases"><img src="https://img.shields.io/github/downloads/fossgap/open-color-picker/total" alt="Release downloads"></a>
</p>
<p align="center">
  <img src="https://img.shields.io/badge/Rust-2021-orange?logo=rust" alt="Rust 2021">
  <img src="https://img.shields.io/badge/GTK-3.24-green?logo=gtk" alt="GTK 3.24">
  <img src="https://img.shields.io/badge/Platform-Linux-blue?logo=linux" alt="Linux">
</p>

> Pick an exact pixel. Copy its value. Keep your favorite colors close at hand.

A native Linux color picker built with **Rust and GTK3**, part of
[Fossgap](https://github.com/fossgap). Capture screen colors on X11 or through
the desktop portal on Wayland, with a magnifier and a local color palette.

[Features](#features) · [Installation](#installation) · [Usage](#usage) · [Desktop support](#desktop-support) · [Building](#building) · [CI and releases](docs/ci-release.md)

![Open Color Picker with a selected color, HEX/RGB/HSL values, and recent swatches](docs/screenshot.png)

## Features

- **Pick precisely:** a 15 × 15 pixel magnifier, with arrow keys for one-pixel
  adjustments and a visible selection frame.
- **Copy in one click:** HEX, RGB, and HSL each have a copy button. Screen picks
  automatically copy HEX.
- **Edit colors:** type a value or open the native GTK color chooser.
- **Keep a palette:** favorites and the 24 most recent colors persist locally.
- **Stay within reach:** tray actions, application shortcuts, and a command for
  desktop-wide shortcut bindings.
- **Fit your desktop:** GTK3 controls follow your theme and font settings,
  including dark themes.

The project aims to make color picking consistent across Linux desktops while
respecting their screen-capture permissions. This is an initial implementation;
desktop support depends on the services described below.

## Installation

Get Linux **x86_64** packages from [Releases](https://github.com/fossgap/open-color-picker/releases)
or [build from source](#building). Release CI produces these
formats and verifies them in fresh containers before publishing:

| Format | Verification target | Install the downloaded package |
| --- | --- | --- |
| `.deb` | Debian 12 and Kali rolling | `sudo apt install ./open-color-picker_0.1.0_amd64.deb` |
| `.rpm` | Fedora 44 | `sudo dnf install ./open-color-picker-0.1.0-1.x86_64.rpm` |
| `.pkg.tar.zst` | Arch Linux | `sudo pacman -U ./open-color-picker-0.1.0-1-x86_64.pkg.tar.zst` |
| `.AppImage` | Ubuntu 24.04, without host GTK | `chmod +x OpenColorPicker-0.1.0-x86_64.AppImage` then run it |

Replace `0.1.0` with the downloaded version. Each release includes `SHA256SUMS`
and a source archive. To verify an individual download alongside `SHA256SUMS`:

```sh
sha256sum --ignore-missing -c SHA256SUMS
```

Native packages install the launcher and icon. AppImage is portable; it needs
glibc 2.36 or newer and desktop fonts. If FUSE is unavailable, launch it with
`APPIMAGE_EXTRACT_AND_RUN=1 ./OpenColorPicker-0.1.0-x86_64.AppImage`.

Optional desktop components:

| Capability | Requirements |
| --- | --- |
| Wayland capture | `xdg-desktop-portal` and a desktop-specific Screenshot backend |
| Tray icon | A desktop StatusNotifier tray host, such as KDE's system tray or GNOME's AppIndicator extension |

Tray support is implemented in Rust and does not require an AppIndicator
library. Install the portal backend for your desktop, such as GNOME, KDE, or
wlroots. An unrelated backend alone does not guarantee working capture.

## Building

Requirements: a Linux graphical session, Rust **1.85+**, a C linker,
`pkg-config`, GTK3 development libraries, and GdkPixbuf's SVG loader for the
application logo. Use [rustup](https://rustup.rs/)
if your distribution's Rust toolchain is older.

```sh
# Debian / Ubuntu
sudo apt install build-essential pkg-config libgtk-3-dev librsvg2-common

# Fedora
sudo dnf install gcc pkgconf-pkg-config gtk3-devel

# Arch Linux
sudo pacman -S --needed base-devel gtk3
```

Clone the repository and run:

```sh
git clone https://github.com/fossgap/open-color-picker.git
cd open-color-picker
cargo run --locked
```

Build an optimized binary and install the launcher and icon:

```sh
cargo build --release --locked
sudo make install PREFIX=/usr/local
open-color-picker
```

For a user-only install, use `make install PREFIX="$HOME/.local"` after building
and ensure `~/.local/bin` is on your `PATH`. Running from Cargo does not require
installation. There is **no Python dependency**.

## Usage

1. Choose **Pick a color**. The window hides before the screen is captured.
2. On Wayland, respond to the desktop's permission dialog if shown.
3. Move across the captured image to inspect pixels through the magnifier.
4. Click or press **Enter** to select and copy HEX. **Escape** cancels.
5. Copy another format, choose **Adjust color…**, or **Save color**.

The picker displays a **still capture**, not a live stream. The preview scales
to fit, but sampling reads original image bytes and the magnifier renders
solid source pixels without interpolation. Arrow keys can reach every source
pixel even when a large or multi-monitor capture is scaled down. Coordinates
refer to the captured image. Values are captured 8-bit RGB; the app does not
perform additional display-profile or HDR conversion.

The value fields accept these opaque color formats. Press Enter to apply:

| Format | Examples |
| --- | --- |
| HEX | `#3584E4`, `3584e4`, `#f80` |
| RGB | `rgb(53, 132, 228)`, `rgb(100%, 50%, 0%)` |
| HSL | `hsl(213, 76%, 55%)`, `hsl(-30deg, 100%, 50%)` |

Click a recent or favorite swatch to restore it. **Remove favorite** removes
the selected saved color. Invalid input and persistence failures are shown
in the window.

With a supported system tray, **left-click the tray icon to pick a color**.
**Right-click** opens the menu with Pick a color, Show colors, and Quit.
If the tray host disconnects, the main window returns so the app stays reachable.

### Keyboard shortcuts

Open **Application menu → Keyboard shortcuts** to customize application and
picker controls. Select a shortcut, press the new combination, then choose
**Set shortcut**. **Disable shortcut** removes a binding. **Save** applies your
changes immediately and keeps them for the next launch; **Cancel** discards
them. **Restore defaults** resets both tabs when you save.

Duplicate bindings are rejected. Tab navigation and standard text-editing
shortcuts remain available. Copy RGB, copy HSL, adjust, hide, and About can
also be assigned shortcuts. The following bindings are the defaults:

| Shortcut | Action |
| --- | --- |
| `Ctrl+P` | Pick from the screen |
| `Ctrl+Shift+C` | Copy current HEX |
| `Ctrl+S` | Save or remove the current favorite |
| `Ctrl+Q` | Quit, including the tray icon |
| `F1` | Configure keyboard shortcuts |
| Arrows, while picking | Move one source pixel |
| `Shift` + arrows | Move ten source pixels |
| `Enter` / `Escape`, while picking | Confirm / cancel |

Application shortcuts require focus. For a **global shortcut**, bind
`open-color-picker --pick` in your desktop's keyboard settings. Repeated
launches forward to the running instance through the session bus.

```sh
open-color-picker --pick     # Pick immediately
open-color-picker --tray     # Start hidden if a tray host is available
open-color-picker --quit     # Quit the running instance
open-color-picker --version
open-color-picker --help
```

**Application menu → About** opens the native GTK About
dialog with the app logo, version, project website, credits, and license
information. The logo is included in the executable, so it also appears when
running directly from Cargo.

## Desktop support

| Environment | Behavior |
| --- | --- |
| Native X11 | Captures the root window through GDK |
| Wayland, including XWayland | Uses the XDG Screenshot portal and the desktop's consent flow |
| No tray host or indicator library | Keeps `--tray` launches visible and explains why |

With an available tray, closing the window hides it; use **Quit** or `Ctrl+Q`
to exit. Without a tray, closing the window exits normally. GNOME may need a
StatusNotifier/AppIndicator extension to show tray icons.

If capture fails, check the session bus, `xdg-desktop-portal`, and your desktop
backend. Cancellation leaves the previous color unchanged. Requests time out
after two minutes. The implementation follows the official
[Screenshot API](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Screenshot.html)
and [Request lifecycle](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Request.html).

### Local data

Colors are stored in `$XDG_DATA_HOME/open-color-picker/palette.json`, defaulting
to `~/.local/share/open-color-picker/palette.json`. Saves use a temporary file
and atomic replacement. Invalid existing data is backed up before the next
save. The app does not persist screenshots; a portal may create its own capture
file. There are no accounts, telemetry, or application network calls.

Keyboard settings are stored separately in
`$XDG_CONFIG_HOME/open-color-picker/shortcuts.json`, defaulting to
`~/.config/open-color-picker/shortcuts.json`. Saves are atomic. An invalid
configuration falls back to defaults, shows an explanation, and is backed up
before you explicitly save new settings. If saving fails, the previous
shortcuts remain active.

## Packaging

The release image builds on Debian Bookworm with Rust 1.90. All package
formats contain the same tested release binary. Build and check locally with
the same Docker environment as GitHub Actions:

```sh
docker build -t open-color-picker-ci .
./scripts/ci.sh check

# From a committed, clean checkout; writes a new dist/release directory.
./scripts/ci.sh release
make verify-packages
```

For individual local packages, install the native build dependencies and use:

| Format | Command | Additional tools |
| --- | --- | --- |
| Debian / Ubuntu | `make deb` | `dpkg-dev` |
| RPM | `make rpm` | `rpm` / `rpm-build` |
| Arch | `make arch` | GNU tar, `zstd` |
| AppImage | `make appimage` | Use the Docker image for GTK runtime bundling |

These commands write to `dist/` and do not install or publish anything. AppImage
packaging downloads checksum-pinned tools. Native packages use the host GTK;
AppImage bundles GTK3, image loaders, schemas, icons, and an indicator library.
It leaves the display backend selection to GTK, including native Wayland.

See [Packaging](docs/packaging.md) for dependencies and artifacts, and
[CI and releases](docs/ci-release.md) for the verification matrix, local
commands, version tags, and publication gates.

## Development

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked

# Complete suite, including portal, GTK and packaging-script checks:
./scripts/ci.sh check
```

Build the Docker image first as shown above. The GUI smoke test uses a private Xvfb display, D-Bus session, and temporary
data directory. It needs `xvfb`, `dbus-run-session`, `xdotool`, `xsetroot`, and
`xclip`. The portal test uses a mock service in a separate session. Neither
test captures your actual desktop.

| Source | Responsibility |
| --- | --- |
| `src/main.rs` | Single-instance application and CLI |
| `src/window.rs` | GTK interface, clipboard, palette, tray |
| `src/capture.rs` | X11 capture and Wayland portal lifecycle |
| `src/picker.rs` | Captured-image view, magnifier, precision controls |
| `src/color.rs`, `src/pixels.rs` | Color conversion and pixel mapping |
| `src/store.rs` | Palette persistence and recovery |
| `src/shortcuts.rs`, `src/preferences.rs` | Shortcut configuration, persistence, and native dialogs |
| `data/`, `packaging/`, `scripts/` | Desktop integration, packages, checks |

GTK3 bindings are intentional to meet the project's native GTK3 requirement.
Keep desktop-specific behavior at the capture/tray boundary, and check UI
changes in both light and dark GTK themes.

## Powered by

- [Rust](https://www.rust-lang.org/) and [gtk-rs GTK3 bindings](https://github.com/gtk-rs/gtk3-rs)
- [GTK](https://www.gtk.org/) and [XDG Desktop Portal](https://github.com/flatpak/xdg-desktop-portal)
- [AppImage](https://appimage.org/) for portable Linux packaging

## Contributing

Report problems or propose improvements in [Issues](https://github.com/fossgap/open-color-picker/issues).
For desktop problems, include the distribution, desktop environment, display
session (X11 or Wayland), and package format. Run the CI checks before opening
a pull request. Package-installation logs are retained by Release CI.

## License

A project license has not been selected yet. Package metadata uses
`LicenseRef-Not-Specified`; this is not a license grant. Bundled third-party
runtime components retain their own copyright and licensing notices.

# Development

Open Color Picker is a native Linux application using Rust 1.85+ and GTK3.
Use [rustup](https://rustup.rs/) if your distribution has an older Rust toolchain.

## Build and run

Install a C compiler, pkg-config, GTK3/GStreamer development libraries, and capture plugins:

```sh
# Debian / Ubuntu / Kali
sudo apt install build-essential pkg-config libgtk-3-dev librsvg2-common \
  libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
  gstreamer1.0-plugins-base gstreamer1.0-pipewire

# Fedora
sudo dnf install gcc pkgconf-pkg-config gtk3-devel gstreamer1-devel \
  gstreamer1-plugins-base-devel gstreamer1-plugins-base pipewire-gstreamer

# Arch Linux
sudo pacman -S --needed base-devel gtk3 gst-plugins-base gst-plugin-pipewire
```

Tray support needs a StatusNotifier host, such as KDE's system tray or GNOME's
AppIndicator extension. Left-click the icon to pick a color; right-click opens
the menu. If the tray host disconnects, the main window returns. Screen capture
on composited X11 uses xdg-desktop-portal's ScreenCast backend and PipeWire for
live frames when available, with direct X11 capture as the frozen fallback.
Wayland uses the Screenshot portal and a frozen desktop overlay: its fullscreen
protocol hides the desktop behind transparent windows, so the live overlay is
not supported there. The ScreenCast permission dialog is skipped on Wayland.
Cancelling the capture permission dialog cancels the pick.

```sh
git clone https://github.com/fossgap/open-color-picker.git
cd open-color-picker
cargo run --locked
```

Build and install the optimized binary, launcher, and icon:

```sh
cargo build --release --locked
sudo make install PREFIX=/usr/local
open-color-picker
```

For a user-only install, use `make install PREFIX="$HOME/.local"` and put
`~/.local/bin` on your PATH. No installation is needed for `cargo run`.

## Commands and local data

```sh
open-color-picker --pick     # Pick immediately
open-color-picker --tray     # Start hidden when a tray is available
open-color-picker --quit     # Quit the running instance
open-color-picker --version
open-color-picker --help
```

Application shortcuts require focus. A desktop-wide shortcut can invoke
`open-color-picker --pick`; repeated launches forward to the running instance.

Colors are saved to `$XDG_DATA_HOME/open-color-picker/palette.json`, defaulting
to `~/.local/share/open-color-picker/palette.json`. Shortcuts are saved to
`$XDG_CONFIG_HOME/open-color-picker/shortcuts.json`, defaulting to
`~/.config/open-color-picker/shortcuts.json`. Saves are atomic, and invalid
existing data is backed up before replacement. Screenshots are not persisted
by the application; a screenshot portal may create its own capture file.

Color fields accept `#3584E4`, `#f80`, `rgb(53, 132, 228)`, `rgb(100%, 50%, 0%)`,
and `hsl(213, 76%, 55%)`. Color syntax and saved settings are language-independent.
Picking samples 8-bit RGB capture pixels without additional display-profile
or HDR conversion. On supported X11 desktops, transparent full-screen overlays keep the desktop live,
with a source-pixel magnifier and keyboard controls. Overlay graphics are
masked out of captured frames to prevent feedback into the magnifier.
If live capture cannot start, a frozen desktop overlay opens instead. If a
running stream stops, the last frame stays available for picking. Both modes
use desktop coordinates, including on HiDPI displays; monitor layout changes
cancel the current pick. Only monitors selected in the sharing dialog are
available during live picking. Sharing ends when picking finishes.

## Checks

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
LC_ALL=C.UTF-8 LANGUAGE=C cargo test --locked
```

Run the complete suite in the supplied Docker image:

```sh
docker build -t open-color-picker-ci .
./scripts/ci.sh check
```

This also checks screen capture, keyboard controls, shortcut persistence,
native dialogs, all six translations, shell scripts, and desktop integration.
GUI tests use private Xvfb displays, D-Bus sessions, and temporary data, never
your desktop or settings. Native Wayland fallback and its composited output are
checked in a private Weston compositor as well. See [CI and releases](ci-release.md) for dependencies
and package verification, and [translations](translations.md) for locale tests.

Check interface changes in both light and dark GTK themes. Real Wayland
permission dialogs and tray hosts also need desktop testing.

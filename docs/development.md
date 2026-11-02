# Development

Open Color Picker is a native Linux application using Rust 1.85+ and GTK3.
Use [rustup](https://rustup.rs/) if your distribution has an older Rust toolchain.

## Build and run

Install a C compiler, pkg-config, GTK3 development libraries, and SVG support:

```sh
# Debian / Ubuntu
sudo apt install build-essential pkg-config libgtk-3-dev librsvg2-common libayatana-appindicator3-1

# Fedora
sudo dnf install gcc pkgconf-pkg-config gtk3-devel libayatana-appindicator-gtk3

# Arch Linux
sudo pacman -S --needed base-devel gtk3 libayatana-appindicator
```

The AppIndicator library is optional unless you want the system tray. Screen
capture on Wayland uses xdg-desktop-portal and your desktop's Screenshot backend.

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
Picking samples original 8-bit RGB pixels from a still capture, without
additional display-profile or HDR conversion.

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
your desktop or settings. See [CI and releases](ci-release.md) for dependencies
and package verification, and [translations](translations.md) for locale tests.

Check interface changes in both light and dark GTK themes. Real Wayland
permission dialogs and tray hosts also need desktop testing.

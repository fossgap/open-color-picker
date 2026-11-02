<p align="center">
  <img src="data/io.github.opencolorpicker.OpenColorPicker.svg" width="128" height="128" alt="Open Color Picker icon">
</p>

<h1 align="center">Open Color Picker</h1>

<p align="center">
  <a href="https://github.com/fossgap/open-color-picker/actions/workflows/test-ci.yml"><img src="https://github.com/fossgap/open-color-picker/actions/workflows/test-ci.yml/badge.svg" alt="Test CI"></a>
  <a href="https://github.com/fossgap/open-color-picker/actions/workflows/release-ci.yml"><img src="https://github.com/fossgap/open-color-picker/actions/workflows/release-ci.yml/badge.svg" alt="Release CI"></a>
  <a href="https://github.com/fossgap/open-color-picker/releases"><img src="https://img.shields.io/github/v/release/fossgap/open-color-picker" alt="Latest release"></a>
</p>

Pick a color from your Linux desktop, copy its value, and save your favorites.
Built with Rust and GTK, part of [Fossgap](https://github.com/fossgap).

![Open Color Picker with HEX, RGB, HSL, and recent colors](docs/screenshot.png)

- Precise screen picking with a magnifier and keyboard controls.
- Copy or edit HEX, RGB, and HSL values.
- Keep favorites and recent colors between sessions.
- Configurable shortcuts and a system tray menu.
- Follows your desktop theme, including dark mode.

## Install

Download a Linux **x86_64** package from
[Releases](https://github.com/fossgap/open-color-picker/releases/latest):
**DEB**, **RPM**, **Arch**, or **AppImage**.

Install a native package with your distribution’s package manager, then launch
**Open Color Picker** from your applications menu or run `open-color-picker`.
For AppImage, make the file executable and open it.

Wayland screen capture needs your desktop’s screenshot portal. Tray support
needs an AppIndicator-compatible tray; GNOME may need an extension.
See [package details](docs/packaging.md) if needed.

## Use

1. Choose **Pick a color** and allow screen capture if prompted.
2. Point at a pixel; use the arrow keys for precise adjustments.
3. Click or press **Enter** to select and copy HEX. **Escape** cancels.
4. Copy another format, edit a value, or choose **Save color**.

Press **F1** to customize keyboard shortcuts. For a desktop-wide shortcut,
bind `open-color-picker --pick` in your desktop’s keyboard settings.

The tray menu offers **Pick a color**, **Show colors**, and **Quit**.
Closing the window keeps the app in the tray when one is available.

## Languages

The app follows your desktop language: **English, French, Spanish, German,
Portuguese, Simplified Chinese, and Japanese**. Unsupported languages use English.

## Build and contribute

With Rust and the GTK3 development libraries installed, run `cargo run --locked`.
See [development](docs/development.md), [translations](docs/translations.md),
[packaging](docs/packaging.md), and [CI and releases](docs/ci-release.md).

Report bugs or suggest improvements in
[Issues](https://github.com/fossgap/open-color-picker/issues).

## License

A project license has not been selected yet. Dependencies retain their own licenses.

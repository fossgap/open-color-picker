# Open Color Picker

<!-- impeccable:product-schema 1 -->

## Platform

Native Linux desktop, using GTK3 as required by the original README.

## Stack

User requirement: Rust, with GTK3. Cargo manages the Rust dependencies; system
packages supply native GTK3 libraries. The application has no Python runtime.

## Product Purpose

Pick accurate screen colors, copy HEX/RGB/HSL values, and keep favorites close
at hand across Linux desktop environments.

## Users

Assumption: designers and developers working with color in desktop applications.
No narrower audience has been specified.

## Capabilities and Constraints

The README requires a magnifier, favorites, keyboard shortcuts, tray access,
X11 and Wayland support, GTK3 styling, and deb/rpm/AppImage/Arch packaging.
Wayland capture requires the user's desktop Screenshot portal and its consent
flow. Tray availability depends on the desktop's indicator host. Do not claim
universal desktop compatibility without testing each environment.

## Brand Commitments

Name: Open Color Picker. Preserve the requested GTK3 look and feel.

## Evidence on Hand

The original README is the product brief. There were no source files, release
artifacts, screenshots, repository remote, or licensing terms at implementation
start. Distribution and license decisions remain open.

## Product Principles

- Sample source pixels without interpolation; distinguish capture from live view.
- Keep picking, copying, and saving accessible by keyboard.
- Preserve useful work locally and report persistence failures.
- Respect the user's desktop theme and capture permissions.

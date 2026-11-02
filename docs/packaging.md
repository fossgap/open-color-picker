# Packaging

Open Color Picker follows the package-and-verify workflow used by
[fossgap/open-data-recovery](https://github.com/fossgap/open-data-recovery).
The supported release architecture is Linux x86_64. The Docker build uses
Rust 1.90 and Debian Bookworm, keeping the glibc baseline at 2.36. Building
on a newer host may produce packages that require newer system libraries.

## Release artifacts

For version `0.1.0`, `dist/release/` contains:

| Artifact | Contents |
| --- | --- |
| `open-color-picker_0.1.0_amd64.deb` | Debian package with computed shared-library dependencies |
| `open-color-picker-0.1.0-1.x86_64.rpm` | RPM with automatic ELF dependencies and GTK3 requirement |
| `open-color-picker-0.1.0-1-x86_64.pkg.tar.zst` | Arch binary package with GTK3, glibc and GCC runtime requirements |
| `OpenColorPicker-0.1.0-x86_64.AppImage` | Portable app and bundled GTK3 runtime |
| `open-color-picker-0.1.0-source.tar.gz` | Exact committed source from `git archive HEAD` |
| `SHA256SUMS` | SHA-256 digests covering all five artifacts |

Native packages install `/usr/bin/open-color-picker`, a desktop launcher,
an SVG icon, and documentation. All formats use one release binary, tested
before packaging. Package names and embedded versions come from `Cargo.toml`;
the binary's `--version` must agree. The source archive includes `Cargo.lock`
but does not vendor crates; its first Cargo build needs registry access.

No project license has been selected. RPM and Arch metadata explicitly use
`LicenseRef-Not-Specified`. This repository does not infer a license from
other Fossgap projects.

## Build locally

Use the same disposable environment as CI:

```sh
docker build -t open-color-picker-ci .
./scripts/ci.sh check
./scripts/ci.sh release
```

Release mode requires a clean, committed checkout and a new output directory.
The runner rejects an image whose input checksums differ from the checkout;
rebuild the image after source changes. Override the image with `OCP_CI_IMAGE`
or choose another fresh directory with `OCP_CI_OUTPUT`. Neither command
publishes anything. See [CI and releases](ci-release.md) for the full process.

Individual builds write to `dist/`:

```sh
make deb
make rpm
make arch
make appdir
# Once an AppDir exists, pack it without rebuilding:
./scripts/appimage.sh
# Or build both in one step from a fresh AppDir:
make appimage
```

Each `make` package target builds the release binary first. To package a
binary already built and tested, set `OCP_BINARY=/absolute/path/to/open-color-picker`.
It must match `Cargo.toml`'s version. Native builders need `dpkg-dev` for DEB,
`rpm` / `rpm-build` for RPM, or GNU tar and `zstd` for Arch. Arch packaging
creates a binary package directly; it does not invoke `makepkg` or publish
an AUR recipe. RPM packaging stages the binary without recompiling it.

## AppImage runtime

Build AppImage inside the supplied Debian image. `bundle-runtime.sh` includes
GTK3's libraries, GIO modules, image loaders, input modules, schemas, MIME
data, Adwaita icons, Fontconfig configuration, and Ayatana AppIndicator.
The launcher resolves these relative to the AppDir. It does not force X11;
GTK chooses the display backend normally. The host supplies glibc, the
dynamic loader, fonts, and desktop services such as the Screenshot portal.

`scripts/appimage.sh` downloads AppImage's `appimagetool` 1.9.1 and type-2
runtime to `target/appimage-tools/`. Both assets are checked against SHA-256
pins before use, including cached copies. The continuous runtime URL is
mutable: if upstream replaces it, a hash mismatch fails the build. Review
the upstream asset and update the pin deliberately. Do not bypass validation.
Tool extraction and verification use `--appimage-extract`, so CI needs no FUSE
device or privileged container. Third-party runtime notices are stored in
`usr/share/doc/open-color-picker/runtime/` inside the AppImage.

An existing `dist/OpenColorPicker.AppDir` is never overwritten automatically.
Remove that generated directory explicitly before rebuilding it.

## Verify installations

```sh
./scripts/verify-release.sh dist/release
./scripts/check-package-installation.sh debian dist/release
./scripts/check-package-installation.sh kali dist/release
./scripts/check-package-installation.sh fedora dist/release
./scripts/check-package-installation.sh arch dist/release
./scripts/check-package-installation.sh appimage dist/release
# Or run the entire matrix:
make verify-packages
```

The checker pulls a fresh distribution image, logs its resolved digest,
mounts artifacts and test scripts read-only, and removes its container on
exit. Package-manager operations happen only inside that container. The
application runs as an ordinary user on a private Xvfb display and private
D-Bus session. Tests cover version/help, desktop integration, screen capture,
cancellation, exact clipboard values, favorites, persistence, single-instance
activation, and visible fallback when a tray host is unavailable.

Dependencies are installed by the native package manager before test tools;
the CLI must already work at that point. AppImage is checked on Ubuntu 24.04
without host GTK, and its embedded executable is compared byte-for-byte
with the DEB's executable. Hashes establish artifact integrity, not an
independent publisher signature.

These checks exercise X11 under Xvfb. A separate mock service tests Wayland
portal responses and cancellation, but real GNOME/KDE/wlroots permission
dialogs, compositor scaling, and a real tray host still need desktop testing.

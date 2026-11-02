# CI and releases

The workflows follow [Fossgap's Open Data Recovery](https://github.com/fossgap/open-data-recovery)
structure: Test CI, Release CI, disposable package-installation checks, then
publication. All actions are pinned to commit SHAs. Jobs default to read-only
repository access; only the final publish job has `contents: write`.

## Test CI

`.github/workflows/test-ci.yml` runs on pushes to `main`, pull requests, and
manual dispatch. It builds the Docker image, then runs `scripts/ci.sh check`:

1. Rust formatting, Clippy with warnings denied, and unit tests.
2. ShellCheck, shell syntax, desktop-file validation, and release-guard tests.
3. Portal capture, cancellation, invalid responses, and the magnified picker
   with default and configured shortcuts in private D-Bus/Xvfb sessions.
   Synthetic live video verifies changing colors, transparent overlays,
   stream shutdown, frozen fallback, and portal session cleanup.
   A private Weston compositor checks native Wayland's opaque frozen
   overlay and ensures unsupported live picking never asks for screen sharing.
4. The ignored picker keyboard/accessibility test under private Xvfb and D-Bus.
5. Shortcut configuration and dialog tests: remapping, conflicts, disabling,
   cancel/reset, persistence, write failures, picker controls, and About metadata.
6. Localized GTK tests in French, Spanish, German, Portuguese, Simplified
   Chinese, and Japanese, including native GTK controls and shortcut conflicts.
7. A mock StatusNotifier host checks left-click activation, translated tray
   menus, menu actions, and recovery when the tray host disconnects/reconnects.
8. A GUI smoke test covering the complete pick/copy/save/restart workflow.
9. An end-to-end X11 test of the unmodified application with real Mutter,
   GNOME ScreenCast portals, PipeWire, and WirePlumber in a private Xvfb session.
   It accepts/cancels the actual consent dialog, checks live desktop and
   magnifier pixels, selects and copies a changing color, verifies session
   cleanup, stops PipeWire to check frozen fallback, and restarts the app to
   verify persistence. This path uses no mock portal or substitute video source.

The release-guard tests ensure changed artifacts, stale/missing files,
incomplete/duplicate checksum entries, a mismatched tag, and accidental
host execution of the installer are rejected. Tests use temporary app data
and do not access the user's desktop, palette, or shortcut configuration.

## Release CI

`.github/workflows/release-ci.yml` runs on `v*` tag pushes or manual dispatch.
The build job repeats the full test suite, compiles an optimized binary,
smoke-tests that binary, creates four package formats, smoke-tests AppImage,
and adds a committed source archive and `SHA256SUMS`.
The real X11 end-to-end test also runs against the optimized binary and the
AppImage before the packages can be published.

The version must be `x.y.z` in `Cargo.toml`. A tag must exactly match `v` plus
that version. Release creation refuses dirty or untracked source files and
an existing output directory, avoiding mismatches and stale artifacts.

Five parallel jobs install and test the resulting artifacts:

| Target | Container image | Package |
| --- | --- | --- |
| Debian | `debian:bookworm-slim` | DEB |
| Kali | `kalilinux/kali-rolling:latest` | The same DEB |
| Fedora | `registry.fedoraproject.org/fedora:44` | RPM |
| Arch | `archlinux:base` | `.pkg.tar.zst` |
| AppImage | `ubuntu:24.04` without GTK | AppImage, without FUSE |

Images are refreshed per check and their digests are logged. This deliberately
tests current distribution packages; a rolling distribution change may reveal
a compatibility issue. The matrix continues other targets after a failure,
and retains each target's log even when its installation fails. Logs and
build artifacts remain available as Actions artifacts for 14 days.

Only a **tag push** can publish, after the build and all five verification
jobs succeed. The publish job downloads those tested artifacts, checks their
hashes again, and creates a release with generated notes using `GITHUB_TOKEN`.
It does not rebuild packages or overwrite an existing release. No additional
repository secret is required. **Manual dispatch never publishes**, including
when it is launched against a tag.

## Run the same checks locally

Docker and an x86_64 Linux host are required. No application package is
installed on the host:

```sh
docker build -t open-color-picker-ci .
./scripts/ci.sh check

# After committing source changes, build a fresh image again:
docker build -t open-color-picker-ci .
./scripts/ci.sh release
make verify-packages
```

For a different generated output directory:

```sh
OCP_CI_OUTPUT=dist/release-review ./scripts/ci.sh release
./scripts/check-package-installation.sh debian dist/release-review
```

To run checks directly on a development machine, use `./scripts/check.sh`
with the dependencies listed in `Dockerfile`. This requires Rust, GTK3/GStreamer
development headers, ShellCheck, desktop-file-utils, and the Xvfb/D-Bus/X11
test tools, the six UTF-8 locales generated in the image, and CJK fonts.
It still uses isolated test sessions.

To rerun just the real X11 end-to-end test in the CI environment:

```sh
cargo build --locked
OCP_E2E_OUTPUT=build/x11-e2e-review ./scripts/check-x11-e2e.sh
```

The optional output directory retains portal/application logs and actual
composited screenshots. The test uses a single virtual 1280×900 monitor and
software rendering with Mutter 43 on Debian 12. Physical GPUs, other portal
backends, and mixed-scale multi-monitor desktops still need desktop testing.

## Maintainer release procedure

1. Update the package version in `Cargo.toml` and regenerate `Cargo.lock`.
2. Run the check suite, commit the intended source, rebuild the Docker image,
   and run the release build and all package-installation checks.
3. Push a matching tag, such as `v0.1.3`, when ready to publish.
4. Inspect the Release CI logs and the resulting release assets.

Source archives represent `HEAD`. The CI runner compares a checksum manifest
of source, documentation, build scripts and workflows with the image before
starting; stale images are rejected. Rebuild the CI image after changing
these files. GitHub Actions always builds its image
from the checked-out revision. See [Packaging](packaging.md) for artifact
contents, AppImage tool pins, licensing metadata, and runtime limitations.

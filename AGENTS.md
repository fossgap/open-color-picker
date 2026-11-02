# AGENTS.md

## Architecture

- Single Rust 2021 binary using GTK3 on Linux; Rust 1.85+ is required. `src/main.rs` owns the single-instance `gtk::Application` and forwards repeated CLI launches to it.
- `src/window.rs` wires the main UI and tray; `src/capture.rs` handles X11 capture and the Wayland Screenshot portal; `src/picker.rs` handles pixel selection. `src/store.rs` and `src/shortcuts.rs` implement versioned, atomic JSON persistence with corrupt-file backups.
- UI tests live in `src/window_tests.rs`; other tests are colocated in their modules. Package-installation tests are shell scripts under `tests/package-install/`.

## Commands

```sh
cargo run --locked
make check
LC_ALL=C.UTF-8 LANGUAGE=C cargo test --locked
cargo test --locked i18n::tests
```

- `make check` runs only rustfmt and Clippy; run tests separately. Clippy denies warnings, and Cargo commands use `--locked`.
- `./scripts/check.sh` is the full host suite: formatting, Clippy, unit tests, ShellCheck and syntax checks, desktop validation, release guards, isolated portal/GTK/locale tests, and a GUI smoke test. It requires the dependencies and six generated locales listed in `Dockerfile`.
- Prefer the CI environment for complete verification:

```sh
docker build -t open-color-picker-ci .
./scripts/ci.sh check
```

Rebuild the image after changing any input covered by `scripts/source-manifest.sh`; `ci.sh` rejects stale images.

## Test isolation

- Plain `cargo test --locked` skips ignored portal and GTK tests. Do not run all ignored tests directly on a developer desktop: use `scripts/check.sh`, which provides private Xvfb/D-Bus sessions, X11 settings, and temporary XDG data/config directories.
- GUI changes need light- and dark-theme review. Real Wayland permission dialogs, compositor scaling, and AppIndicator tray hosts still require desktop testing; Xvfb and mock portal tests do not cover them.

## Change-specific rules

- New UI text needs a typed `Text` key and English fallback in `src/i18n.rs`, plus every catalog in `data/i18n/`. Preserve named placeholders such as `{action}` and `{color}`. Launcher translations in `data/io.github.opencolorpicker.OpenColorPicker.desktop` are separate; native GTK controls use system catalogs.
- Use the supplied Bookworm Docker image for release packages to retain the glibc 2.36 baseline; supported release artifacts are Linux x86_64 only. `scripts/ci.sh release` requires a clean committed checkout and a nonexistent output directory.
- Keep GitHub Actions pinned to full commit SHAs. Workflow permissions are read-only except for the final tag-triggered publish job; manual dispatch never publishes.
- No project license has been selected. Do not infer or add one; RPM and Arch metadata deliberately use `LicenseRef-Not-Specified`.

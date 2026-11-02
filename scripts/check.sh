#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
shellcheck scripts/*.sh packaging/AppRun tests/package-install/*.sh
for script in scripts/*.sh tests/package-install/*.sh; do bash -n "$script"; done
sh -n packaging/AppRun
desktop-file-validate data/*.desktop
./scripts/test-release-checks.sh
dbus-run-session --config-file=scripts/test-bus.conf -- cargo test --locked portal_round_trip -- --ignored
test_data=$(mktemp -d)
trap 'rm -rf -- "$test_data"' EXIT
for test_name in picker_keyboard shortcuts_configuration preferences_dialogs tray_dbus; do
  xvfb-run -a dbus-run-session --config-file=scripts/test-bus.conf -- \
    env GDK_BACKEND=x11 XDG_SESSION_TYPE=x11 GTK_USE_PORTAL=0 NO_AT_BRIDGE=1 \
      XDG_DATA_HOME="$test_data/data" XDG_CONFIG_HOME="$test_data/config" \
      cargo test --locked "$test_name" -- --ignored
done
cargo build --locked
./scripts/smoke-test.sh

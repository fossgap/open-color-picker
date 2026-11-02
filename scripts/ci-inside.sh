#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mode=${1:-check}
case "$mode" in check|release) ;; *) exit 2 ;; esac
./scripts/check.sh
if [[ "$mode" == release ]]; then
  cargo build --release --locked
  export OCP_BINARY="$PWD/target/release/open-color-picker"
  ./scripts/smoke-test.sh
  for format in deb rpm arch appimage; do ./scripts/package.sh "$format"; done
  APPIMAGE_EXTRACT_AND_RUN=1 OCP_BINARY="$PWD/dist/OpenColorPicker-$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)-x86_64.AppImage" \
    ./scripts/smoke-test.sh
  mkdir dist/release
  mv dist/*.deb dist/*.rpm dist/*.pkg.tar.zst dist/*.AppImage dist/release/
fi

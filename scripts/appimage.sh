#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
[[ $(uname -m) == x86_64 ]] || { echo 'The pinned AppImage toolchain targets x86_64.' >&2; exit 1; }
[[ -d dist/OpenColorPicker.AppDir ]] || { echo 'Build the AppDir first: make appdir' >&2; exit 1; }
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)
cache="$PWD/target/appimage-tools"
mkdir -p "$cache"
# Match the verified toolchain used by fossgap/open-data-recovery. Downloads
# fail closed on a changed asset, including the upstream continuous runtime.
tool_version=1.9.1
tool_hash=ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0
runtime_hash=617aa77314d57c8b8e4fae9c3f0b35af7b30d5529cb69e98f64f92510ed98a46
fetch_verified() {
  local url=$1 output=$2 digest=$3
  if [[ ! -f "$output" ]]; then
    curl -fL --retry 2 "$url" -o "$output.download"
    printf '%s  %s\n' "$digest" "$output.download" | sha256sum --strict -c -
    mv "$output.download" "$output"
  fi
  printf '%s  %s\n' "$digest" "$output" | sha256sum --strict -c -
}
tool="$cache/appimagetool-$tool_version.AppImage"
fetch_verified "https://github.com/AppImage/appimagetool/releases/download/$tool_version/appimagetool-x86_64.AppImage" "$tool" "$tool_hash"
fetch_verified https://github.com/AppImage/type2-runtime/releases/download/continuous/runtime-x86_64 "$cache/runtime-x86_64" "$runtime_hash"
chmod +x "$tool"
mkdir -p "$cache/appimagetool-$tool_version"
(cd "$cache/appimagetool-$tool_version" && "$tool" --appimage-extract >/dev/null)
ARCH=x86_64 "$cache/appimagetool-$tool_version/squashfs-root/AppRun" \
  --no-appstream --runtime-file "$cache/runtime-x86_64" \
  --mksquashfs-opt -processors --mksquashfs-opt 2 \
  dist/OpenColorPicker.AppDir "dist/OpenColorPicker-$version-x86_64.AppImage"

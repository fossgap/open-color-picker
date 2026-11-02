#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
root=$PWD
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)
fixture=$(mktemp -d)
trap 'rm -rf -- "$fixture"' EXIT
files=("open-color-picker_${version}_amd64.deb" "open-color-picker-${version}-1.x86_64.rpm"
  "open-color-picker-${version}-1-x86_64.pkg.tar.zst" "OpenColorPicker-${version}-x86_64.AppImage"
  "open-color-picker-${version}-source.tar.gz")
for artifact in "${files[@]}"; do printf 'test fixture for %s\n' "$artifact" > "$fixture/$artifact"; done
(cd "$fixture" && sha256sum "${files[@]}" > SHA256SUMS)
"$root/scripts/verify-release.sh" "$fixture" >/dev/null
# Write logs outside the artifact directory so they cannot cause false failures.
reject() {
  local log
  log=$(mktemp)
  if "$@" >"$log" 2>&1; then cat "$log" >&2; rm "$log"; echo "Expected failure: $*" >&2; exit 1; fi
  rm "$log"
}
reject_message() {
  local expected=$1 output
  shift
  if output=$("$@" 2>&1); then echo "Expected failure: $*" >&2; exit 1; fi
  grep -F "$expected" <<< "$output" >/dev/null || {
    printf 'Unexpected failure for %s:\n%s\n' "$*" "$output" >&2; exit 1;
  }
}
printf 'tampered\n' >> "$fixture/${files[0]}"
reject "$root/scripts/verify-release.sh" "$fixture"
printf 'test fixture for %s\n' "${files[0]}" > "$fixture/${files[0]}"
cp "$fixture/${files[0]}" "$fixture/stale.deb"
reject "$root/scripts/verify-release.sh" "$fixture"
rm "$fixture/stale.deb"
mv "$fixture/${files[1]}" "$fixture/missing.rpm"
reject "$root/scripts/verify-release.sh" "$fixture"
mv "$fixture/missing.rpm" "$fixture/${files[1]}"
sed -i '$d' "$fixture/SHA256SUMS"
reject "$root/scripts/verify-release.sh" "$fixture"
(cd "$fixture" && sha256sum "${files[@]}" > SHA256SUMS)
first_hash=$(head -1 "$fixture/SHA256SUMS")
printf '%s\n' "$first_hash" >> "$fixture/SHA256SUMS"
reject "$root/scripts/verify-release.sh" "$fixture"
reject_message 'Release tag must match Cargo.toml version.' env GITHUB_REF_TYPE=tag GITHUB_REF_NAME=v999.0.0 "$root/scripts/ci.sh" release
reject_message 'Use scripts/check-package-installation.sh' "$root/tests/package-install/install.sh" deb "$version"
echo 'Release guards passed: tampering, stale/missing artifacts, incomplete/duplicate hashes, tag mismatch and host-install refusal.'

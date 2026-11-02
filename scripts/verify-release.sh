#!/usr/bin/env bash
# Check the complete release set before installing or uploading its artifacts.
set -euo pipefail
cd "$(dirname "$0")/.."
artifacts=$(realpath "${1:-dist/release}")
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)
expected=(
  "open-color-picker_${version}_amd64.deb"
  "open-color-picker-${version}-1.x86_64.rpm"
  "open-color-picker-${version}-1-x86_64.pkg.tar.zst"
  "OpenColorPicker-${version}-x86_64.AppImage"
  "open-color-picker-${version}-source.tar.gz"
)
cd "$artifacts"
for artifact in "${expected[@]}" SHA256SUMS; do
  [[ -s "$artifact" && ! -L "$artifact" ]] || { echo "Missing or invalid release artifact: $artifact" >&2; exit 1; }
done
# Reject stale versions, unexpected files, omitted hashes and duplicated entries.
diff -u <(printf '%s\n' "${expected[@]}" SHA256SUMS | LC_ALL=C sort) \
  <(find . -mindepth 1 -maxdepth 1 -printf '%f\n' | LC_ALL=C sort)
while IFS= read -r line || [[ -n "$line" ]]; do
  [[ "$line" =~ ^[a-fA-F0-9]{64}\ \ (\./)?[a-zA-Z0-9][a-zA-Z0-9._-]*$ ]] || { echo 'Malformed checksum entry.' >&2; exit 1; }
done < SHA256SUMS
diff -u <(printf '%s\n' "${expected[@]}" | LC_ALL=C sort) \
  <(awk '{print $2}' SHA256SUMS | sed 's|^\./||' | LC_ALL=C sort)
sha256sum --strict -c SHA256SUMS

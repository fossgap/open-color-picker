#!/usr/bin/env bash
# Run locally or in Actions. Containers are disposable; this script never publishes.
set -euo pipefail
cd "$(dirname "$0")/.."
mode=${1:-check}
case "$mode" in check|release) ;; *) echo 'Usage: scripts/ci.sh check|release' >&2; exit 2 ;; esac
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)
[[ -n "$version" ]]
output=${OCP_CI_OUTPUT:-dist/release}
if [[ "$mode" == release ]]; then
  if [[ ${GITHUB_REF_TYPE:-} == tag && ${GITHUB_REF_NAME:-} != "v$version" ]]; then
    echo 'Release tag must match Cargo.toml version.' >&2; exit 1
  fi
  [[ ! -e "$output" ]] || { echo "Choose a new output directory: $output" >&2; exit 1; }
  [[ -z $(git status --porcelain --untracked-files=normal) ]] || {
    echo 'Commit source changes before building a release, so its source archive matches the packages.' >&2; exit 1;
  }
fi
container=
manifest=
cleanup() {
  if [[ -n "$container" ]]; then docker rm -f "$container" >/dev/null 2>&1 || true; fi
  if [[ -n "$manifest" ]]; then rm -f -- "$manifest"; fi
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
container=$(docker create --tmpfs /tmp:rw,exec --shm-size=1g \
  "${OCP_CI_IMAGE:-open-color-picker-ci:latest}" bash scripts/ci-inside.sh "$mode")
manifest=$(mktemp)
docker cp "$container:/source-checksums" "$manifest"
cmp -s "$manifest" <(./scripts/source-manifest.sh) || {
  echo 'CI image does not match this checkout. Rebuild it with docker build -t open-color-picker-ci .' >&2; exit 1;
}
docker start -a "$container"
# docker start -a can return success after a signalled container; check its status.
[[ $(docker inspect --format '{{.State.ExitCode}}' "$container") == 0 ]]
if [[ "$mode" == release ]]; then
  mkdir -p "$output"
  docker cp "$container:/workspace/dist/release/." "$output/"
  git archive --format=tar.gz --prefix="open-color-picker-$version/" HEAD > "$output/open-color-picker-$version-source.tar.gz"
  (cd "$output" && sha256sum ./*.deb ./*.rpm ./*.pkg.tar.zst ./*.AppImage ./*-source.tar.gz > SHA256SUMS)
  ./scripts/verify-release.sh "$output"
fi

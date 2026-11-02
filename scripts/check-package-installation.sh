#!/usr/bin/env bash
# Install only in disposable containers, never on the host.
set -euo pipefail
cd "$(dirname "$0")/.."
[[ $# -ge 1 && $# -le 2 ]] || { echo 'Usage: scripts/check-package-installation.sh debian|kali|fedora|arch|appimage [ARTIFACT_DIRECTORY]' >&2; exit 2; }
case "$1" in
  debian) image=debian:bookworm-slim; format=deb ;;
  kali) image=kalilinux/kali-rolling:latest; format=deb ;;
  fedora) image=registry.fedoraproject.org/fedora:44; format=rpm ;;
  arch) image=archlinux:base; format=arch ;;
  appimage) image=ubuntu:24.04; format=appimage ;;
  *) echo "Unknown installation target: $1" >&2; exit 2 ;;
esac
[[ $(uname -m) == x86_64 ]] || { echo 'Release installation checks require an x86_64 host.' >&2; exit 1; }
artifacts=$(realpath "${2:-dist/release}")
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)
./scripts/verify-release.sh "$artifacts"
docker pull "$image"
docker image inspect "$image" --format '{{json .RepoTags}} {{json .RepoDigests}} {{.Id}}'
container=
cleanup() { if [[ -n "$container" ]]; then docker rm -f "$container" >/dev/null 2>&1 || true; fi; }
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
# Keep disposable repository indexes and download caches off the Docker disk.
container=$(docker create --tmpfs /tmp:rw,exec --tmpfs /var/cache:rw,exec \
  --tmpfs /var/lib/apt/lists:rw --shm-size=1g \
  --env OCP_INSTALL_TEST_CONTAINER=1 --env DEBIAN_FRONTEND=noninteractive \
  --volume "$artifacts:/artifacts:ro" \
  --volume "$PWD/scripts:/workspace/scripts:ro" \
  --volume "$PWD/tests/package-install:/ocp-test:ro" \
  "$image" bash /ocp-test/install.sh "$format" "$version")
docker start -a "$container"
[[ $(docker inspect --format '{{.State.ExitCode}}' "$container") == 0 ]]

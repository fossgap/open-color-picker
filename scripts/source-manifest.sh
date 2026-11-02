#!/usr/bin/env bash
# Identify every build, packaging and verification input in a local CI image.
set -euo pipefail
cd "$(dirname "$0")/.."
find Cargo.toml Cargo.lock src Makefile README.md data docs scripts tests packaging \
  Dockerfile .dockerignore .github -type f -print0 \
  | LC_ALL=C sort -z | xargs -0 sha256sum

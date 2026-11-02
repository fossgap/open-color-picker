# syntax=docker/dockerfile:1
# Bookworm supplies the glibc 2.36 baseline for all release formats.
FROM rust:1.90-bookworm

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
       build-essential pkg-config libgtk-3-dev libgtk-3-bin \
       libayatana-appindicator3-1 librsvg2-common \
       xvfb xauth dbus-x11 xdotool x11-utils x11-xserver-utils xclip \
       fonts-dejavu-core fonts-noto-cjk fontconfig-config shared-mime-info locales \
       curl ca-certificates file rpm zstd desktop-file-utils shellcheck \
    && rm -rf /var/lib/apt/lists/*
RUN for language in fr_FR es_ES de_DE pt_BR zh_CN ja_JP; do \
      localedef -i "$language" -f UTF-8 "$language.UTF-8"; \
    done
RUN rustup component add rustfmt clippy

WORKDIR /workspace
ENV CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY data ./data
RUN cargo build --locked && cargo test --locked
COPY Makefile README.md ./
COPY docs ./docs
COPY scripts ./scripts
COPY tests ./tests
COPY packaging ./packaging
COPY Dockerfile .dockerignore ./
COPY .github ./.github
RUN ./scripts/source-manifest.sh > /source-checksums
CMD ["bash", "scripts/ci-inside.sh", "check"]

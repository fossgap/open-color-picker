PREFIX ?= /usr/local
DESTDIR ?=
CARGO ?= cargo
CARGO_TARGET_DIR ?= target
BINARY ?= $(CARGO_TARGET_DIR)/release/open-color-picker
APP_ID = io.github.opencolorpicker.OpenColorPicker

.PHONY: all build test check run install deb rpm appdir appimage arch ci verify-packages
all: build

build:
	$(CARGO) build --release --locked

run:
	$(CARGO) run --locked

test:
	$(CARGO) test --locked

check:
	$(CARGO) fmt --all -- --check
	$(CARGO) clippy --all-targets --locked -- -D warnings

install:
	install -Dm755 "$(BINARY)" "$(DESTDIR)$(PREFIX)/bin/open-color-picker"
	install -Dm644 data/$(APP_ID).desktop "$(DESTDIR)$(PREFIX)/share/applications/$(APP_ID).desktop"
	install -Dm644 data/$(APP_ID).svg "$(DESTDIR)$(PREFIX)/share/icons/hicolor/scalable/apps/$(APP_ID).svg"
	install -Dm644 README.md "$(DESTDIR)$(PREFIX)/share/doc/open-color-picker/README.md"
	install -Dm644 docs/screenshot.png "$(DESTDIR)$(PREFIX)/share/doc/open-color-picker/docs/screenshot.png"
	install -Dm644 docs/packaging.md "$(DESTDIR)$(PREFIX)/share/doc/open-color-picker/docs/packaging.md"
	install -Dm644 docs/ci-release.md "$(DESTDIR)$(PREFIX)/share/doc/open-color-picker/docs/ci-release.md"
	install -Dm644 docs/development.md "$(DESTDIR)$(PREFIX)/share/doc/open-color-picker/docs/development.md"
	install -Dm644 docs/translations.md "$(DESTDIR)$(PREFIX)/share/doc/open-color-picker/docs/translations.md"

deb rpm appdir appimage arch:
	./scripts/package.sh $@

ci:
	./scripts/ci.sh check

verify-packages:
	@for distro in debian kali fedora arch appimage; do \
		./scripts/check-package-installation.sh $$distro || exit $$?; \
	done


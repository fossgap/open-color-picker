#!/usr/bin/env bash
# Package one release binary; native package managers resolve its runtime deps.
set -euo pipefail
cd "$(dirname "$0")/.."
kind=${1:-}
case "$kind" in deb|rpm|arch|appdir|appimage) ;; *) echo 'Usage: scripts/package.sh deb|rpm|arch|appdir|appimage' >&2; exit 2 ;; esac
[[ $(uname -m) == x86_64 ]] || { echo 'The release packages currently target Linux x86_64.' >&2; exit 1; }
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo 'Packages require an x.y.z version.' >&2; exit 1; }
if [[ -z ${OCP_BINARY:-} ]]; then
  cargo build --release --locked
  binary="${CARGO_TARGET_DIR:-target}/release/open-color-picker"
else
  binary=$OCP_BINARY
fi
binary=$(realpath "$binary")
[[ $("$binary" --version) == "Open Color Picker $version" ]] || { echo 'Binary version does not match Cargo.toml.' >&2; exit 1; }
mkdir -p dist target/packages
stage=$(mktemp -d "$PWD/target/packages/stage.XXXXXX")
trap 'rm -rf -- "$stage"' EXIT
make install PREFIX=/usr DESTDIR="$stage/root" BINARY="$binary"

case "$kind" in
  deb)
    mkdir -p "$stage/root/DEBIAN" "$stage/debian"
    cat > "$stage/debian/control" <<'EOF'
Source: open-color-picker
Section: graphics
Priority: optional
Maintainer: Fossgap contributors

Package: open-color-picker
Architecture: any
Description: Native GTK3 color picker
EOF
    depends=$(cd "$stage" && dpkg-shlibdeps -O -e"$stage/root/usr/bin/open-color-picker" | sed -n 's/^shlibs:Depends=//p')
    [[ -n "$depends" ]] || { echo 'Could not determine shared-library dependencies.' >&2; exit 1; }
    cat > "$stage/root/DEBIAN/control" <<EOF
Package: open-color-picker
Version: $version
Section: graphics
Priority: optional
Architecture: amd64
Maintainer: Fossgap contributors
Homepage: https://github.com/fossgap/open-color-picker
Depends: $depends, librsvg2-common
Recommends: xdg-desktop-portal, libayatana-appindicator3-1 | libappindicator3-1
Installed-Size: $(du -sk "$stage/root/usr" | cut -f1)
Description: Native GTK3 color picker for X11 and Wayland
 Pick screen pixels with a magnifier, copy HEX/RGB/HSL values,
 and keep a local palette of favorite colors.
EOF
    dpkg-deb --root-owner-group --build "$stage/root" "dist/open-color-picker_${version}_amd64.deb"
    ;;
  rpm)
    top="$stage/rpmbuild"
    mkdir -p "$top"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS,tmp,rpmdb}
    tar -C "$stage/root" -czf "$top/SOURCES/payload.tar.gz" usr
    sed "s/@VERSION@/$version/g" packaging/rpm/open-color-picker.spec > "$top/SPECS/open-color-picker.spec"
    rpmbuild -bb --define "_topdir $top" --define "_tmppath $top/tmp" --define "_dbpath $top/rpmdb" \
      --define '_build_id_links none' --define '__os_install_post %{nil}' "$top/SPECS/open-color-picker.spec"
    cp "$top/RPMS/x86_64/"*.rpm dist/
    ;;
  arch)
    sed -e "s/@VERSION@/$version/g" \
      -e "s/@BUILDDATE@/${SOURCE_DATE_EPOCH:-$(date +%s)}/g" \
      -e "s/@SIZE@/$(du -sb "$stage/root/usr" | cut -f1)/g" \
      packaging/arch/PKGINFO.in > "$stage/root/.PKGINFO"
    tar --zstd --owner=0 --group=0 -cf "dist/open-color-picker-${version}-1-x86_64.pkg.tar.zst" \
      -C "$stage/root" .PKGINFO usr
    ;;
  appdir|appimage)
    destination="$PWD/dist/OpenColorPicker.AppDir"
    [[ ! -e "$destination" ]] || { echo "Remove the previous AppDir before rebuilding: $destination" >&2; exit 1; }
    ./scripts/bundle-runtime.sh "$stage/root"
    install -m755 packaging/AppRun "$stage/root/AppRun"
    cp data/io.github.opencolorpicker.OpenColorPicker.{desktop,svg} "$stage/root/"
    mv "$stage/root" "$destination"
    echo "AppDir created: $destination"
    if [[ "$kind" == appimage ]]; then ./scripts/appimage.sh; fi
    ;;
esac

#!/usr/bin/env bash
# Invoked by check-package-installation.sh inside its disposable container only.
set -Eeuo pipefail
trap 'printf "Package verification failed at line %s: %s\n" "$LINENO" "$BASH_COMMAND" >&2' ERR
if [[ ${OCP_INSTALL_TEST_CONTAINER:-} != 1 || ! -f /.dockerenv || ! -d /artifacts ]]; then
  echo 'Use scripts/check-package-installation.sh; this script installs container packages.' >&2
  exit 2
fi
format=${1:?Choose deb, rpm, arch or appimage}
version=${2:?Provide the expected application version}
one_file() {
  [[ $# == 1 && -f "$1" ]] || { echo 'Expected exactly one release artifact for this package.' >&2; exit 1; }
  printf '%s\n' "$1"
}
cat /etc/os-release
binary=/usr/bin/open-color-picker
case "$format" in
  deb)
    app=$(one_file /artifacts/open-color-picker_*_amd64.deb)
    [[ $(dpkg-deb -f "$app" Version) == "$version" ]]
    [[ $(dpkg-deb -f "$app" Architecture) == amd64 ]]
    dpkg-deb --fsys-tarfile "$app" | tar -tf - \
      | grep -Fx './usr/share/doc/open-color-picker/README.md' >/dev/null
    apt-get update
    apt-get -y --no-remove --no-install-recommends install "$app"
    [[ $(dpkg-query -W -f='${Version}' open-color-picker) == "$version" ]]
    # The About logo uses GdkPixbuf's dynamically loaded SVG decoder, which
    # dpkg-shlibdeps cannot discover from the executable's ELF dependencies.
    [[ $(dpkg-query -W -f='${db:Status-Status}' librsvg2-common) == installed ]]
    apt-get check
    ;;
  rpm)
    app=$(one_file /artifacts/open-color-picker-[0-9]*.x86_64.rpm)
    [[ $(rpm -qp --qf '%{VERSION}-%{RELEASE}.%{ARCH}' "$app") == "$version-1.x86_64" ]]
    rpm -qlp "$app" | grep -Fx '/usr/share/doc/open-color-picker/README.md' >/dev/null
    dnf -y --setopt=install_weak_deps=False install "$app"
    [[ $(rpm -q --qf '%{VERSION}-%{RELEASE}.%{ARCH}' open-color-picker) == "$version-1.x86_64" ]]
    dnf check
    ;;
  arch)
    app=$(one_file /artifacts/open-color-picker-[0-9]*-x86_64.pkg.tar.zst)
    pacman --noconfirm -Syu
    pacman --noconfirm -U "$app"
    pacman -Qlp "$app" | grep -Fx 'open-color-picker /usr/share/doc/open-color-picker/README.md' >/dev/null
    [[ $(pacman -Q open-color-picker) == "open-color-picker $version-1" ]]
    [[ $(pacman -Q open-color-picker) == "$(pacman -Qp "$app")" ]]
    pacman -Qk open-color-picker
    ;;
  appimage)
    app=$(one_file /artifacts/open-color-picker_*_amd64.deb)
    portable=$(one_file /artifacts/OpenColorPicker-*-x86_64.AppImage)
    install -m755 "$portable" /opt/OpenColorPicker.AppImage
    binary=/opt/OpenColorPicker.AppImage
    export APPIMAGE_EXTRACT_AND_RUN=1
    apt-get update
    ;;
  *) echo "Unknown package format: $format" >&2; exit 2 ;;
esac
# Check library resolution before any test-tool dependencies can mask it.
[[ $("$binary" --version) == "Open Color Picker $version" ]]
# Fedora's minimal image omits runuser. It is test infrastructure, not an
# application dependency; install it only after the application's CLI works.
if [[ "$format" == rpm ]]; then dnf -y --setopt=install_weak_deps=False install util-linux; fi
useradd -m ocptest
[[ $(runuser -u ocptest -- "$binary" --version) == "Open Color Picker $version" ]]
runuser -u ocptest -- "$binary" --help >/tmp/ocp-installed-help.txt
echo 'PACKAGE_INSTALLED_AND_CLI_VERIFIED'
case "$format" in
  deb|appimage)
    apt-get -y --no-install-recommends install xvfb xauth dbus-x11 xdotool x11-utils x11-xserver-utils xclip fonts-dejavu-core desktop-file-utils
    ;;
  rpm)
    dnf -y --setopt=install_weak_deps=False install xorg-x11-server-Xvfb xorg-x11-xauth dbus-daemon xdotool xsetroot xclip dejavu-sans-fonts desktop-file-utils
    ;;
  arch)
    pacman --noconfirm -S --needed xorg-server-xvfb xorg-xauth dbus xdotool xorg-xsetroot xclip ttf-dejavu desktop-file-utils
    ;;
esac
if [[ "$format" == appimage ]]; then
  [[ $(dpkg-query -W -f='${db:Status-Status}' libgtk-3-0t64 2>/dev/null || true) != installed ]]
  [[ $(dpkg-query -W -f='${db:Status-Status}' libgtk-3-0 2>/dev/null || true) != installed ]]
  echo 'APPIMAGE_HOST_HAS_NO_GTK'
  inspection=$(runuser -u ocptest -- mktemp -d /tmp/ocp-appimage-inspect.XXXXXX)
  dpkg-deb -x "$app" "$inspection/deb"
  # Positional arguments are expanded by the child shell, not this one.
  # shellcheck disable=SC2016
  runuser -u ocptest -- sh -c 'cd "$1"; "$2" --appimage-extract >/dev/null' sh "$inspection" "$binary"
  cmp "$inspection/deb/usr/bin/open-color-picker" "$inspection/squashfs-root/usr/bin/open-color-picker"
  prefix="$inspection/squashfs-root/usr"
  test -s "$prefix/share/doc/open-color-picker/README.md"
else
  prefix=/usr
fi
desktop="$prefix/share/applications/io.github.opencolorpicker.OpenColorPicker.desktop"
desktop-file-validate "$desktop"
test -s "$prefix/share/icons/hicolor/scalable/apps/io.github.opencolorpicker.OpenColorPicker.svg"
# Minimal distro images may deliberately omit installed documentation. Its
# presence is checked in each package archive before relying on host policy.
[[ $(stat -c '%u:%g' "$prefix/bin/open-color-picker") == 0:0 || "$format" == appimage ]]
runuser -u ocptest -- env OCP_BINARY="$binary" /workspace/scripts/smoke-test.sh
echo "PACKAGE_INSTALLATION_SUITE_PASSED: $format"

%global debug_package %{nil}
Name: open-color-picker
Version: @VERSION@
Release: 1
Summary: Native GTK3 color picker for X11 and Wayland
URL: https://github.com/fossgap/open-color-picker
License: GPL-3.0-only
Source0: payload.tar.gz
BuildArch: x86_64
Requires: gtk3 >= 3.24
Requires: gstreamer1-plugins-base, pipewire-gstreamer
Recommends: xdg-desktop-portal

%description
Pick screen pixels with a magnifier, copy HEX/RGB/HSL values,
and keep a local palette of favorite colors.

%install
mkdir -p %{buildroot}
tar -xzf %{SOURCE0} -C %{buildroot}

%files
/usr/bin/open-color-picker
/usr/share/applications/io.github.opencolorpicker.OpenColorPicker.desktop
/usr/share/icons/hicolor/scalable/apps/io.github.opencolorpicker.OpenColorPicker.svg
%license /usr/share/doc/open-color-picker/LICENSE
%doc /usr/share/doc/open-color-picker/README.md
%doc /usr/share/doc/open-color-picker/docs

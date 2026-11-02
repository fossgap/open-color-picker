#!/usr/bin/env bash
# Bundle GTK3 from the Debian build image, retaining the host glibc and loader.
set -euo pipefail
destination=${1:?Provide a staged AppDir}
destination=$(realpath "$destination")
[[ -x "$destination/usr/bin/open-color-picker" ]]
runtime_docs="$destination/usr/share/doc/open-color-picker/runtime"
mkdir -p "$destination/usr/lib/gio/modules" "$destination/usr/lib/gtk-3.0" "$destination/usr/etc" "$runtime_docs"
shopt -s nullglob

copyright_for() {
  local library=$1 owner package
  owner=$(dpkg-query -S "$library" 2>/dev/null || dpkg-query -S "*/$(basename "$library")" 2>/dev/null || true)
  while IFS= read -r package; do
    package=${package%%: /*}
    package=${package%%:*}
    if [[ -f "/usr/share/doc/$package/copyright" ]]; then
      cp "/usr/share/doc/$package/copyright" "$runtime_docs/$package.copyright"
    fi
  done <<< "$owner"
}
copy_libraries() {
  local input=$1 library name
  # ldd resolves the complete transitive closure, including a module's deps.
  while IFS= read -r library; do
    name=$(basename "$library")
    case "$name" in libc.so.*|libm.so.*|libdl.so.*|libpthread.so.*|librt.so.*|ld-linux*) continue ;; esac
    if [[ ! -f "$destination/usr/lib/$name" ]]; then
      cp -L "$library" "$destination/usr/lib/$name"
      copyright_for "$library"
    fi
  done < <(ldd "$input" | awk '/=> \// {print $3}')
}
copy_libraries "$destination/usr/bin/open-color-picker"
for modules in /usr/lib/*/gio/modules /usr/lib/*/gtk-3.0; do
  [[ -d "$modules" ]] || continue
  if [[ "$modules" == */gio/modules ]]; then
    cp -a "$modules/." "$destination/usr/lib/gio/modules/"
  else
    cp -a "$modules/." "$destination/usr/lib/gtk-3.0/"
  fi
done
for loaders in /usr/lib/*/gdk-pixbuf-2.0/*/loaders; do
  cp -L "$loaders/"*.so "$destination/usr/lib/"
  # A basename is resolved through AppRun's LD_LIBRARY_PATH after relocation.
  sed -E 's@"[^"]*/(libpixbufloader-[^"]+)"@"\1"@' "$(dirname "$loaders")/loaders.cache" > "$destination/usr/lib/loaders.cache"
  for loader in "$loaders/"*.so; do copyright_for "$loader"; done
done
for cache in /usr/lib/*/gtk-3.0/*/immodules.cache; do
  sed -E 's@"[^"]*/(im-[^"]+)"@"\1"@' "$cache" > "$destination/usr/lib/gtk.immodules"
  for module in "$(dirname "$cache")/immodules/"*.so; do cp -L "$module" "$destination/usr/lib/"; done
done
while IFS= read -r module; do copy_libraries "$module"; done < <(find "$destination/usr/lib" -name '*.so' -type f)
[[ -s "$destination/usr/lib/loaders.cache" && -s "$destination/usr/lib/gtk.immodules" ]]
mkdir -p "$destination/usr/share/glib-2.0" "$destination/usr/share/icons"
cp -a /usr/share/glib-2.0/schemas "$destination/usr/share/glib-2.0/"
cp -a /usr/share/mime "$destination/usr/share/"
# Native GTK dialogs and accelerator labels use gettext catalogs in addition
# to the application's embedded Rust catalogs.
for language in fr es de pt pt_BR zh_CN ja; do
  locale_dir="$destination/usr/share/locale/$language/LC_MESSAGES"
  mkdir -p "$locale_dir"
  for domain in gtk30 gtk30-properties glib20 atk10; do
    source_catalog="/usr/share/locale/$language/LC_MESSAGES/$domain.mo"
    if [[ -f "$source_catalog" ]]; then cp "$source_catalog" "$locale_dir/"; fi
  done
done
cp -aL /etc/fonts "$destination/usr/etc/"
for item in /usr/share/icons/Adwaita /usr/share/icons/hicolor; do
  if [[ -d "$item" ]]; then cp -a "$item" "$destination/usr/share/icons/"; fi
done
if [[ -d /usr/share/themes ]]; then cp -a /usr/share/themes "$destination/usr/share/"; fi
for package in shared-mime-info fontconfig-config adwaita-icon-theme hicolor-icon-theme gsettings-desktop-schemas libgtk-3-common libglib2.0-data libatk1.0-data; do
  if [[ -f /usr/share/doc/$package/copyright ]]; then cp "/usr/share/doc/$package/copyright" "$runtime_docs/$package.copyright"; fi
done

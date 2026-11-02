#!/usr/bin/env bash
# Native Wayland checks in a private compositor, bus and XDG directories.
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ ${1:-} != --inside ]]; then
  exec xvfb-run -a -s '-screen 0 1280x900x24' \
    dbus-run-session --config-file=scripts/test-bus.conf -- bash "$0" --inside
fi
test_root=$(mktemp -d)
compositor_pid=
cleanup() {
  local status=$?
  if [[ -n "$compositor_pid" ]]; then
    kill "$compositor_pid" 2>/dev/null || true
    for ((attempt=0; attempt<40; attempt++)); do
      if ! kill -0 "$compositor_pid" 2>/dev/null; then break; fi
      sleep 0.05
    done
    kill -KILL "$compositor_pid" 2>/dev/null || true
    wait "$compositor_pid" 2>/dev/null || true
  fi
  if [[ "$status" != 0 && -f "$test_root/weston.log" ]]; then cat "$test_root/weston.log" >&2; fi
  rm -rf -- "$test_root"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
unset WAYLAND_DISPLAY
export XDG_RUNTIME_DIR="$test_root/runtime" XDG_CONFIG_HOME="$test_root/config"
export XDG_DATA_HOME="$test_root/data" XDG_CACHE_HOME="$test_root/cache" XDG_STATE_HOME="$test_root/state"
mkdir -m700 "$XDG_RUNTIME_DIR"
export LC_ALL=C.UTF-8 LANGUAGE=C GSETTINGS_BACKEND=memory
export GDK_BACKEND=wayland XDG_SESSION_TYPE=wayland WAYLAND_DISPLAY=ocp-test
export NO_AT_BRIDGE=1 GTK_USE_PORTAL=0 GST_XINITTHREADS=1
# Weston supplies native Wayland protocol behavior. Its private X11 backend
# provides a test keyboard/pointer and lets us inspect the composited result.
weston --backend=x11-backend.so --use-pixman --width=1280 --height=900 \
  --socket="$WAYLAND_DISPLAY" --idle-time=0 --no-config --log="$test_root/weston.log" &
compositor_pid=$!
for ((attempt=0; attempt<100; attempt++)); do
  if [[ -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ]]; then break; fi
  kill -0 "$compositor_pid"
  sleep 0.05
done
[[ -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ]]
for theme in light dark; do
  gtk_theme=Adwaita
  if [[ "$theme" == dark ]]; then gtk_theme=Adwaita:dark; fi
  GTK_THEME="$gtk_theme" OCP_UI_THEME_LABEL="$theme" \
    cargo test --locked native_wayland_frozen_overlay -- --ignored
done

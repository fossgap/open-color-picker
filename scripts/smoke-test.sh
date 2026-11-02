#!/usr/bin/env bash
set -Eeuo pipefail
trap 'printf "Smoke test failed at line %s: %s\n" "$LINENO" "$BASH_COMMAND" >&2' ERR
cd "$(dirname "$0")/.."
export LC_ALL=C.UTF-8 LANGUAGE=C
if [[ ${1:-} != --inside ]]; then
  for tool in xvfb-run dbus-run-session xdotool xsetroot xclip; do
    command -v "$tool" >/dev/null || { printf 'Missing smoke-test tool: %s\n' "$tool" >&2; exit 1; }
  done
  exec xvfb-run -a -s '-screen 0 1280x900x24' dbus-run-session --config-file=scripts/test-bus.conf -- "$0" --inside
fi
export GDK_BACKEND=x11 XDG_SESSION_TYPE=x11 NO_AT_BRIDGE=1 GTK_USE_PORTAL=0 GIO_USE_VFS=local
unset WAYLAND_DISPLAY
export XDG_DATA_HOME
XDG_DATA_HOME=$(mktemp -d)
export XDG_CONFIG_HOME="$XDG_DATA_HOME/config"
export XDG_RUNTIME_DIR="$XDG_DATA_HOME/runtime"
mkdir -m700 "$XDG_RUNTIME_DIR"
binary=${OCP_BINARY:-./target/debug/open-color-picker}
app_pid=
cleanup() {
  local status=$? log
  if [[ -n "$app_pid" ]]; then kill "$app_pid" 2>/dev/null || true; fi
  if [[ "$status" != 0 ]]; then
    for log in "$XDG_DATA_HOME"/*.log; do
      if [[ -f "$log" ]]; then printf '\n%s\n' "$(basename "$log")" >&2; cat "$log" >&2; fi
    done
  fi
  rm -rf -- "$XDG_DATA_HOME"
}
trap cleanup EXIT
wait_window() {
  local pattern=$1 window=
  for ((attempt=0; attempt<60; attempt++)); do
    window=$(xdotool search --onlyvisible --name "$pattern" 2>/dev/null | head -1) || true
    if [[ -n "$window" ]]; then printf '%s\n' "$window"; return; fi
    sleep 0.1
  done
  cat "$XDG_DATA_HOME/app.log" >&2
  printf 'Window did not appear: %s\n' "$pattern" >&2
  return 1
}
assert_clipboard() {
  local expected=$1 actual
  for ((attempt=0; attempt<20; attempt++)); do
    actual=$(xclip -o -selection clipboard 2>/dev/null) || true
    [[ "$actual" == "$expected" ]] && return
    sleep 0.1
  done
  printf 'Clipboard: expected %s, got %s\n' "$expected" "$actual" >&2
  return 1
}
# The private bus has no tray host: --tray must fall back to a visible window.
"$binary" --tray > "$XDG_DATA_HOME/app.log" 2>&1 & app_pid=$!
main=$(wait_window '^Open Color Picker$')
xdotool windowfocus --sync "$main"
xdotool key ctrl+shift+c
assert_clipboard '#3584E4'
xdotool key ctrl+s
sleep 0.2
test -s "$XDG_DATA_HOME/open-color-picker/palette.json"

# Cancellation keeps the prior selection; the CLI activates the same process.
"$binary" --pick
picker=$(wait_window '^Pick a pixel')
xdotool windowfocus --sync "$picker"
xdotool key Escape
main=$(wait_window '^Open Color Picker$')
xdotool windowfocus --sync "$main"
xdotool key ctrl+shift+c
assert_clipboard '#3584E4'

# A controlled root color verifies capture, magnification, confirmation and copy.
xsetroot -solid '#1a2b3c'
"$binary" --pick
picker=$(wait_window '^Pick a pixel')
xdotool windowfocus --sync "$picker"
xdotool key Right Down Return
main=$(wait_window '^Open Color Picker$')
assert_clipboard '#1A2B3C'
"$binary" --quit
wait "$app_pid"
app_pid=
"$binary" > "$XDG_DATA_HOME/restart.log" 2>&1 & app_pid=$!
main=$(wait_window '^Open Color Picker$')
xdotool windowfocus --sync "$main"
xdotool key ctrl+shift+c
assert_clipboard '#1A2B3C'
"$binary" --quit
wait "$app_pid"
app_pid=
if grep -Ei 'panicked|CRITICAL|assertion.*failed' "$XDG_DATA_HOME"/*.log; then exit 1; fi
printf 'GTK smoke test passed: capture, cancel, clipboard, favorites, persistence, single instance, tray fallback.\n'


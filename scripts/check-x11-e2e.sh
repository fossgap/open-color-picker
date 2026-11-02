#!/usr/bin/env bash
# Drive the unmodified application through real Mutter, portal and PipeWire services.
set -Eeuo pipefail
cd "$(dirname "$0")/.."
if [[ ${1:-} != --inside ]]; then
  cargo build --locked --example e2e-desktop
  exec xvfb-run -a -s '-screen 0 1280x900x24 +extension GLX' \
    dbus-run-session --config-file=scripts/test-bus.conf -- bash "$0" --inside
fi
test_root=$(mktemp -d)
pids=()
app_pid=
cleanup() {
  local status=$? pid
  for pid in "${pids[@]}"; do kill "$pid" 2>/dev/null || true; done
  for ((attempt=0; attempt<40; attempt++)); do
    local running=false
    for pid in "${pids[@]}"; do
      if kill -0 "$pid" 2>/dev/null; then running=true; fi
    done
    if [[ "$running" == false ]]; then break; fi
    sleep 0.05
  done
  for pid in "${pids[@]}"; do kill -KILL "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true; done
  if [[ -n ${OCP_E2E_OUTPUT:-} ]]; then mkdir -p "$OCP_E2E_OUTPUT"; cp "$test_root"/*.log "$OCP_E2E_OUTPUT/"; fi
  if [[ "$status" != 0 ]]; then tail -n 50 "$test_root"/*.log >&2; fi
  rm -rf -- "$test_root"
}
trap cleanup EXIT
trap 'printf "X11 E2E failed at line %s: %s\n" "$LINENO" "$BASH_COMMAND" >&2' ERR
trap 'exit 130' INT
trap 'exit 143' TERM
unset WAYLAND_DISPLAY
export XDG_RUNTIME_DIR="$test_root/runtime" XDG_CONFIG_HOME="$test_root/config"
export XDG_DATA_HOME="$test_root/data" XDG_CACHE_HOME="$test_root/cache" XDG_STATE_HOME="$test_root/state"
mkdir -m700 "$XDG_RUNTIME_DIR"
export LC_ALL=C.UTF-8 LANGUAGE=C GSETTINGS_BACKEND=memory
export GDK_BACKEND=x11 XDG_SESSION_TYPE=x11 XDG_CURRENT_DESKTOP=GNOME
export GTK_USE_PORTAL=0 NO_AT_BRIDGE=1 GTK_A11Y=none LIBGL_ALWAYS_SOFTWARE=1
# The private desktop has no Bluetooth devices or logind seat. Leave the real
# PipeWire access/linking policy enabled, but skip Bluetooth seat arbitration.
mkdir -p "$XDG_CONFIG_HOME/wireplumber/bluetooth.lua.d"
printf '%s\n' 'bluez_monitor.properties["with-logind"] = false' \
  > "$XDG_CONFIG_HOME/wireplumber/bluetooth.lua.d/51-e2e.lua"
binary=${OCP_BINARY:-./target/debug/open-color-picker}
probe=./target/debug/examples/e2e-desktop
start_service() {
  local name=$1; shift
  "$@" > "$test_root/$name.log" 2>&1 &
  pids+=("$!")
}
wait_window() {
  local name=$1 window=
  for ((attempt=0; attempt<100; attempt++)); do
    window=$(xdotool search --onlyvisible --name "$name" 2>/dev/null | head -1) || true
    if [[ -n "$window" ]]; then printf '%s\n' "$window"; return; fi
    sleep 0.1
  done
  printf 'Window did not appear: %s\n' "$name" >&2; return 1
}
wait_pixel() {
  local x=$1 y=$2 expected=$3 comparison=${4:-equal} actual=
  for ((attempt=0; attempt<60; attempt++)); do
    actual=$("$probe" pixel "$x" "$y")
    if [[ "$actual" == "$expected" && "$comparison" == equal || "$actual" != "$expected" && "$comparison" == different ]]; then return; fi
    sleep 0.1
  done
  printf 'Pixel %s,%s: expected %s to %s, got %s\n' "$x" "$y" "$comparison" "$expected" "$actual" >&2; return 1
}
assert_clipboard() {
  local expected=$1 actual=
  for ((attempt=0; attempt<30; attempt++)); do
    actual=$(xclip -o -selection clipboard 2>/dev/null) || true
    if [[ "$actual" == "$expected" ]]; then return; fi
    sleep 0.1
  done
  printf 'Clipboard: expected %s, got %s\n' "$expected" "$actual" >&2; return 1
}
snapshot() {
  if [[ -n ${OCP_E2E_OUTPUT:-} ]]; then
    mkdir -p "$OCP_E2E_OUTPUT"
    "$probe" snapshot "$OCP_E2E_OUTPUT/$1.png"
  fi
}
share() {
  local dialog
  dialog=$(wait_window '^Screen Share$')
  xdotool windowactivate --sync "$dialog"
  # The real GNOME portal has one monitor preselected and a _Share mnemonic.
  sleep 0.2
  snapshot consent
  xdotool key alt+s
  picker=$(wait_window '^Pick a pixel')
  xdotool windowactivate --sync "$picker" mousemove 640 400
}

start_service pipewire pipewire
pipewire_pid=${pids[-1]}
for ((attempt=0; attempt<100; attempt++)); do
  if [[ -S "$XDG_RUNTIME_DIR/pipewire-0" ]]; then break; fi
  sleep 0.05
done
[[ -S "$XDG_RUNTIME_DIR/pipewire-0" ]]
start_service wireplumber wireplumber
start_service mutter mutter --x11
gdbus wait --session --timeout 10 org.gnome.Mutter.ScreenCast
start_service permission /usr/libexec/xdg-permission-store
start_service gnome-portal /usr/libexec/xdg-desktop-portal-gnome
gdbus wait --session --timeout 10 org.freedesktop.impl.portal.desktop.gnome
start_service portal /usr/libexec/xdg-desktop-portal
gdbus wait --session --timeout 10 org.freedesktop.portal.Desktop
start_service requests dbus-monitor --session \
  "interface='org.freedesktop.portal.ScreenCast'" \
  "interface='org.freedesktop.portal.Session'"
printf '#1A2B3C\n' > "$test_root/color"
start_service backdrop "$probe" background "$test_root/color"
wait_pixel 640 400 '#1A2B3C'
start_service app "$binary"
app_pid=${pids[-1]}
main=$(wait_window '^Open Color Picker$')
xdotool windowactivate --sync "$main" key ctrl+shift+c
assert_clipboard '#3584E4'

# Cancellation in the real permission dialog preserves the prior selection.
"$binary" --pick
dialog=$(wait_window '^Screen Share$')
xdotool windowactivate --sync "$dialog" key Escape
main=$(wait_window '^Open Color Picker$')
xdotool windowactivate --sync "$main" key ctrl+shift+c
assert_clipboard '#3584E4'

# Real CreateSession -> SelectSources -> Start -> OpenPipeWireRemote -> frames.
"$binary" --pick
share
wait_pixel 700 450 '#1A2B3C' # Inside the magnifier grid, away from its cursor.
wait_pixel 667 427 '#1A2B3C' different # Wait for the magnifier border to render.
printf '#4488CC\n' > "$test_root/color"
wait_pixel 300 300 '#4488CC' # Actual desktop is visible through the overlay.
wait_pixel 700 450 '#4488CC' # Magnified pixels update from the real stream.
snapshot live
# Move into the previous magnifier and verify there is no capture feedback.
xdotool mousemove 700 450
printf '#CC8844\n' > "$test_root/color"
wait_pixel 760 500 '#CC8844'
xdotool key Right Down Return
main=$(wait_window '^Open Color Picker$')
assert_clipboard '#CC8844'
xdotool windowactivate --sync "$main" key ctrl+s
test -s "$XDG_DATA_HOME/open-color-picker/palette.json"
grep -q 'member=OpenPipeWireRemote' "$test_root/requests.log"
grep -q 'interface=org.freedesktop.portal.Session; member=Close' "$test_root/requests.log"
if grep -q 'Live desktop capture unavailable' "$test_root/app.log"; then exit 1; fi

# Cancelling an active stream closes it and preserves the picked color.
"$binary" --pick
share
xdotool key Escape
main=$(wait_window '^Open Color Picker$')
xdotool windowactivate --sync "$main" key ctrl+shift+c
assert_clipboard '#CC8844'

# A real PipeWire shutdown freezes the last frame with its magnifier intact.
"$binary" --pick
share
wait_pixel 700 450 '#CC8844'
kill "$pipewire_pid"
for ((attempt=0; attempt<60; attempt++)); do
  if grep -q 'Keeping the frozen desktop' "$test_root/app.log"; then break; fi
  sleep 0.1
done
grep -q 'Keeping the frozen desktop' "$test_root/app.log"
printf '#22AA66\n' > "$test_root/color"
sleep 0.2
wait_pixel 300 300 '#CC8844'
wait_pixel 700 450 '#CC8844'
snapshot frozen
"$probe" region 0 660 1280 140 '#CC8844' # No stale or animated controls in the capture.
"$probe" region 677 536 165 66 '#CC8844' # No overlay feedback in the magnified pixels.
xdotool key Return
main=$(wait_window '^Open Color Picker$')
assert_clipboard '#CC8844'

# Restart the external application and verify persistent selection/favorites.
"$binary" --quit
wait "$app_pid"
start_service restart "$binary"
main=$(wait_window '^Open Color Picker$')
xdotool windowactivate --sync "$main" key ctrl+shift+c
assert_clipboard '#CC8844'
if grep -Ei 'panicked|CRITICAL|assertion.*failed' "$test_root/app.log" "$test_root/restart.log"; then exit 1; fi
printf 'X11 E2E passed: real portal consent/cancel, PipeWire frames, live magnifier, selection, session cleanup, frozen fallback, clipboard and persistence.\n'

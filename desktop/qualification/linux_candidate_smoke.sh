#!/usr/bin/env bash
# Test the installed, unmodified ARM64 release archive on a disposable desktop.
set -euo pipefail
umask 077
[[ "$RUNNER_ENVIRONMENT" == github-hosted && $(uname -m) == aarch64 && $(id -u) != 0 ]]
[[ $# == 3 && -n "$DISPLAY" && -n "$DBUS_SESSION_BUS_ADDRESS" ]]
archive=$(realpath "$1")
revision=$2
proof=$(realpath -m "$3")
[[ "$revision" =~ ^[0-9a-f]{40}$ && ! -e "$proof" ]]
mkdir "$proof"
fixture=$(mktemp -d "$HOME/mcw-arm-gui.XXXXXXXX")
export XDG_CONFIG_HOME="$fixture/config" XDG_DATA_HOME="$fixture/data"
export XDG_CACHE_HOME="$fixture/cache" XDG_STATE_HOME="$fixture/state" XDG_RUNTIME_DIR="$fixture/runtime"
mkdir -p "$XDG_CONFIG_HOME/MicCamWatch" "$XDG_DATA_HOME" "$XDG_CACHE_HOME" "$XDG_STATE_HOME" "$XDG_RUNTIME_DIR"
printf '%s\n' 'notifications_enabled = false' 'sound_enabled = false' \
    'history_enabled = false' 'mute_on_lock = false' > "$XDG_CONFIG_HOME/MicCamWatch/settings.toml"

(cd "$(dirname "$archive")" && sha256sum --strict --check SHA256SUMS) | tee "$proof/checksums.log"
digest=$(sha256sum "$archive")
digest=${digest%% *}
python3 -B installer/hybrid_linux_install.py --root "$fixture/install" install "$archive" --sha256 "$digest" \
    | tee "$proof/install.log"
payload="$fixture/install/bundle"
python3 -B - "$payload/miccamwatch-bundle.json" "$revision" <<'PY'
import json, sys
with open(sys.argv[1]) as stream:
    manifest = json.load(stream)
if (manifest["revision"], manifest["target"], manifest["version"], manifest["ipc"]) != (
    sys.argv[2], "linux-aarch64", "0.22.0", 10
):
    raise ValueError("Unexpected installed ARM64 candidate")
PY
engine="$payload/mcw"
frontend="$payload/desktop/miccamwatch_desktop"
cc desktop/qualification/linux_xwindow.c -O2 -Wall -Wextra -Werror -lX11 -o "$fixture/xwindow"
openbox > "$proof/openbox.log" 2>&1 &
manager=$!
panel=
owner=
cleanup() {
    if [[ -n "$owner" && $(readlink "/proc/$owner/exe" 2>/dev/null) == "$engine" ]]; then
        timeout 15 "$engine" tray stop >> "$proof/owner.log" 2>&1 || true
    fi
    if [[ -n "$panel" ]]; then kill "$panel" 2>/dev/null || true; fi
    kill "$manager" 2>/dev/null || true
}
trap cleanup EXIT
dbus-update-activation-environment DISPLAY XAUTHORITY HOME XDG_CONFIG_HOME XDG_DATA_HOME XDG_CACHE_HOME XDG_STATE_HOME XDG_RUNTIME_DIR
xfconf-query -c xfce4-panel -p /configver -n -t int -s 2
xfconf-query -c xfce4-panel -p /panels -n -a -t int -s 1
xfconf-query -c xfce4-panel -p /panels/panel-1/position -n -t string -s 'p=6;x=640;y=20'
xfconf-query -c xfce4-panel -p /panels/panel-1/length -n -t uint -s 100
xfconf-query -c xfce4-panel -p /panels/panel-1/plugin-ids -n -a -t int -s 1
xfconf-query -c xfce4-panel -p /plugins/plugin-1 -n -t string -s systray
xfce4-panel --disable-wm-check > "$proof/panel.log" 2>&1 &
panel=$!
for attempt in $(seq 1 40); do
    if gdbus call --session --dest org.kde.StatusNotifierWatcher --object-path /StatusNotifierWatcher \
        --method org.freedesktop.DBus.Properties.Get org.kde.StatusNotifierWatcher IsStatusNotifierHostRegistered \
        2>/dev/null | grep true > "$proof/sni-ready.log"; then
        break
    fi
    sleep 0.5
done
[[ -s "$proof/sni-ready.log" ]]
"$engine" tray open > "$proof/owner.log" 2>&1 &
owner=$!

owned_child() {
    for pid in $(cat "/proc/$owner/task/$owner/children" 2>/dev/null); do
        if [[ $(readlink "/proc/$pid/exe" 2>/dev/null) == "$frontend" ]]; then
            printf '%s\n' "$pid"
        fi
    done
}

for cycle in 1 2; do
    child=
    for attempt in $(seq 1 80); do
        [[ $(readlink "/proc/$owner/exe" 2>/dev/null) == "$engine" ]] || break
        child=$(owned_child)
        if [[ "$child" =~ ^[0-9]+$ ]] && "$fixture/xwindow" "$child" info \
            > "$proof/window-$cycle.json" 2> "$proof/window-$cycle.log"; then
            break
        fi
        sleep 0.5
    done
    [[ -s "$proof/window-$cycle.json" && "$(owned_child)" == "$child" ]]
    "$fixture/xwindow" "$child" close > "$proof/close-$cycle.json"
    for attempt in $(seq 1 60); do
        [[ -e "/proc/$child" ]] || break
        sleep 0.5
    done
    [[ ! -e "/proc/$child" && $(readlink "/proc/$owner/exe") == "$engine" ]]
    "$engine" tray status | tee "$proof/tray-after-close-$cycle.log"
    grep -Fx running "$proof/tray-after-close-$cycle.log"
    if [[ "$cycle" == 1 ]]; then timeout 20 "$engine" tray open; fi
done
timeout 20 "$engine" tray stop
wait "$owner"
"$engine" uninstall | tee "$proof/uninstall.log"
[[ ! -e "$payload" ]]
printf 'status=passed\nsource=%s\narchive_sha256=%s\ncycles=2\nrenderer=hosted-Xvfb\n' "$revision" "$digest" > "$proof/result.txt"

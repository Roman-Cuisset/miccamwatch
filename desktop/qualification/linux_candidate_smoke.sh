#!/usr/bin/env bash
# Test the installed release archive and the actual SNI menu on a disposable desktop.
set -euo pipefail
umask 077
[[ "$RUNNER_ENVIRONMENT" == github-hosted && $(id -u) != 0 ]]
arch=$(uname -m)
[[ "$arch" == aarch64 || "$arch" == x86_64 ]]
[[ $# == 3 && -n "$DISPLAY" && -n "$DBUS_SESSION_BUS_ADDRESS" ]]
archive=$(realpath "$1")
revision=$2
proof=$(realpath -m "$3")
[[ "$revision" =~ ^[0-9a-f]{40}$ && ! -e "$proof" ]]
mkdir "$proof"
fixture=$(mktemp -d "$HOME/mcw-release-gui.XXXXXXXX")
export LANG=C LC_ALL=C
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
python3 -B - "$payload/miccamwatch-bundle.json" "$revision" "$arch" <<'PY'
import json, sys
sys.path.insert(0, "installer")
from hybrid_payload import release_metadata
with open(sys.argv[1]) as stream:
    manifest = json.load(stream)
version, ipc = release_metadata()
if (manifest["revision"], manifest["target"], manifest["version"], manifest["ipc"]) != (
    sys.argv[2], "linux-" + sys.argv[3], version, ipc
):
    raise ValueError("Unexpected installed Linux candidate")
PY
engine="$payload/mcw"
frontend="$payload/desktop/miccamwatch_desktop"
{
    ldconfig -p | grep -E 'lib(EGL|GLESv2)\.so\.'
    ldd "$engine"
    ldd "$frontend"
    ldd "$payload/desktop/lib/libflutter_linux_gtk.so"
} > "$proof/dependencies.log"
grep -Fq 'libEGL.so.1' "$proof/dependencies.log"
grep -Fq 'libGLESv2.so.2' "$proof/dependencies.log"
if grep -Fq 'not found' "$proof/dependencies.log"; then exit 1; fi
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
"$engine" tray run > "$proof/owner.log" 2>&1 &
owner=$!

owned_child() {
    local pids
    pids=$(cat "/proc/$owner"/task/*/children 2>/dev/null || true)
    for pid in $pids; do
        if [[ $(readlink "/proc/$pid/exe" 2>/dev/null) == "$frontend" ]]; then
            printf '%s\n' "$pid"
        fi
    done
}

for cycle in 1 2; do
    [[ -z "$(owned_child)" ]]
    /usr/bin/python3 -B - "$owner" > "$proof/menu-$cycle.json" <<'PY'
import os, sys, time, json
from gi.repository import Gio, GLib
sys.path.insert(0, "desktop/qualification")
from linux import sni_owned
pid = int(sys.argv[1])
for _ in range(40):
    item = sni_owned(pid, os.environ)
    if item is not None:
        break
    time.sleep(.25)
else:
    raise RuntimeError("Installed owner did not register its StatusNotifierItem")
bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
def call(method, args, reply=None):
    return bus.call_sync(item["service"], item["menu_path"], "com.canonical.dbusmenu",
                         method, args, reply, Gio.DBusCallFlags.NONE, 5000, None)

layout = call("GetLayout", GLib.Variant("(iias)", (0, 1, [])),
              GLib.VariantType.new("(u(ia{sv}av))")).unpack()[1]
labels = [(row[0], row[1].get("label")) for row in layout[2]]
# This is the first item in LinuxTray::menu, populated from the product's
# localized Open label. Reject any unrelated or duplicate item.
matches = [ident for ident, label in labels if label == "Open MicCamWatch…"]
if len(matches) != 1 or not labels or matches[0] != labels[0][0]:
    raise RuntimeError(f"Installed native Open action not found: {labels!r}")
call("Event", GLib.Variant("(isvu)", (matches[0], "clicked", GLib.Variant("i", 0), 0)))
print(json.dumps({"pid": pid, "action": "Open MicCamWatch…", "menu_id": matches[0]}))
PY
    child=
    for attempt in $(seq 1 80); do
        [[ $(readlink "/proc/$owner/exe" 2>/dev/null) == "$engine" ]] || break
        child=$(owned_child)
        if [[ "$child" =~ ^[0-9]+$ ]] &&
            grep -F "desktop event=ready endpoint=parent_received pid=$child " "$proof/owner.log" >/dev/null &&
            "$fixture/xwindow" "$child" info \
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
done
timeout 20 "$engine" tray stop
wait "$owner"
# Package removal is qualified separately on the fresh native container.
# This hosted image's /usr/share ownership is rejected by the root helper guard.
printf 'status=passed\nsource=%s\narchive_sha256=%s\ncycles=2\nentrypoint=native-SNI-menu\nrenderer=hosted-Xvfb\n' "$revision" "$digest" > "$proof/result.txt"

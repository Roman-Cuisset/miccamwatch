#!/bin/bash
# A fresh prefix and XDG profile. Never use the daily configuration.
set -euo pipefail
umask 077
[[ $# == 2 && $(id -u) != 0 ]]
archive=$(realpath "$1")
receipt=$(realpath -m "$2")
fixture=$(mktemp -d "$HOME/mcw-managed.XXXXXXXX")
root="$fixture/install"
export XDG_CONFIG_HOME="$fixture/config"
export XDG_DATA_HOME="$fixture/data"
export XDG_STATE_HOME="$fixture/state"
export XDG_CACHE_HOME="$fixture/cache"
export XDG_RUNTIME_DIR="$fixture/runtime"
mkdir -m 700 -p "$XDG_CONFIG_HOME/MicCamWatch" "$XDG_DATA_HOME/MicCamWatch" "$XDG_STATE_HOME" "$XDG_CACHE_HOME" "$XDG_RUNTIME_DIR"
settings="$XDG_CONFIG_HOME/MicCamWatch/settings.toml"
history="$XDG_DATA_HOME/MicCamWatch/history.jsonl"
sha=$(sha256sum "$archive")
sha=${sha%% *}
printf '%s\n' 'notifications_enabled = false' 'history_enabled = false' 'mute_on_lock = true' > "$settings"
printf '%s\n' '{"fixture":"preserve-history"}' > "$history"
python3 -B installer/hybrid_linux_install.py --root "$root" install "$archive" --sha256 "$sha"
engine="$root/bundle/mcw"
version=$("$engine" --version)
if flock -x "$root/.bundle.lock" "$engine" notifications status > "$fixture/lease-refusal.log" 2>&1; then
    echo 'Package lease refusal was expected' >&2
    exit 1
fi
grep -F 'package transaction in progress' "$fixture/lease-refusal.log"
"$engine" autostart enable
test -f "$XDG_CONFIG_HOME/autostart/com.roman-cuisset.miccamwatch.desktop"
if "$engine" uninstall > "$fixture/protection-refusal.log" 2>&1; then
    echo 'Refusing protection was expected' >&2
    exit 1
fi
grep -F 'automatic protection is requested' "$fixture/protection-refusal.log"
test -x "$engine"
sed -i 's/mute_on_lock = true/mute_on_lock = false/' "$settings"
sha256sum "$settings" "$history" > "$fixture/data.sha256"
"$engine" uninstall
test ! -e "$root/bundle" && test ! -e "$root/.removed-bundle"
if find "$XDG_CONFIG_HOME/autostart" -maxdepth 1 -name '*.desktop' -print -quit | grep .; then
    echo 'Owned autostart entry remains' >&2
    exit 1
fi
sha256sum --check "$fixture/data.sha256"
python3 - "$receipt" "$sha" "$version" <<'PY'
import json, os, sys
from pathlib import Path
path, sha, version = sys.argv[1:]
Path(path).write_text(json.dumps({
    "status": "passed", "source": os.environ.get("GITHUB_SHA"), "archive_sha256": sha,
    "installed_version": version, "protection_preflight_refused": True, "exclusive_lease_refused": True,
    "owned_autostart_removed": True, "bundle_removed": True, "user_data_preserved": True,
    "graphical_launch_executed": False,
}, indent=2) + "\n")
PY

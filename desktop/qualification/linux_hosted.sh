#!/bin/bash
# Ubuntu22.04 GitHub-hosted x64 ONLY. Workflow owns pinned SDK setup and ephemeral
# apt prerequisites. This script never installs anything or changes permissions
# on an existing path. Do not run it on a user's Mint session or WSL B2 sysroot.
set -euo pipefail
umask 077
[[ $# == 1 && ${GITHUB_ACTIONS:-} == true && ${RUNNER_OS:-} == Linux ]] || { echo 'Expected new output on GitHub-hosted Linux' >&2; exit 64; }
[[ $(uname -m) == x86_64 && $(id -u) != 0 ]] || { echo 'Ordinary x64 runner user required' >&2; exit 1; }
. /etc/os-release
[[ $ID == ubuntu && $VERSION_ID == 22.04 && -n ${ImageVersion:-} ]] || { echo 'Pinned Ubuntu22.04 hosted baseline required' >&2; exit 1; }
repo=$(pwd -P)
out=$1
[[ ! -e $out ]] || { echo 'New output directory required' >&2; exit 1; }
mkdir -m 700 "$out"
out=$(realpath "$out")
export MCW_B3_OUTPUT="$out"
target="${RUNNER_TEMP:?}/mcw-b3-target-${GITHUB_RUN_ID:?}-${GITHUB_RUN_ATTEMPT:?}"
mkdir -m 700 "$target"
export CARGO_TARGET_DIR="$target"
[[ $(rustc --version) == rustc\ 1.98.1\ * ]] || { echo 'Wrong Rust SDK' >&2; exit 1; }
flutter --version --machine > "$out/flutter-sdk.json"
python3 -c 'import json,os; from pathlib import Path; p=Path(os.environ["MCW_B3_OUTPUT"]); v=json.loads((p/"flutter-sdk.json").read_text()); assert v["frameworkVersion"]=="3.47.8" and v["frameworkRevision"]=="737ba8f885dc774b53b442759999a312c9461e4e",v'
{
  cargo clippy --all-targets --locked --features flutter-desktop -- -D warnings
  cargo test --lib --locked --features flutter-desktop frontends::desktop
  cargo test --lib --locked --features flutter-desktop settings::tests
  cargo build --locked --release --features flutter-desktop --bin mcw
  (cd desktop/flutter && flutter pub get --enforce-lockfile && flutter analyze --no-pub && flutter test --no-pub && flutter build linux --release --no-pub)
  cc desktop/qualification/linux_xwindow.c -O2 -Wall -Wextra -Werror -lX11 -o "$out/linux_xwindow"
  dpkg-query -W > "$out/toolchain-packages.txt"
  python3 - <<'PY'
import json,os,subprocess
from pathlib import Path
out=Path(os.environ['MCW_B3_OUTPUT'])
release={}
for line in Path('/etc/os-release').read_text().splitlines():
    if '=' in line:
        key,value=line.split('=',1); release[key]=value.strip('"')
receipt={'baseline':'ubuntu:22.04','builder':'github-hosted-ubuntu-22.04',
         'host_os':release,'runner_image_version':os.environ['ImageVersion'],
         'runner_image_os':os.environ.get('ImageOS'),
         'source_revision':os.environ['GITHUB_SHA'],
         'source_status':subprocess.check_output(['git','status','--porcelain'],text=True),
         'rust':subprocess.check_output(['rustc','--version'],text=True),
         'flutter':json.loads((out/'flutter-sdk.json').read_text()),
         'gtk_build':subprocess.check_output(['pkg-config','--modversion','gtk+-3.0'],text=True),
         'packages':(out/'toolchain-packages.txt').read_text()}
(out/'build-receipt.json').write_text(json.dumps(receipt,indent=2))
PY
  python3 desktop/qualification/linux.py package --engine "$CARGO_TARGET_DIR/release/mcw" \
    --desktop desktop/flutter/build/linux/x64/release/bundle --out "$out/bundle" \
    --revision "$GITHUB_SHA" --build-receipt "$out/build-receipt.json"
  python3 desktop/qualification/linux.py check --prefix "$out/bundle" > "$out/runtime-preflight.json"
  tar -C "$out" -czf "$out/miccamwatch-b3-linux-x64-qualification.tar.gz" bundle
  sha256sum "$out/miccamwatch-b3-linux-x64-qualification.tar.gz" > "$out/archive.SHA256SUMS"
} 2>&1 | tee "$out/build.log"

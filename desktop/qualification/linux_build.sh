#!/bin/bash
# Run only on Linux x64, as an ordinary user with ALREADY configured rootless
# Podman. No sudo/host installation/root permission changes. Existing B2 SDKs
# are reused in a NEW private copy, never downloaded or set up again.
# Usage: bash linux_build.sh REPO B2_SDK_ROOT NEW_OUTPUT ubuntu@sha256:DIGEST REVISION
# Obtain DIGEST separately from official Ubuntu image metadata; never substitute
# a guessed digest. The base must be the official Ubuntu22.04 amd64 image.
set -euo pipefail
umask 077
[[ $# == 5 ]] || { echo 'Expected REPO B2_SDK_ROOT NEW_OUTPUT ubuntu@sha256:DIGEST REVISION' >&2; exit 64; }
repo=$(realpath "$1"); sdk=$(realpath "$2"); out=$3; image=$4; revision=$5
[[ $(id -u) != 0 && $(uname -m) == x86_64 ]] || { echo 'Ordinary Linux x64 user required' >&2; exit 1; }
[[ $image =~ ^ubuntu@sha256:[0-9a-f]{64}$ ]] || { echo 'Official digest-pinned Ubuntu22.04 image required' >&2; exit 1; }
[[ $(podman info --format '{{.Host.Security.Rootless}}') == true ]] || { echo 'Existing rootless Podman required; do not install/configure it here' >&2; exit 1; }
[[ ! -e $out ]] || { echo 'Output must be new' >&2; exit 1; }
mkdir -m 700 "$out"; out=$(realpath "$out")
mkdir -m 700 "$out/src" "$out/sdk" "$out/home" "$out/cargo-home" "$out/pub-cache"
# No old target/CMake/host-generated Flutter files cross the baseline boundary.
tar -C "$repo" --exclude=.git --exclude=target --exclude=desktop/flutter/build --exclude=desktop/flutter/.dart_tool --exclude=desktop/flutter/linux/flutter/ephemeral -cf - . | tar -C "$out/src" -xf -
cp -a --reflink=auto "$sdk/rust" "$sdk/flutter" "$out/sdk/"
cp -a --reflink=auto "$sdk/cargo-home/." "$out/cargo-home/"
cp -a --reflink=auto "$sdk/pub-cache/." "$out/pub-cache/"
mkdir -m 700 "$out/toolchain-context"
cat > "$out/Containerfile" <<'EOF'
ARG BASE
FROM ${BASE}
RUN test "$(. /etc/os-release; echo "$VERSION_ID")" = 22.04 && \
    apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    ca-certificates git curl unzip xz-utils zip clang cmake ninja-build pkg-config \
    libgtk-3-dev liblzma-dev libstdc++-12-dev libpipewire-0.3-dev libclang-dev \
    python3 binutils libx11-dev && rm -rf /var/lib/apt/lists/*
EOF
# Container root is namespaced by an existing rootless runtime; never host root.
# Retain the complete toolchain OCI image so the apt result can be replayed even
# if repository versions change. Receipts alone would not make apt reproducible.
podman build --build-arg "BASE=$image" --iidfile "$out/toolchain-image.id" -f "$out/Containerfile" "$out/toolchain-context"
toolchain=$(<"$out/toolchain-image.id")
podman save --format oci-archive --output "$out/toolchain.oci.tar" "$toolchain"
podman run --rm --network=slirp4netns --userns=keep-id --user "$(id -u):$(id -g)" \
    -v "$out:/work:rw" --workdir /work/src \
    -e HOME=/work/home -e CARGO_HOME=/work/cargo-home -e PUB_CACHE=/work/pub-cache \
    -e PATH=/work/sdk/rust/bin:/work/sdk/flutter/bin:/usr/bin:/bin \
    -e FLUTTER_SUPPRESS_ANALYTICS=true -e DART_SUPPRESS_ANALYTICS=true -e CI=true \
    -e "MCW_BASE_IMAGE=$image" -e "MCW_TOOLCHAIN=$toolchain" -e "MCW_REVISION=$revision" \
    "$toolchain" bash -euc '
      test "$(. /etc/os-release; echo "$VERSION_ID")" = 22.04
      rustc --version; flutter --version
      cargo build --locked --release --features flutter-desktop --bin mcw
      cd desktop/flutter
      flutter pub get --offline
      flutter build linux --release --no-pub
      cd /work/src
      cc desktop/qualification/linux_xwindow.c -O2 -Wall -Wextra -Werror -lX11 -o /work/linux_xwindow
      dpkg-query -W > /work/toolchain-packages.txt
      python3 -c '\''import json,os,subprocess; from pathlib import Path; Path("/work/build-receipt.json").write_text(json.dumps({"baseline":"ubuntu:22.04","image":os.environ["MCW_BASE_IMAGE"],"toolchain_image":os.environ["MCW_TOOLCHAIN"],"source_revision":os.environ["MCW_REVISION"],"rust":subprocess.check_output(["rustc","--version"],text=True),"flutter":subprocess.check_output(["flutter","--version"],text=True),"gtk_build":subprocess.check_output(["pkg-config","--modversion","gtk+-3.0"],text=True),"packages":Path("/work/toolchain-packages.txt").read_text()},indent=2))'\''
      python3 desktop/qualification/linux.py package --engine target/release/mcw \
        --desktop desktop/flutter/build/linux/x64/release/bundle --out /work/bundle \
        --revision "$MCW_REVISION" --build-receipt /work/build-receipt.json
    ' 2>&1 | tee "$out/build.log"
sha256sum "$out/toolchain.oci.tar" "$out/build-receipt.json" > "$out/build-evidence.SHA256SUMS"
printf 'Bundle: %s/bundle\nPrivate toolchain/evidence: %s\n' "$out" "$out"

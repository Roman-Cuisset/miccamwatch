#!/bin/bash
# Use the actual compiler host when its ownership boundaries qualify. If the
# hosted tool image has writable system parents, use an ordinary user in a
# fresh native Ubuntu 22.04 container with its own root camera namespace.
set -euo pipefail
[[ $# == 2 && ${RUNNER_ENVIRONMENT:-} == github-hosted && $(id -u) != 0 ]]
distribution=$(realpath "$(dirname "$1")")
archive=$(basename "$1")
receipt="$2"
log="$distribution/hosted-install.log"
python3 -B installer/tests/test_hybrid_linux_install.py -v
if bash installer/tests/linux-hybrid-installed.sh "$1" "$receipt" >"$log" 2>&1; then
    cat "$log"
    exit 0
fi
cat "$log"
if ! grep -Eq '^mcw: (Root camera directory .* is unsafe or inaccessible|Cannot inspect root camera directory)' "$log"; then
    exit 1
fi
echo 'Hosted system directory ownership blocked the installation fixture. Trying a fresh native Ubuntu 22.04 container.'
case "$(uname -m)" in
    x86_64) architecture=amd64 ;;
    aarch64) architecture=arm64 ;;
    *) exit 1 ;;
esac
docker run --rm --platform "linux/$architecture" \
    --mount "type=bind,src=$(pwd -P),dst=/source,readonly" \
    --mount "type=bind,src=$distribution,dst=/distribution" \
    --env "MCW_UID=$(id -u)" --env "MCW_ARCHIVE=$archive" --env "GITHUB_SHA=$GITHUB_SHA" \
    ubuntu:22.04 /bin/bash -ceu '
        export DEBIAN_FRONTEND=noninteractive
        apt-get update -qq
        apt-get install -y --no-install-recommends python3 libpipewire-0.3-0 util-linux > /tmp/mcw-apt.log ||
            { cat /tmp/mcw-apt.log; exit 1; }
        useradd -m -u "$MCW_UID" mcwqa
        test "$(getconf GNU_LIBC_VERSION)" = "glibc 2.35"
        cd /source
        runuser -u mcwqa -- env GITHUB_SHA="$GITHUB_SHA" MCW_INSTALL_TEST_ENV=ubuntu-22.04-container \
            /bin/bash installer/tests/linux-hybrid-installed.sh "/distribution/$MCW_ARCHIVE" /distribution/installed-lifecycle.json
    '

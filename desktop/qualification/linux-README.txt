LINUX B3 PRIVATE QUALIFICATION (not a release installer)

Compatibility findings from immutable B2 runtime-final-9aa4689:
- mcw requires GLIBC_2.39. It cannot run on Mint21/Ubuntu22.04 GLIBC2.35.
- Flutter runner: GLIBC2.34, GLIBCXX3.4, CXXABI1.3.
- Official libflutter_linux_gtk.so: GLIBC2.17 strong,2.18 weak.
- B2 compiled GTK tooling came from Kali GTK3.24.52/t64. Unversioned GTK
  imports do not establish a portable GTK minimum. Do NOT ship that sysroot
  or assume that a Kali build is compatible with all Mint releases.

Conservative target: official Ubuntu22.04 amd64 baseline, Mint21.x/22.x.
linux.py audits every payload ELF against GLIBC2.35, GLIBCXX3.4.30 and
CXXABI1.3.13 ceilings. It records complete DT_NEEDED and RPATH requirements.
GTK minimum is grounded in baseline build provenance, not guessed from ELF.
Actual Mint runtime preflight remains mandatory. No accelerated-GPU, native
Wayland, physical hardware or SNI visual certification is inferred from WSLg.

Build only when the parent coordinates its verification/build wave:

  # Existing B2 SDKs: do not install/download/setup them again.
  sdk=/home/roman/mcw-qualification/b2-20261029-082656
  repo=/absolute/path/to/current/source
  # Rootless Podman must ALREADY be configured. No sudo or host package install.
  test "$(podman info --format '{{.Host.Security.Rootless}}')" = true
  podman pull docker.io/library/ubuntu:22.04
  digest=$(podman image inspect docker.io/library/ubuntu:22.04 --format '{{index .RepoDigests 0}}')
  image="ubuntu@${digest#*@}"
  revision=$(git -C "$repo" rev-parse HEAD)
  out="$HOME/mcw-qualification/ubuntu22-b3-$(date +%s)"
  bash "$repo/desktop/qualification/linux_build.sh" "$repo" "$sdk" "$out" "$image" "$revision"

This builds only inside a NEW private source/SDK copy. Namespaced container
root installs build dependencies in a disposable rootless image, NEVER host
root. No Docker daemon, privileged container, host graphics/device mount,
chown/chmod of existing paths or permission/group/ptrace change is requested.
If rootless Podman is absent, STOP this path: do not install it, sudo, relabel
the Kali binary, or claim an Ubuntu baseline build happened. Parent may use
existing B2 env for a clearly HOST-ONLY bundle instead:

  # After parent has rebuilt native/Flutter outputs in its existing source mirror:
  "$sdk/env.sh" python3 "$repo/desktop/qualification/linux.py" package \
    --engine /absolute/new/mcw --desktop /absolute/new/Flutter/bundle \
    --out "$HOME/mcw-qualification/b3-host-only-NEW" --revision "$revision"

Retained baseline evidence: full toolchain.oci.tar, its SHA256, exact base image
digest, toolchain image ID, source revision, package inventory, SDK versions,
GTK build version, build.log, all ELF audits and bundle SHA256SUMS. A fresh
APT resolution alone is not reproducible; retain/load the OCI image to replay
that exact dependency environment. Source revision may describe a dirty tree:
parent must record its patch/working-tree identity with final evidence.

Mint runtime (ordinary desktop session, no installer):

  # Transfer/copy the COMPLETE bundle directory under a private owned HOME tree.
  cd "$out/bundle"
  sha256sum -c SHA256SUMS
  python3 linux.py check --prefix .
  python3 linux.py run --prefix . --tray
  # Native GTK fallback, all native mutation widgets disabled:
  python3 linux.py run --prefix . --tray --native

Each invocation creates a new 0700 ~/mcw-qualification/linux-b3-* tree with
separate XDG config/data/cache/runtime. It preserves existing display/bus and
read-only media-server namespaces; no fake snapshots are supplied. The stable
settings/history/autostart/protections/devices remain outside this namespace.
No installer/uninstaller or operational tray command is invoked. Quit only the
qualification tray, or Ctrl+C its terminal. Never use an unisolated tray stop.

Automated X11 evidence (owned processes/windows only):

  # Compile in a private output location; X11 development headers must already
  # exist, or use the baseline builder's $out/linux_xwindow. No host apt install.
  cc linux_xwindow.c -O2 -Wall -Wextra -Werror -lX11 -o "$HOME/mcw-qualification/linux_xwindow-NEW"
  python3 linux.py qualify --prefix . \
    --out "$HOME/mcw-qualification/b3-evidence-NEW" \
    --xwindow "$HOME/mcw-qualification/linux_xwindow-NEW" \
    --tray --cycles 3 --seconds 20 --stabilize 15 --captures --preferences

--tray requires an ACTUAL registered StatusNotifierWatcher. It reads its real
SNI item/menu, PID-matches and pins the unique D-Bus owner, and opens via that
item's actual Activate method (not global pointer/keyboard input). No fake SNI
service is created. SNI absence fails explicitly. Omit --tray on WSLg only if
window-only evidence is intended; it is NOT tray qualification.

--preferences requires the desktop's existing AT-SPI service/libatspi and
English labels (qualification launcher uses C.UTF-8). It scopes actions to the
owned Flutter PID/generation, exercises real sound writes/readback/restore,
server-generated pause/resume/deadline, and an actual persistence-error fixture
inside ONLY its disposable settings tree. The temporary fixture is restored
in finally; there is no permission mutation. Accessibility absence fails; no
global gsettings toggle, whole-desktop input/capture or dependency install is
used as a fallback. Do not interact with the qualification UI during automation.

Native Wayland lifecycle/screenshot automation is NOT implemented by this X11
helper. Run interactively under Wayland or record this gate as unexecuted; do
not use a whole-screen screenshot/global-input substitute. X11 captures are
owned-window PPMs; a successful capture call is not visual approval. Parent
must examine actual pixels. Logs, AT-SPI text, settings and screenshots are
PRIVATE evidence and can identify real applications; privacy-review before sharing.

Resource evidence records process-tree RSS/PSS and cumulative CPU endpoints.
CPU is percent of one logical core; shared RSS can be double-counted and
short-lived children are a lower-bound limitation. Renderer evidence is from
the actual Flutter child's threads, mapped libraries and DRI descriptors.
llvmpipe/lavapipe worker names establish software rendering; driver mappings or
an open DRI fd alone do NOT establish hardware acceleration, and are labeled
unknown. No GPU configuration/cadence change is made for better measurements.

Prepared files are not an executed qualification result. Parent should run
Python syntax checks, bash -n, native check/Clippy/tests, Flutter analyze/tests,
release builds, package/runtime preflight and the relevant real desktop runs
once contributions are integrated. Keep failed logs/receipts; no success claim
for missing Mint/SNI/accessibility/Wayland/accelerated hardware.

Alternative coordinated hosted baseline:
The manual Flutter qualification workflow may use a fresh official Ubuntu22.04
x64 GitHub-hosted runner, with SDKs pinned to the same official versions/hashes.
Only that disposable runner may install build/Xvfb/XFCE/accessibility packages.
No local or stable-machine package installation is implied.

  # After workflow-provided pinned SDKs/dependencies; ordinary hosted user:
  test -d "$HOME/mcw-qualification" || mkdir -m700 "$HOME/mcw-qualification"
  output="$HOME/mcw-qualification/b3-hosted-$GITHUB_RUN_ID-$GITHUB_RUN_ATTEMPT"
  bash desktop/qualification/linux_hosted.sh "$output"
  dbus-run-session -- xvfb-run -a -s '-screen 0 1280x1000x24' \
    python3 -B desktop/qualification/linux_hosted.py \
      --prefix "$output/bundle" --out "$output/runtime" \
      --xwindow "$output/linux_xwindow"

The hosted build saves actual Ubuntu release, runner image version, package
inventory, SDK identities, source status, ELF ceilings, complete bundle,
SHA256 manifest and qualification tar.gz. Build targets stay outside the
artifact root in RUNNER_TEMP. Hosted runtime saves a real private XFCE SNI menu,
actual D-Bus activation, owned-window screenshots/cycles/resources and a
separate preference/error pass. Xvfb software rendering is expected; hosted
Ubuntu evidence is not physical Mint, Wayland or accelerated-GPU certification.
Remote workflow execution requires the user's separately authorized push/run;
prepared YAML/source is never reported as an already passing hosted result.

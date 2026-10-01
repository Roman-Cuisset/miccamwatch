#!/bin/sh
# Standalone per-user installer. Receipt files are data, never shell code.
set -eu
umask 077
LC_ALL=C
export LC_ALL

REPOSITORY=https://github.com/Roman-Cuisset/miccamwatch
VERSION=latest
PREFIX=
PATH_MODE=ask
UNINSTALL=no
EXPLICIT_PREFIX=no
WORK=
LOCK=
STAGE=
BACKUP=
CONFIG_WORK=

say() { printf '%s\n' "$*"; }
warn() { printf 'mcw installer: %s\n' "$*" >&2; }
die() { warn "$*"; exit 1; }
cleanup() {
    [ -z "$STAGE" ] || rm -f "$STAGE"
    [ -z "$BACKUP" ] || rm -f "$BACKUP"
    [ -z "$CONFIG_WORK" ] || rm -rf "$CONFIG_WORK"
    [ -z "$WORK" ] || rm -rf "$WORK"
    [ -z "$LOCK" ] || rmdir "$LOCK" 2>/dev/null || :
}
trap cleanup 0
trap 'exit 130' INT
trap 'exit 143' TERM HUP

usage() {
    cat <<'HELP'
Install MicCamWatch for the current user (no sudo).

Usage: sh install.sh [--version v0.14.0] [--prefix ABSOLUTE_PREFIX]
                     [--add-path | --no-modify-path]
       sh install.sh --uninstall [--prefix ABSOLUTE_PREFIX]
       sh install.sh --help

--version           Release tag; default: latest, resolved before downloading.
--prefix            Default: $HOME/.local. Executable: PREFIX/bin/mcw.
--add-path          Explicitly add managed blocks to this user's shell startup files.
--no-modify-path    Never change shell startup files.
--uninstall         Remove only receipt-owned, unchanged executable/PATH blocks.
--help              Show this help without making changes.

Without a PATH flag, an interactive terminal is asked (default: no).
Noninteractive installs never prompt or change PATH without --add-path.
The child installer cannot change the parent shell: open a new terminal afterward.
Supported: Linux x86_64 with glibc 2.35+, macOS 15+ on Intel/Apple Silicon.
A Rosetta-translated shell receives the native Apple Silicon executable.
No autostart, device controls, elevation or Gatekeeper changes are performed.
Checksums protect integrity, not independent publisher authenticity.
HELP
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --help) usage; exit 0 ;;
        --version)
            [ "$#" -ge 2 ] || die '--version requires a release tag.'
            VERSION=$2; shift 2 ;;
        --prefix)
            [ "$#" -ge 2 ] || die '--prefix requires an absolute directory.'
            PREFIX=$2; EXPLICIT_PREFIX=yes; shift 2 ;;
        --add-path)
            [ "$PATH_MODE" != never ] || die '--add-path and --no-modify-path are mutually exclusive.'
            PATH_MODE=add; shift ;;
        --no-modify-path)
            [ "$PATH_MODE" != add ] || die '--add-path and --no-modify-path are mutually exclusive.'
            PATH_MODE=never; shift ;;
        --uninstall) UNINSTALL=yes; shift ;;
        *) die "Unknown option: $1 (use --help)." ;;
    esac
done

valid_tag() {
    printf '%s\n' "$1" | awk '/^v[0-9]+\.[0-9]+\.[0-9]+([-+][A-Za-z0-9][A-Za-z0-9.-]*)?$/ { ok=1 } END { exit !ok }'
}
[ "$VERSION" = latest ] || valid_tag "$VERSION" || die 'Use a release tag such as v0.14.0.'
[ "$(id -u)" != 0 ] || die 'Run this installer as your ordinary user, without sudo or root.'
[ -n "${HOME:-}" ] || die 'HOME is not set.'
case "$HOME" in /*) ;; *) die 'HOME must be an absolute path.' ;; esac
if [ "$EXPLICIT_PREFIX" = no ]; then PREFIX=$HOME/.local; fi
# Newlines/control bytes cannot be represented safely in receipt or startup files;
# a colon cannot be represented as one PATH component.
valid_path() {
    case "$1" in /*) ;; *) return 1 ;; esac
    case "$1" in *:*) return 1 ;; esac
    case "$1" in *'
'*) return 1 ;; esac
    printf '%s' "$1" | LC_ALL=C grep '[[:cntrl:]]' >/dev/null 2>&1 && return 1
    return 0
}
valid_path "$PREFIX" || die 'Prefix must be absolute and contain no colon or control characters.'
valid_path "$HOME" || die 'HOME contains unsupported colon/control characters.'

OS=$(uname -s)
ARCH=$(uname -m)
case "$OS" in
    Linux)
        [ "$ARCH" = x86_64 ] || die "Unsupported Linux architecture: $ARCH (only x86_64 is published)."
        command -v getconf >/dev/null 2>&1 || die 'getconf is required to verify glibc.'
        GLIBC=$(getconf GNU_LIBC_VERSION 2>/dev/null) || die 'Linux requires glibc 2.35+; musl is not supported.'
        printf '%s\n' "$GLIBC" | awk '$1=="glibc" && $2 ~ /^[0-9]+\.[0-9]+$/ { split($2,v,"."); ok=(v[1]>2 || (v[1]==2 && v[2]>=35)) } END { exit !ok }' || die "Unsupported C library: $GLIBC; glibc 2.35+ is required."
        PLATFORM=linux; ASSET_ARCH=x86_64 ;;
    Darwin)
        MACOS=$(sw_vers -productVersion) || die 'Cannot determine the macOS version.'
        printf '%s\n' "$MACOS" | awk '/^[0-9]+\.[0-9]+(\.[0-9]+)?$/ { split($0,v,"."); ok=(v[1]>=15) } END { exit !ok }' || die "macOS 15+ is required; found $MACOS."
        TRANSLATED=$(sysctl -n sysctl.proc_translated 2>/dev/null || :)
        if [ "$TRANSLATED" = 1 ]; then
            ARCH=arm64
            say 'Rosetta shell detected: selecting the native Apple Silicon archive.'
        fi
        case "$ARCH" in
            arm64) ASSET_ARCH=aarch64 ;;
            x86_64) ASSET_ARCH=x86_64 ;;
            *) die "Unsupported macOS architecture: $ARCH." ;;
        esac
        PLATFORM=macos ;;
    *) die "Unsupported operating system: $OS. Only Linux and macOS are supported." ;;
esac

for TOOL in awk grep sed tar mktemp cmp cp mv stat readlink dirname cat chmod rm rmdir mkdir; do
    command -v "$TOOL" >/dev/null 2>&1 || die "Required command is missing: $TOOL."
done
if [ "$OS" = Linux ]; then
    command -v sha256sum >/dev/null 2>&1 || die 'sha256sum is required.'
else
    command -v shasum >/dev/null 2>&1 || die 'shasum is required.'
    command -v lsof >/dev/null 2>&1 || die 'lsof is required to check whether the installed macOS executable is in use.'
fi
sha_file() {
    if [ "$OS" = Linux ]; then DIGEST_OUTPUT=$(sha256sum < "$1"); else DIGEST_OUTPUT=$(shasum -a 256 < "$1"); fi || return 1
    printf '%s\n' "${DIGEST_OUTPUT%% *}"
}
sha_text() {
    if [ "$OS" = Linux ]; then DIGEST_OUTPUT=$(sha256sum); else DIGEST_OUTPUT=$(shasum -a 256); fi || return 1
    printf '%s\n' "${DIGEST_OUTPUT%% *}"
}
link_count() {
    if [ "$OS" = Linux ]; then stat -c %h "$1"; else stat -f %l "$1"; fi
}
regular_file() {
    [ ! -L "$1" ] && [ -f "$1" ] && [ "$(link_count "$1")" = 1 ]
}

if [ "$UNINSTALL" = yes ] && [ ! -e "$PREFIX" ] && [ ! -L "$PREFIX" ]; then
    say "Nothing to uninstall at $PREFIX."
    exit 0
fi
[ ! -L "$PREFIX" ] || die 'Prefix itself must not be a symbolic link.'
mkdir -p "$PREFIX" || die "Cannot create prefix: $PREFIX."
PREFIX=$(CDPATH= cd "$PREFIX" && pwd -P) || die 'Cannot resolve prefix.'
valid_path "$PREFIX" || die 'Resolved prefix contains unsupported characters.'
BIN=$PREFIX/bin
TARGET=$BIN/mcw
STATE=$PREFIX/.miccamwatch-install
LOCK_PATH=$PREFIX/.miccamwatch-install.lock
mkdir "$LOCK_PATH" 2>/dev/null || die "Another installer is running, or $LOCK_PATH exists. After confirming no installer is running, remove only that empty lock directory."
LOCK=$LOCK_PATH
WORK=$(mktemp -d "$PREFIX/.mcw-install.XXXXXXXX") || die 'Cannot create a private temporary directory.'
[ ! -L "$BIN" ] || die "Refusing symbolic-link bin directory: $BIN."
[ ! -L "$STATE" ] || die 'Refusing symbolic-link receipt directory.'
if [ -e "$STATE" ]; then
    [ -d "$STATE" ] || die 'Receipt location is not a directory.'
    regular_file "$STATE/format" && [ "$(cat "$STATE/format")" = 1 ] || die 'Unrecognized installer receipt; refusing to overwrite it.'
    regular_file "$STATE/prefix" && [ "$(cat "$STATE/prefix")" = "$PREFIX" ] || die 'Receipt prefix mismatch; refusing to change files.'
    if [ -e "$STATE/binary.sha256" ] || [ -L "$STATE/binary.sha256" ]; then
        regular_file "$STATE/binary.sha256" || die 'Unsafe binary receipt.'
        awk 'length($0)!=64 || $0 ~ /[^0-9a-f]/ { bad=1 } END { exit bad || NR<1 || NR>2 }' "$STATE/binary.sha256" || die 'Invalid binary checksum receipt.'
    fi
    if [ -e "$STATE/version" ] || [ -L "$STATE/version" ]; then
        regular_file "$STATE/version" || die 'Unsafe version receipt.'
    fi
else
    if [ "$UNINSTALL" = yes ]; then
        say "No installer receipt at $PREFIX; no user files were removed."
        exit 0
    fi
    if [ -e "$TARGET" ] || [ -L "$TARGET" ]; then die "Refusing to overwrite unmanaged $TARGET. Choose another prefix or move it yourself."; fi
    mkdir "$STATE" || die 'Cannot create receipt directory.'
    printf '1\n' > "$STATE/format"
    printf '%s\n' "$PREFIX" > "$STATE/prefix"
fi

owned_binary() {
    if [ -e "$TARGET" ] || [ -L "$TARGET" ]; then
        regular_file "$TARGET" || die "Refusing nonregular, symlink or hardlinked executable: $TARGET."
        [ -f "$STATE/binary.sha256" ] || die "No ownership receipt for $TARGET; preserving it."
        CURRENT_HASH=$(sha_file "$TARGET") || die 'Cannot checksum installed executable.'
        grep -Fx "$CURRENT_HASH" "$STATE/binary.sha256" >/dev/null || die "Installed executable was changed outside the installer; preserving $TARGET."
    else
        CURRENT_HASH=
    fi
}
refuse_running() {
    if [ "$OS" = Linux ]; then
        [ -d /proc/self ] || die 'Cannot inspect /proc to check whether the installed executable is running.'
        for EXE in /proc/[0-9]*/exe; do
            RUNNING=$(readlink "$EXE" 2>/dev/null || :)
            [ "$RUNNING" != "$TARGET" ] || die 'The installed executable is in use. Stop it yourself and retry; no process was stopped.'
        done
    else
        if lsof -t "$TARGET" > "$WORK/running" 2> "$WORK/running-errors"; then
            die 'The installed executable is in use. Stop it yourself and retry; no process was stopped.'
        else
            LSTATUS=$?
            [ "$LSTATUS" = 1 ] && [ ! -s "$WORK/running-errors" ] || die 'Cannot check whether the installed executable is in use; refusing to replace/remove it.'
        fi
    fi
}
owned_binary

# Blocks are byte-for-byte snapshots. Never evaluate receipt contents.
# Return 0 only when exactly one complete unchanged block is present.
block_matches() {
    awk 'NR==FNR { b[++n]=$0; next } { a[++m]=$0 } END { hits=0; for(i=1;i<=m-n+1;i++) { same=1; for(j=1;j<=n;j++) if(a[i+j-1]!=b[j]) { same=0; break } if(same) hits++ } exit hits!=1 }' "$1" "$2"
}
remove_block() {
    RECORD=$1
    CONFIG=$(cat "$STATE/$RECORD.path")
    valid_path "$CONFIG" || die 'Unsafe PATH receipt filename.'
    if [ ! -e "$CONFIG" ] && [ ! -L "$CONFIG" ]; then
        rm -f "$STATE/$RECORD.path" "$STATE/$RECORD.block"
        return 0
    fi
    if regular_file "$CONFIG" && ! grep -F "$(printf '%s' "$PREFIX" | sha_text)" "$CONFIG" >/dev/null; then
        # A user already removed the managed markers. Keep their content and
        # discard only this now-unneeded ownership record.
        rm -f "$STATE/$RECORD.path" "$STATE/$RECORD.block"
        return 0
    fi
    if ! regular_file "$CONFIG" || ! block_matches "$STATE/$RECORD.block" "$CONFIG"; then
        warn "Preserving modified/ambiguous PATH configuration: $CONFIG (receipt retained)."
        return 1
    fi
    CONFIG_WORK=$(mktemp -d "$(dirname "$CONFIG")/.mcw-path.XXXXXXXX") || die 'Cannot stage PATH removal.'
    cp -p "$CONFIG" "$CONFIG_WORK/original" || die 'Cannot snapshot shell configuration.'
    cp -p "$CONFIG" "$CONFIG_WORK/new" || die 'Cannot stage shell configuration.'
    awk 'NR==FNR { b[++n]=$0; next } { a[++m]=$0 } END { start=0; for(i=1;i<=m-n+1;i++) { same=1; for(j=1;j<=n;j++) if(a[i+j-1]!=b[j]) { same=0; break } if(same) { start=i; break } } for(i=1;i<=m;i++) if(i<start || i>=start+n) print a[i] }' "$STATE/$RECORD.block" "$CONFIG_WORK/original" > "$CONFIG_WORK/new"
    regular_file "$CONFIG" && cmp -s "$CONFIG" "$CONFIG_WORK/original" || die "Shell configuration changed concurrently; preserving $CONFIG."
    mv -f "$CONFIG_WORK/new" "$CONFIG" || die 'Cannot atomically remove PATH block.'
    rm -rf "$CONFIG_WORK"; CONFIG_WORK=
    rm -f "$STATE/$RECORD.path" "$STATE/$RECORD.block"
}
validate_records() {
    # Reject unknown receipt content before deleting anything, so a failed
    # uninstall never destroys the metadata needed for a later safe retry.
    for ENTRY in "$STATE"/* "$STATE"/.[!.]* "$STATE"/..?*; do
        [ -e "$ENTRY" ] || [ -L "$ENTRY" ] || continue
        ENTRY_NAME=${ENTRY##*/}
        case "$ENTRY_NAME" in
            format|prefix|binary.sha256|version) ;;
            path-*.path|path-*.block)
                ENTRY_INDEX=${ENTRY_NAME#path-}; ENTRY_INDEX=${ENTRY_INDEX%.*}
                case "$ENTRY_INDEX" in ''|*[!0-9]*) die 'Invalid PATH receipt filename.' ;; esac ;;
            *) die "Unknown receipt content was preserved: $ENTRY." ;;
        esac
        regular_file "$ENTRY" || die "Unsafe receipt file was preserved: $ENTRY."
    done
    for PATH_RECORD in "$STATE"/path-*.path; do
        [ -e "$PATH_RECORD" ] || [ -L "$PATH_RECORD" ] || continue
        regular_file "$PATH_RECORD" || die 'Unsafe PATH receipt.'
        RECORD_NAME=${PATH_RECORD##*/}; RECORD_NAME=${RECORD_NAME%.path}
        RECORD_INDEX=${RECORD_NAME#path-}
        case "$RECORD_INDEX" in ''|*[!0-9]*) die 'Invalid PATH receipt.' ;; esac
        regular_file "$STATE/$RECORD_NAME.block" || die 'Missing or unsafe PATH block receipt.'
    done
}
validate_records

if [ "$UNINSTALL" = yes ]; then
    PARTIAL=no
    if [ -n "$CURRENT_HASH" ]; then
        refuse_running
        owned_binary
        rm -f "$TARGET" || die "Cannot remove $TARGET."
        say "Removed installer-owned $TARGET."
    fi
    rm -f "$STATE/binary.sha256" "$STATE/version"
    for PATH_RECORD in "$STATE"/path-*.path; do
        [ -e "$PATH_RECORD" ] || continue
        RECORD_NAME=${PATH_RECORD##*/}; RECORD_NAME=${RECORD_NAME%.path}
        remove_block "$RECORD_NAME" || PARTIAL=yes
    done
    if [ "$PARTIAL" = yes ]; then
        die 'Executable removed; modified PATH content and its receipt were preserved. Restore the original managed block or remove it manually, then rerun --uninstall.'
    fi
    # A crash between saving a block and its filename can leave an orphan
    # snapshot. It never changed a shell file and is safe to discard now.
    for BLOCK_RECORD in "$STATE"/path-*.block; do
        [ -e "$BLOCK_RECORD" ] || continue
        rm -f "$BLOCK_RECORD"
    done
    rm -f "$STATE/format" "$STATE/prefix"
    rmdir "$STATE" || die 'Unknown files remain in the receipt directory; they were preserved.'
    say 'Uninstalled. Preferences, aliases, logs and policy data were not removed.'
    exit 0
fi

command -v curl >/dev/null 2>&1 || die 'curl is required for HTTPS downloads.'
fetch() {
    curl --proto '=https' --proto-redir '=https' --tlsv1.2 --fail --location --silent --show-error --connect-timeout 30 --max-time 300 --output "$2" "$1"
}
if [ "$VERSION" = latest ]; then
    RESOLVED=$(curl --proto '=https' --proto-redir '=https' --tlsv1.2 --fail --location --silent --show-error --connect-timeout 30 --max-time 300 --head --output /dev/null --write-out '%{url_effective}' "$REPOSITORY/releases/latest") || die 'Cannot resolve the latest GitHub release.'
    case "$RESOLVED" in "$REPOSITORY/releases/tag/"*) VERSION=${RESOLVED#"$REPOSITORY/releases/tag/"} ;; *) die "Latest did not resolve to a concrete release: $RESOLVED." ;; esac
    valid_tag "$VERSION" || die "Unsupported resolved release tag: $VERSION."
fi
ASSET=miccamwatch-$PLATFORM-$ASSET_ARCH.tar.gz
BASE=$REPOSITORY/releases/download/$VERSION
say "Downloading $ASSET from $VERSION."
fetch "$BASE/$ASSET" "$WORK/package.tar.gz" || die 'Archive download failed; installed executable was not changed.'
fetch "$BASE/SHA256SUMS" "$WORK/SHA256SUMS" || die 'Checksum manifest download failed; installed executable was not changed.'
EXPECTED=$(awk -v asset="$ASSET" '$2==asset || $2=="*" asset { count++; hash=$1; if(NF!=2 || length(hash)!=64 || hash ~ /[^0-9a-fA-F]/) bad=1 } END { if(count!=1 || bad) exit 1; print tolower(hash) }' "$WORK/SHA256SUMS") || die "Manifest must contain exactly one valid checksum for $ASSET."
ACTUAL=$(sha_file "$WORK/package.tar.gz") || die 'Cannot checksum downloaded archive.'
[ "$ACTUAL" = "$EXPECTED" ] || die 'Archive SHA-256 mismatch; installed executable was not changed.'
# List every member, reject unknown/duplicate names and all nonregular entries.
# Extract only mcw to stdout: no attacker-controlled archive path is written.
tar -tzf "$WORK/package.tar.gz" > "$WORK/members" || die 'Invalid gzip/tar archive.'
awk '$0!="mcw" && $0!="README.md" && $0!="LICENSE" { bad=1 } { seen[$0]++ } END { exit bad || NR!=3 || seen["mcw"]!=1 || seen["README.md"]!=1 || seen["LICENSE"]!=1 }' "$WORK/members" || die 'Unsafe archive: expected exactly mcw, README.md and LICENSE once each.'
tar -tvzf "$WORK/package.tar.gz" > "$WORK/types" || die 'Cannot inspect archive member types.'
awk 'substr($0,1,1)!="-" { bad=1 } END { exit bad || NR!=3 }' "$WORK/types" || die 'Unsafe archive: symlinks, hardlinks and nonregular entries are not allowed.'
mkdir -p "$BIN" || die "Cannot create $BIN."
STAGE=$(mktemp "$BIN/.mcw-stage.XXXXXXXX") || die 'Cannot create same-directory executable staging file.'
tar -xOzf "$WORK/package.tar.gz" mcw > "$STAGE" || die 'Cannot stream the executable from the archive.'
[ -s "$STAGE" ] || die 'Archive executable is empty.'
chmod 755 "$STAGE" || die 'Cannot make staged executable runnable.'
REPORTED=$("$STAGE" --version) || die 'Staged mcw --version failed; old executable preserved. On macOS check your Gatekeeper policy; this installer does not bypass it.'
[ "$REPORTED" = "mcw ${VERSION#v}" ] || die "Executable version mismatch: expected mcw ${VERSION#v}, got $REPORTED. Old executable preserved."
NEW_HASH=$(sha_file "$STAGE") || die 'Cannot checksum staged executable.'
owned_binary
[ -z "$CURRENT_HASH" ] || refuse_running
if [ -n "$CURRENT_HASH" ]; then
    BACKUP=$(mktemp "$BIN/.mcw-backup.XXXXXXXX") || die 'Cannot stage rollback executable; old executable preserved.'
    cp -p "$TARGET" "$BACKUP" || die 'Cannot save rollback executable; old executable preserved.'
fi
# Journal both hashes before rename: an interrupted update still identifies the
# old OR new executable as owned, without claiming ownership of changed files.
{ [ -z "$CURRENT_HASH" ] || printf '%s\n' "$CURRENT_HASH"; printf '%s\n' "$NEW_HASH"; } > "$WORK/binary.sha256"
mv -f "$WORK/binary.sha256" "$STATE/binary.sha256" || die 'Cannot commit executable ownership receipt; old executable preserved.'
mv -f "$STAGE" "$TARGET" || die 'Atomic executable replacement failed; old executable preserved and recovery receipt retained.'
STAGE=
VALIDATED=yes
REPORTED=$("$TARGET" --version) || VALIDATED=no
[ "$REPORTED" = "mcw ${VERSION#v}" ] || VALIDATED=no
if [ "$VALIDATED" = no ]; then
    if [ -n "$BACKUP" ]; then
        if ! mv -f "$BACKUP" "$TARGET"; then
            SAVED_BACKUP=$BACKUP
            BACKUP=
            die "Final validation failed and rollback could not be committed. Old executable preserved at $SAVED_BACKUP; restore it manually. Recovery receipt retained."
        fi
        BACKUP=
    else
        rm -f "$TARGET" || die 'Final validation failed and the new executable could not be removed; receipt retained.'
        rm -f "$STATE/binary.sha256"
    fi
    die 'Final installed-version validation failed; previous executable restored (or initial failed install removed).'
fi
[ -z "$BACKUP" ] || rm -f "$BACKUP"
BACKUP=
printf '%s\n' "$NEW_HASH" > "$WORK/binary.sha256"
mv -f "$WORK/binary.sha256" "$STATE/binary.sha256" || die 'Installed executable is valid, but receipt finalization failed; recovery receipt retained.'
printf '%s\n' "$VERSION" > "$WORK/version"
mv -f "$WORK/version" "$STATE/version" || die 'Installed executable is valid, but version receipt could not be saved.'
say "Installed $REPORTED at $TARGET."

# Quote literal paths, not expressions evaluated by the user's shell.
posix_quote() { printf "'"; printf '%s' "$1" | sed "s/'/'\\\\''/g"; printf "'"; }
fish_quote() { printf "'"; printf '%s' "$1" | sed "s/\\\\/\\\\\\\\/g; s/'/\\\\'/g"; printf "'"; }
PREFIX_ID=$(printf '%s' "$PREFIX" | sha_text)
START_MARKER="# >>> MicCamWatch PATH $PREFIX_ID >>>"
END_MARKER="# <<< MicCamWatch PATH $PREFIX_ID <<<"
SHELL_NAME=${SHELL:-}
SHELL_NAME=${SHELL_NAME##*/}
SHELL_NAME=${SHELL_NAME:-unknown}
case "$SHELL_NAME" in
    fish) QUOTED_BIN=$(fish_quote "$BIN"); MANUAL="set -gx PATH $QUOTED_BIN \$PATH" ;;
    *) QUOTED_BIN=$(posix_quote "$BIN"); MANUAL="export PATH=$QUOTED_BIN:\"\$PATH\"" ;;
esac
IN_PATH=no
OLD_IFS=$IFS; IFS=:
# Disable filename expansion when splitting PATH components.
set -f
for COMPONENT in ${PATH:-}; do
    if [ "$COMPONENT" = "$BIN" ]; then IN_PATH=yes; break; fi
    case "$COMPONENT" in
        /*)
            if [ -d "$COMPONENT" ] && [ "$(CDPATH= cd "$COMPONENT" 2>/dev/null && pwd -P)" = "$BIN" ]; then
                IN_PATH=yes
                break
            fi ;;
    esac
done
set +f
IFS=$OLD_IFS
if [ "$IN_PATH" = yes ]; then
    say 'The installation directory is already on PATH; no shell files were changed.'
    exit 0
fi
if [ "$PATH_MODE" = ask ]; then
    # `tty` tests the controlling terminal, not stdin occupied by curl | sh.
    if ( : < /dev/tty ) 2>/dev/null && [ -t 1 ]; then
        printf 'Add %s to your shell PATH? [y/N] ' "$BIN" > /dev/tty
        ANSWER=
        IFS= read -r ANSWER < /dev/tty || :
        case "$ANSWER" in y|Y|yes|YES|Yes) PATH_MODE=add ;; *) PATH_MODE=never ;; esac
    else
        PATH_MODE=never
    fi
fi
if [ "$PATH_MODE" != add ]; then
    say "PATH was not changed. Run $TARGET directly, or add this to your shell configuration:"
    say "$MANUAL"
    exit 0
fi

make_block() {
    printf '%s\n' "$START_MARKER"
    if [ "$SHELL_NAME" = fish ]; then
        printf 'if not contains -- %s $PATH\n    set -gx PATH %s $PATH\nend\n' "$QUOTED_BIN" "$QUOTED_BIN"
    else
        printf 'case ":${PATH-}:" in\n    *:%s:*) ;;\n    *) export PATH=%s:"${PATH-}" ;;\nesac\n' "$QUOTED_BIN" "$QUOTED_BIN"
    fi
    printf '%s\n' "$END_MARKER"
}
make_block > "$WORK/path.block"
add_block() {
    CONFIG=$1
    valid_path "$CONFIG" || die 'Shell configuration path must be absolute and contain no colon/control characters.'
    if [ -e "$CONFIG" ] || [ -L "$CONFIG" ]; then
        regular_file "$CONFIG" || die "Executable installed, but refusing nonregular/symlink/hardlinked shell configuration: $CONFIG."
    fi
    FOUND=
    for PATH_RECORD in "$STATE"/path-*.path; do
        [ -e "$PATH_RECORD" ] || continue
        if [ "$(cat "$PATH_RECORD")" = "$CONFIG" ]; then
            FOUND=${PATH_RECORD##*/}; FOUND=${FOUND%.path}; break
        fi
    done
    if [ -n "$FOUND" ] && [ -f "$CONFIG" ] && block_matches "$STATE/$FOUND.block" "$CONFIG"; then
        say "Managed PATH block already present in $CONFIG."
        return
    fi
    if [ -f "$CONFIG" ] && grep -F "$PREFIX_ID" "$CONFIG" >/dev/null; then
        die "Executable installed, but a modified/ambiguous managed PATH block exists in $CONFIG; it was preserved."
    fi
    if [ -z "$FOUND" ]; then
        INDEX=1
        while [ -e "$STATE/path-$INDEX.path" ] || [ -e "$STATE/path-$INDEX.block" ] || [ -L "$STATE/path-$INDEX.path" ] || [ -L "$STATE/path-$INDEX.block" ]; do INDEX=$((INDEX + 1)); done
        FOUND=path-$INDEX
    fi
    mkdir -p "$(dirname "$CONFIG")" || die 'Executable installed, but shell configuration directory cannot be created.'
    CONFIG_WORK=$(mktemp -d "$(dirname "$CONFIG")/.mcw-path.XXXXXXXX") || die 'Cannot stage PATH configuration.'
    EXISTED=no
    if [ -f "$CONFIG" ]; then
        EXISTED=yes
        cp -p "$CONFIG" "$CONFIG_WORK/original" || die 'Cannot snapshot shell configuration.'
        cp -p "$CONFIG" "$CONFIG_WORK/new" || die 'Cannot stage shell configuration.'
    else
        : > "$CONFIG_WORK/new"
        chmod 644 "$CONFIG_WORK/new"
    fi
    printf '\n' >> "$CONFIG_WORK/new"
    cat "$WORK/path.block" >> "$CONFIG_WORK/new"
    # Record exact ownership before changing config. If interrupted, uninstall
    # preserves a missing/changed block instead of deleting unrelated content.
    cp "$WORK/path.block" "$WORK/receipt.block"
    printf '%s\n' "$CONFIG" > "$WORK/receipt.path"
    mv -f "$WORK/receipt.block" "$STATE/$FOUND.block" || die 'Cannot save PATH block receipt.'
    mv -f "$WORK/receipt.path" "$STATE/$FOUND.path" || die 'Cannot save PATH filename receipt.'
    if [ "$EXISTED" = yes ]; then
        regular_file "$CONFIG" && cmp -s "$CONFIG" "$CONFIG_WORK/original" || die 'Shell configuration changed concurrently; no PATH block was written.'
    else
        [ ! -e "$CONFIG" ] && [ ! -L "$CONFIG" ] || die 'Shell configuration appeared concurrently; no PATH block was written.'
    fi
    mv -f "$CONFIG_WORK/new" "$CONFIG" || die 'Executable installed, but PATH configuration could not be replaced.'
    rm -rf "$CONFIG_WORK"; CONFIG_WORK=
    say "Added managed PATH block to $CONFIG."
}
case "$SHELL_NAME" in
    zsh)
        ZSH_HOME=${ZDOTDIR:-$HOME}
        valid_path "$ZSH_HOME" || die 'ZDOTDIR must be absolute and contain no colon/control characters.'
        add_block "$ZSH_HOME/.zshrc"
        say "Open a new interactive zsh terminal, or run: $MANUAL"
        say 'The block is in .zshrc (interactive terminals), not noninteractive .zshenv.' ;;
    bash)
        add_block "$HOME/.bashrc"
        LOGIN_CONFIG=$HOME/.bash_profile
        if [ -e "$HOME/.bash_profile" ] || [ -L "$HOME/.bash_profile" ]; then :
        elif [ -e "$HOME/.bash_login" ] || [ -L "$HOME/.bash_login" ]; then LOGIN_CONFIG=$HOME/.bash_login
        elif [ -e "$HOME/.profile" ] || [ -L "$HOME/.profile" ]; then LOGIN_CONFIG=$HOME/.profile
        fi
        add_block "$LOGIN_CONFIG"
        say "Open a new bash terminal, or run: $MANUAL"
        say 'Managed blocks cover both interactive non-login .bashrc and the active login profile.' ;;
    fish)
        FISH_HOME=${XDG_CONFIG_HOME:-$HOME/.config}
        valid_path "$FISH_HOME" || die 'XDG_CONFIG_HOME must be absolute and contain no colon/control characters.'
        add_block "$FISH_HOME/fish/config.fish"
        say "Open a new fish terminal, or run: $MANUAL" ;;
    *)
        warn "Unknown shell ($SHELL_NAME): no shell configuration was changed."
        say "Add $BIN to PATH using your shell's syntax. POSIX shell example:"
        say "$MANUAL"
        exit 1 ;;
esac
say 'The installer cannot modify this terminal process; its PATH changes apply to new shell sessions.'

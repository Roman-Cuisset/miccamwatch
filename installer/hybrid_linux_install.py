#!/usr/bin/env python3
"""Install and exchange complete verified Linux bundles under an owned prefix."""
import argparse
from contextlib import contextmanager
import ctypes
import fcntl
import json
import os
from pathlib import Path
import platform
import re
import stat
import tarfile
import tempfile
import uuid

from hybrid_payload import MANIFEST, check, digest, expected_files, verify

MARKER = ".miccamwatch-managed-v1"
MARKER_DATA = b"MicCamWatch Linux bundle 1\n"
LEASE = ".bundle.lock"
ACTIVE = "bundle"
BACKUPS = "rollback"
MAX_PACKAGE = 128 * 1024 * 1024
AT_FDCWD = -100
RENAME_EXCHANGE = 2
EXECUTABLES = {"mcw", "mcw-camera-helper", "install-camera-helper.sh",
               "desktop/miccamwatch_desktop"}


def safe_directory(path, owner_required=False):
    info = path.lstat()
    check(stat.S_ISDIR(info.st_mode) and info.st_mode & 0o022 == 0,
          f"unsafe directory: {path}")
    owners = {os.geteuid()} if owner_required else {0, os.geteuid()}
    check(info.st_uid in owners, f"foreign directory owner: {path}")


def safe_file(descriptor):
    info = os.fstat(descriptor)
    check(stat.S_ISREG(info.st_mode) and info.st_uid == os.geteuid()
          and info.st_mode & 0o022 == 0 and info.st_nlink == 1,
          "invalid managed installation record")


def durable_directory(path):
    descriptor = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


@contextmanager
def owned_root(root, *, create=False):
    check(platform.system() == "Linux" and os.geteuid() != 0,
          "Linux non-root user required")
    check(root.is_absolute() and root == root.resolve(), "use a normalized absolute installation root")
    for parent in reversed(root.parents):
        safe_directory(parent)
    new = not os.path.lexists(root)
    if new:
        check(create, "managed root does not exist")
        root.mkdir(mode=0o700)
    safe_directory(root, owner_required=True)
    if new:
        # Exclusive marker initialization preserves other installations. The
        # parent directory is never changed except to create this new prefix.
        with (root / MARKER).open("xb") as marker:
            marker.write(MARKER_DATA)
            marker.flush()
            os.fsync(marker.fileno())
        durable_directory(root)
        durable_directory(root.parent)
    descriptor = os.open(root / MARKER, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(descriptor, "rb") as marker:
        safe_file(marker.fileno())
        check(marker.read(len(MARKER_DATA) + 1) == MARKER_DATA, "unknown installation marker")
    descriptor = os.open(root / LEASE, os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW | os.O_NONBLOCK, 0o600)
    try:
        safe_file(descriptor)
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise ValueError("MicCamWatch is in use. Quit the owned tray before changing the bundle.") from error
        history = root / BACKUPS
        history.mkdir(mode=0o700, exist_ok=True)
        safe_directory(history, owner_required=True)
        yield
    finally:
        os.close(descriptor)


def unpack(archive, checksum, stage, target):
    check(re.fullmatch(r"[a-f0-9]{64}", checksum), "a verified SHA-256 is required")
    allowed = expected_files(target) | {MANIFEST}
    total = 0
    descriptor = os.open(archive, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(descriptor, "rb") as incoming, tempfile.TemporaryFile(dir=stage.parent) as source:
        info = os.fstat(incoming.fileno())
        check(stat.S_ISREG(info.st_mode) and info.st_size <= MAX_PACKAGE,
              "invalid package input or size")
        size = 0
        while block := incoming.read(1024 * 1024):
            size += len(block)
            check(size <= MAX_PACKAGE, "compressed archive exceeds payload budget")
            source.write(block)
        source.seek(0)
        check(digest(source) == checksum, "archive SHA-256 mismatch")
        source.seek(0)
        with tarfile.open(fileobj=source, mode="r|gz") as stream:
            seen = set()
            for member in stream:
                name = member.name
                check(name in allowed and name not in seen and member.isreg() and member.size > 0,
                      f"unexpected archive member: {name}")
                total += member.size
                check(total <= MAX_PACKAGE, "decompressed archive exceeds payload budget")
                mode = 0o755 if name in EXECUTABLES else 0o644
                check(member.mode == mode, f"wrong installed mode for {name}")
                destination = stage / name
                destination.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
                with destination.open("xb") as file, stream.extractfile(member) as payload:
                    while block := payload.read(1024 * 1024):
                        file.write(block)
                    file.flush()
                    os.fchmod(file.fileno(), mode)
                    os.fsync(file.fileno())
                seen.add(name)
            check(seen == allowed, f"package missing {sorted(allowed - seen)}")
    for directory, _, _ in os.walk(stage, topdown=False):
        durable_directory(directory)
    return verify(stage, target)


def exchange(active, other):
    # RENAME_EXCHANGE is one namespace operation, so interruption can leave
    # either complete bundle active. Never fall back to file-by-file copying.
    libc = ctypes.CDLL(None, use_errno=True)
    swap = libc.renameat2
    swap.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
    swap.restype = ctypes.c_int
    if swap(AT_FDCWD, os.fsencode(active), AT_FDCWD, os.fsencode(other), RENAME_EXCHANGE):
        error = ctypes.get_errno()
        raise OSError(error, os.strerror(error))
    durable_directory(active.parent)
    durable_directory(other.parent)


def install(archive, checksum, root, target):
    with owned_root(root, create=True):
        active = root / ACTIVE
        previous = verify(active, target, current=False) if os.path.lexists(active) else None
        with tempfile.TemporaryDirectory(prefix=".verify-", dir=root) as temporary:
            verified = Path(temporary) / ACTIVE
            verified.mkdir(mode=0o700)
            candidate = unpack(archive, checksum, verified, target)
            if candidate == previous:
                return {"status": "already-installed", "revision": candidate["revision"]}
            if previous is None:
                os.rename(verified, active)
                durable_directory(root)
                return {"status": "installed", "revision": candidate["revision"]}
            check(tuple(map(int, candidate["version"].split("."))) >=
                  tuple(map(int, previous["version"].split("."))),
                  "older version requires an explicit rollback")
            slot = uuid.uuid4().hex
            rollback = root / BACKUPS / slot
            os.rename(verified, rollback)
            durable_directory(rollback.parent)
            exchange(active, rollback)
            check(verify(active, target, current=False) == candidate and verify(rollback, target, current=False) == previous,
                  "unexpected post-exchange state; preserve both complete trees")
            return {"status": "upgraded", "revision": candidate["revision"], "rollback_slot": slot,
                    "previous_revision": previous["revision"]}


def rollback(root, slot, target):
    check(re.fullmatch(r"[a-f0-9]{32}", slot), "exact managed rollback slot required")
    with owned_root(root):
        active = root / ACTIVE
        backup = root / BACKUPS / slot
        current, previous = verify(active, target, current=False), verify(backup, target, current=False)
        exchange(active, backup)
        check(verify(active, target, current=False) == previous and verify(backup, target, current=False) == current,
              "unexpected post-exchange state; preserve both complete trees")
        return {"status": "rolled-back", "revision": previous["revision"], "rollback_slot": slot}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True, help="Absolute installation prefix")
    commands = parser.add_subparsers(dest="action", required=True)
    install_args = commands.add_parser("install")
    install_args.add_argument("archive", type=Path)
    install_args.add_argument("--sha256", required=True)
    commands.add_parser("rollback").add_argument("slot")
    args = parser.parse_args()
    target = f"linux-{platform.machine()}"
    try:
        if args.action == "install":
            result = install(args.archive, args.sha256, args.root, target)
        else:
            result = rollback(args.root, args.slot, target)
        print(json.dumps(result))
    except (OSError, ValueError, KeyError, tarfile.TarError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    main()

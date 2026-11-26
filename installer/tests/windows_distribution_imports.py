#!/usr/bin/env python3
"""Audit the PE import tables of the Windows executables we distribute.

This is an import-table gate, not proof of execution on every clean Windows
edition: optional OS features, runtime LoadLibrary calls and exported-symbol
availability need separate runtime qualification.

Both ordinary and delay-loaded dependencies are checked against a reviewed
Windows baseline. Merely finding a DLL in this machine's System32 is NOT proof
that it ships with Windows. Unknown DLLs and redistributable CRT families fail.
The optional Flutter bundle's runner and engine DLL are also audited as x64 PE.
The separate Dart AOT payload must be present; it is ELF, not a PE import table.

Usage:
  python -B installer/tests/windows_distribution_imports.py target/release/mcw.exe target/release/mcw-tray.exe
  ... --zip miccamwatch-windows-x86_64.zip [--msi miccamwatch-windows-x86_64.msi]
  ... --flutter-bundle desktop/flutter/build/windows/x64/runner/Release

Exit code 0 only when every analysed executable is clean.
"""

import argparse
import hashlib
from pathlib import Path, PurePosixPath
import struct
import subprocess
import sys
import tempfile
import zipfile

# Deliberately exact, reviewed imports used by our supported Windows builds.
# Adding an import requires reviewing the minimum OS / optional-feature boundary,
# not testing whether a developer has installed that DLL locally.
SYSTEM_DLLS = frozenset(name + '.dll' for name in (
    'advapi32 bcrypt bcryptprimitives cfgmgr32 combase crypt32 d3d9 dbghelp '
    'dwmapi dxgi gdi32 imm32 iphlpapi kernel32 mf mfplat mfsensorgroup msvcrt ntdll '
    'ole32 oleacc oleaut32 opengl32 propsys psapi rpcrt4 setupapi shell32 '
    'shlwapi uiautomationcore user32 uxtheme winmm wintrust ws2_32'
).split())
API_SETS = frozenset(name + '.dll' for name in (
    'api-ms-win-core-path-l1-1-0 api-ms-win-core-synch-l1-2-0 '
    'api-ms-win-core-winrt-l1-1-0 api-ms-win-core-winrt-error-l1-1-0'
).split())
CRT_PREFIXES = ('vcruntime', 'msvcp', 'msvcr', 'concrt', 'vccorlib', 'mfc', 'ucrtbased')
RUST_EXECUTABLES = frozenset(('mcw.exe', 'mcw-tray.exe'))
FLUTTER_PAYLOAD = ('miccamwatch_desktop.exe', 'flutter_windows.dll', 'data/app.so')
PE32_PLUS = 0x20B


class UnreadablePE(Exception):
    pass


def _unpack(fmt, data, offset):
    if offset < 0 or offset + struct.calcsize(fmt) > len(data):
        raise UnreadablePE(f'truncated PE structure at 0x{offset:x}')
    return struct.unpack_from(fmt, data, offset)[0]


class PEImage:
    """Bounded PE reader: inspect dependency names, not unused thunk symbols."""

    def __init__(self, path):
        self.path = Path(path)
        if not 64 <= self.path.stat().st_size <= 512 * 1024 * 1024:
            raise UnreadablePE('PE size outside audit bounds')
        self.data = self.path.read_bytes()
        if self.data[:2] != b'MZ':
            raise UnreadablePE('missing DOS header')
        pe_offset = _unpack('<I', self.data, 0x3C)
        if self.data[pe_offset:pe_offset + 4] != b'PE\0\0':
            raise UnreadablePE('missing PE signature')
        coff = pe_offset + 4
        self.machine = _unpack('<H', self.data, coff)
        section_count = _unpack('<H', self.data, coff + 2)
        optional_size = _unpack('<H', self.data, coff + 16)
        optional = coff + 20
        self.magic = _unpack('<H', self.data, optional)
        if self.magic not in (0x10B, PE32_PLUS) or not 1 <= section_count <= 96:
            raise UnreadablePE('unsupported optional header or section count')
        directory_offset = 112 if self.magic == PE32_PLUS else 96
        directory_count = _unpack('<I', self.data, optional + directory_offset - 4)
        if (optional_size < directory_offset + min(directory_count, 16) * 8
                or optional + optional_size + 40 * section_count > len(self.data)):
            raise UnreadablePE('truncated optional header or section table')
        self.image_base = _unpack('<Q' if self.magic == PE32_PLUS else '<I',
                                  self.data, optional + (24 if self.magic == PE32_PLUS else 28))
        self.header_size = _unpack('<I', self.data, optional + 60)
        if not 0 < self.header_size <= len(self.data):
            raise UnreadablePE('invalid header extent')
        self.directories = [
            (_unpack('<I', self.data, optional + directory_offset + i * 8),
             _unpack('<I', self.data, optional + directory_offset + i * 8 + 4))
            for i in range(min(directory_count, 16))
        ]
        self.sections = []
        for i in range(section_count):
            header = optional + optional_size + i * 40
            address = _unpack('<I', self.data, header + 12)
            size = _unpack('<I', self.data, header + 16)
            pointer = _unpack('<I', self.data, header + 20)
            if pointer + size > len(self.data):
                raise UnreadablePE('truncated section')
            self.sections.append((address, size, pointer))

    @property
    def architecture(self):
        return {0x14C: 'x86', 0x8664: 'x64', 0xAA64: 'arm64'}.get(self.machine, f'0x{self.machine:04x}')

    def span(self, rva, size):
        matches = [(rva, self.header_size)] if 0 <= rva < self.header_size else []
        matches += [(pointer + rva - address, pointer + raw_size)
                    for address, raw_size, pointer in self.sections
                    if address <= rva < address + raw_size]
        if len(matches) != 1 or size < 0 or matches[0][0] + size > matches[0][1]:
            raise UnreadablePE(f'unbacked, overlapping or truncated RVA 0x{rva:x}')
        return matches[0]

    def c_string(self, rva):
        start, end = self.span(rva, 1)
        end = self.data.find(b'\0', start, min(end, start + 261))
        if end < 0:
            raise UnreadablePE('unterminated or oversized DLL name')
        try:
            name = self.data[start:end].decode('ascii')
        except UnicodeDecodeError as error:
            raise UnreadablePE('non-ASCII DLL name') from error
        if not name or any(c in name for c in '\\/:') or any(ord(c) < 32 for c in name):
            raise UnreadablePE('invalid dependency name')
        return name

    def dependencies(self, directory, stride):
        if directory >= len(self.directories):
            return
        rva, size = self.directories[directory]
        if rva == size == 0:
            return
        if not rva or size < stride:
            raise UnreadablePE('invalid dependency directory')
        start, _ = self.span(rva, size)
        for offset in range(0, min(size // stride, 4096) * stride, stride):
            words = struct.unpack_from('<' + 'I' * (stride // 4), self.data, start + offset)
            if not any(words):
                return
            if directory == 1:
                name = words[3]
            else:
                if words[0] not in (0, 1):
                    raise UnreadablePE('unsupported delay-import attributes')
                name = words[1] - (self.image_base if words[0] == 0 else 0)
            yield self.c_string(name)
        raise UnreadablePE('unterminated or excessive dependency descriptors')

    def imports(self):
        return self.dependencies(1, 20)

    def delay_imports(self):
        return self.dependencies(13, 32)


def classify(dll):
    name = dll.lower()
    # The legacy inbox msvcrt is an OS component, not a VC redistributable.
    if name != 'msvcrt.dll' and (name.startswith(CRT_PREFIXES) or name == 'ucrtbase.dll'
                                or name.startswith('api-ms-win-crt-')):
        return 'dynamic-crt', 'unexpected dynamic CRT; rebuild this component with a static runtime'
    if name in SYSTEM_DLLS or name in API_SETS:
        return 'os', 'reviewed Windows import; optional features still need runtime validation'
    return 'unknown', 'unreviewed dependency; do not infer OS ownership from System32'


def audit(path, flutter_bundle=None):
    image = PEImage(path)
    rows, failures = [], []
    for kind, entries in (('import', image.imports()), ('delay-import', image.delay_imports())):
        for dll in entries:
            verdict, reason = classify(dll)
            if (dll.lower() == 'flutter_windows.dll' and flutter_bundle is not None
                    and image.path.parent.resolve() == flutter_bundle.resolve()
                    and (flutter_bundle / 'flutter_windows.dll').is_file()):
                verdict, reason = 'bundled', 'declared app-local Flutter engine, audited in the same invocation'
            rows.append(dict(kind=kind, dll=dll, verdict=verdict, reason=reason))
            if verdict not in ('os', 'bundled'):
                failures.append(f'{kind}: {dll}: {reason}')
    if image.architecture != 'x64':
        failures.append('distributed PE is not x64: ' + image.architecture)
    return image, rows, failures


def require_rust_payload(paths, originals):
    names = [p.name.lower() for p in paths]
    if len(names) != 2 or set(names) != RUST_EXECUTABLES:
        raise UnreadablePE('MSI/ZIP must contain exactly one mcw.exe and one mcw-tray.exe')
    for path in paths:
        if hashlib.sha256(path.read_bytes()).digest() != hashlib.sha256(originals[path.name.lower()].read_bytes()).digest():
            raise UnreadablePE('packaged executable differs from audited build: ' + path.name)
    return paths


def collect_from_zip(path, workdir, originals):
    extracted = []
    with zipfile.ZipFile(path) as archive:
        for member in archive.infolist():
            if member.filename.lower().endswith('.exe'):
                relative = PurePosixPath(member.filename)
                if any(c in member.filename for c in '\\:') or relative.is_absolute() or '..' in relative.parts:
                    raise UnreadablePE('unsafe ZIP executable path')
                destination = workdir / 'zip' / relative
                if destination.exists():
                    raise UnreadablePE('duplicate ZIP executable')
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes(archive.read(member))
                extracted.append(destination)
    return require_rust_payload(extracted, originals)


def collect_from_msi(path, workdir, originals):
    destination = workdir / 'msi'
    destination.mkdir()
    result = subprocess.run(
        ['msiexec', '/a', str(path.resolve(strict=True)), '/qn', f'TARGETDIR={destination.resolve()}'],
        capture_output=True, text=True, errors='replace', timeout=120,
    )
    if result.returncode != 0:
        raise UnreadablePE(f'MSI administrative extraction failed ({result.returncode}): {result.stderr.strip()}')
    return require_rust_payload(sorted(p for p in destination.rglob('*') if p.suffix.lower() == '.exe'), originals)


def main():
    # Windows runners default to a legacy code page on redirected output.
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, 'reconfigure'):
            stream.reconfigure(encoding='utf-8', errors='replace')
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('binaries', nargs='+', type=Path, help='executables actually shipped')
    parser.add_argument('--zip', type=Path, help='also audit the executables inside this release archive')
    parser.add_argument('--msi', type=Path, help='also audit the executables inside this MSI')
    parser.add_argument('--flutter-bundle', type=Path, help='also audit every native PE in this Windows Flutter bundle')
    args = parser.parse_args()

    targets = list(args.binaries)
    originals = {path.name.lower(): path for path in targets}
    try:
        if (args.zip or args.msi) and (len(targets) != 2 or set(originals) != RUST_EXECUTABLES):
            raise UnreadablePE('package audit requires the two original Rust executables')
        if args.flutter_bundle:
            bundle = args.flutter_bundle.resolve(strict=True)
            for name in FLUTTER_PAYLOAD:
                path = bundle / name
                if not path.is_file() or not path.resolve().is_relative_to(bundle):
                    raise UnreadablePE('Flutter payload missing or escaping bundle: ' + name)
            targets += sorted(path for path in bundle.rglob('*') if path.is_file()
                              and path.suffix.lower() in ('.exe', '.dll'))
            if any(not path.resolve().is_relative_to(bundle) for path in targets[len(args.binaries):]):
                raise UnreadablePE('native payload escapes Flutter bundle')
        with tempfile.TemporaryDirectory(prefix='mcw-import-audit-') as work:
            workdir = Path(work)
            if args.zip:
                targets += collect_from_zip(args.zip, workdir, originals)
            if args.msi:
                targets += collect_from_msi(args.msi, workdir, originals)
            failed = False
            for target in targets:
                image, rows, failures = audit(target, args.flutter_bundle)
                print(f'\n{target}  ({image.architecture}, {len(rows)} imported DLLs)')
                for row in rows:
                    marker = 'ok  ' if row['verdict'] in ('os', 'bundled') else 'FAIL'
                    print(f"  {marker} {row['kind']:<12} {row['dll']:<40} {row['reason']}")
                for failure in failures:
                    print('  !! ' + failure)
                    failed = True
            if failed:
                print('\nWindows import audit FAILED. Inspect each component, Cargo RUSTFLAGS / static CRT, '
                      'CMake runtime settings and the explicit OS/payload contract. '
                      'Do not install a runtime on CI to hide this failure.', file=sys.stderr)
                return 1
            print(f'\n{len(targets)} PE images checked: no unexpected dynamic CRT or undeclared dependency.')
            print('Import-table validation only; no clean-OS execution or optional-feature guarantee.')
            return 0
    except (OSError, UnreadablePE, zipfile.BadZipFile, subprocess.TimeoutExpired) as error:
        print('Windows import audit FAILED: ' + str(error), file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())

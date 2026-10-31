#!/usr/bin/env python3
"""Audit the PE import tables of the Windows executables we distribute.

The shipped executables must not depend on the Visual C++ Redistributable.
winget validation and other clean Windows images do not install it, so a dynamic
VCRUNTIME dependency aborts the process in the loader with 0xC0000135
STATUS_DLL_NOT_FOUND before any application code runs.

This inspects the real binaries rather than the build configuration, so a
regression cannot hide behind a changed .cargo/config.toml or workflow flag.

Rules applied to every static and delay-loaded import:
  * dynamic CRT / C++ runtime DLLs (VCRUNTIME*, MSVCP*, CONCRT*, VCCORLIB*, MFC*)
    always fail: the executables must link the CRT statically;
  * API sets (api-ms-win-*, ext-ms-win-*) are Windows components;
  * any other DLL must exist in %SystemRoot%\\System32 on the auditing machine.
    Anything else is an unknown non-system dependency: packaging would have to
    ship it explicitly, so it fails with an actionable message.

Usage:
  python -B installer/tests/windows_distribution_imports.py target/release/mcw.exe target/release/mcw-tray.exe
  ... --zip miccamwatch-windows-x86_64.zip [--msi miccamwatch-windows-x86_64.msi]

Exit code 0 only when every analysed executable is clean.
"""

import argparse
import os
from pathlib import Path
import re
import struct
import subprocess
import sys
import tempfile
import zipfile

CRT_DEPENDENCY = re.compile(
    r'^(?:vcruntime\d*(?:_\d+)?|msvcp\d+(?:_.+)?|concrt\d+|vccorlib\d+|mfc\d+u?)\.dll$',
    re.IGNORECASE,
)
API_SET = re.compile(r'^(?:api-ms-win|ext-ms-win)-.+\.dll$', re.IGNORECASE)
PE32_PLUS = 0x20B


class UnreadablePE(Exception):
    pass


def _unpack(fmt, data, offset):
    if offset < 0 or offset + struct.calcsize(fmt) > len(data):
        raise UnreadablePE(f'truncated PE structure at 0x{offset:x}')
    return struct.unpack_from(fmt, data, offset)[0]


class PEImage:
    """Minimal PE32/PE32+ reader for import and delay-import descriptors."""

    def __init__(self, path):
        self.path = Path(path)
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
        if self.magic not in (0x10B, PE32_PLUS):
            raise UnreadablePE(f'unsupported optional header magic 0x{self.magic:x}')
        self.image_base = (
            _unpack('<Q', self.data, optional + 24)
            if self.magic == PE32_PLUS
            else _unpack('<I', self.data, optional + 28)
        )
        directories = optional + (112 if self.magic == PE32_PLUS else 96)
        directory_count = _unpack('<I', self.data, directories - 4)
        self.directories = []
        for index in range(min(directory_count, 16)):
            rva, size = struct.unpack_from('<II', self.data, directories + index * 8)
            self.directories.append((rva, size))
        self.sections = []
        table = optional + optional_size
        for index in range(section_count):
            header = table + index * 40
            virtual_size = _unpack('<I', self.data, header + 8)
            virtual_address = _unpack('<I', self.data, header + 12)
            raw_size = _unpack('<I', self.data, header + 16)
            raw_pointer = _unpack('<I', self.data, header + 20)
            self.sections.append((virtual_address, max(virtual_size, raw_size), raw_pointer, raw_size))

    @property
    def architecture(self):
        return {0x14C: 'x86', 0x8664: 'x64', 0xAA64: 'arm64'}.get(self.machine, f'0x{self.machine:04x}')

    def offset(self, rva):
        for virtual_address, size, raw_pointer, raw_size in self.sections:
            if virtual_address <= rva < virtual_address + size:
                delta = rva - virtual_address
                if delta >= raw_size:
                    raise UnreadablePE(f'RVA 0x{rva:x} has no file backing')
                return raw_pointer + delta
        raise UnreadablePE(f'RVA 0x{rva:x} is outside every section')

    def c_string(self, rva):
        start = self.offset(rva)
        end = self.data.find(b'\0', start)
        if end < 0:
            raise UnreadablePE(f'unterminated string at 0x{start:x}')
        return self.data[start:end].decode('ascii', 'replace')

    def _thunk_names(self, rva, count_hint=None):
        stride = 8 if self.magic == PE32_PLUS else 4
        flag = 1 << (63 if self.magic == PE32_PLUS else 31)
        names = []
        cursor = self.offset(rva)
        while True:
            value = _unpack('<Q' if stride == 8 else '<I', self.data, cursor)
            if value == 0:
                break
            if value & flag:
                names.append(f'ordinal {value & 0xFFFF}')
            else:
                # IMAGE_IMPORT_BY_NAME: 2-byte hint followed by the name.
                names.append(self.c_string((value & 0x7FFFFFFF) + 2))
            cursor += stride
            if len(names) > 65536:
                raise UnreadablePE('unreasonable thunk count')
        return names

    def imports(self):
        """Yield (dll, [symbols]) for static imports."""
        if len(self.directories) < 2 or not self.directories[1][0]:
            return
        cursor = self.offset(self.directories[1][0])
        while True:
            original, _stamp, _chain, name_rva, first_thunk = struct.unpack_from('<IIIII', self.data, cursor)
            if name_rva == 0:
                break
            dll = self.c_string(name_rva)
            table_rva = original or first_thunk
            yield dll, self._thunk_names(table_rva) if table_rva else []
            cursor += 20

    def delay_imports(self):
        """Yield (dll, [symbols]) for delay-loaded imports, when present."""
        if len(self.directories) < 14 or not self.directories[13][0]:
            return
        cursor = self.offset(self.directories[13][0])
        while True:
            attributes, name_rva, _module, _iat, int_rva, _bound, _unload, _stamp = struct.unpack_from(
                '<IIIIIIII', self.data, cursor
            )
            if name_rva == 0:
                break
            if not attributes & 1:  # legacy descriptors store virtual addresses
                name_rva -= self.image_base
                int_rva = int_rva - self.image_base if int_rva else 0
            dll = self.c_string(name_rva)
            yield dll, self._thunk_names(int_rva) if int_rva else []
            cursor += 32


def classify(dll, system32):
    if CRT_DEPENDENCY.match(dll):
        return 'dynamic-crt', 'CRT dynamique: doit être lié statiquement (+crt-static)'
    if API_SET.match(dll):
        return 'os', 'API set Windows'
    if (system32 / dll).is_file():
        return 'os', 'composant Windows'
    return 'unknown', 'dépendance non système inconnue: doit être livrée explicitement'


def audit(path, system32):
    image = PEImage(path)
    rows = []
    failures = []
    for kind, entries in (('import', image.imports()), ('delay-import', image.delay_imports())):
        for dll, symbols in entries:
            verdict, reason = classify(dll, system32)
            rows.append({'kind': kind, 'dll': dll, 'symbols': len(symbols), 'verdict': verdict, 'reason': reason})
            if verdict != 'os':
                failures.append((kind, dll, reason, symbols[:4]))
    if image.architecture != 'x64':
        failures.append(('architecture', image.architecture, 'exécutable distribué non x64', []))
    return image, rows, failures


def collect_from_zip(path, workdir):
    extracted = []
    with zipfile.ZipFile(path) as archive:
        for member in archive.namelist():
            if member.lower().endswith('.exe'):
                target = workdir / ('zip-' + Path(member).name)
                target.write_bytes(archive.read(member))
                extracted.append(target)
    return extracted


def collect_from_msi(path, workdir):
    destination = workdir / 'msi'
    destination.mkdir()
    result = subprocess.run(
        ['msiexec', '/a', str(path), '/qn', f'TARGETDIR={destination}'],
        capture_output=True, text=True, errors='replace',
    )
    if result.returncode != 0:
        raise SystemExit(f'msiexec administrative extraction failed ({result.returncode}): {result.stderr.strip()}')
    return sorted(p for p in destination.rglob('*.exe'))


def main():
    # Windows runners default to a legacy code page on redirected output.
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, 'reconfigure'):
            stream.reconfigure(encoding='utf-8', errors='replace')
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('binaries', nargs='+', type=Path, help='executables actually shipped')
    parser.add_argument('--zip', type=Path, help='also audit the executables inside this release archive')
    parser.add_argument('--msi', type=Path, help='also audit the executables inside this MSI')
    args = parser.parse_args()

    root = os.environ.get('SystemRoot') or r'C:\Windows'
    system32 = Path(root) / 'System32'
    if not system32.is_dir():
        raise SystemExit(f'cannot locate {system32}; run this audit on Windows')

    targets = list(args.binaries)
    with tempfile.TemporaryDirectory(prefix='mcw-import-audit-') as work:
        workdir = Path(work)
        if args.zip:
            targets += collect_from_zip(args.zip, workdir)
        if args.msi:
            targets += collect_from_msi(args.msi, workdir)

        failed = False
        for target in targets:
            if not target.is_file():
                raise SystemExit(f'missing executable: {target}')
            image, rows, failures = audit(target, system32)
            print(f'\n{target}  ({image.architecture}, {len(rows)} imported DLLs)')
            for row in rows:
                marker = 'ok  ' if row['verdict'] == 'os' else 'FAIL'
                print(f"  {marker} {row['kind']:<12} {row['dll']:<40} {row['symbols']:>4} fn  {row['reason']}")
            for kind, dll, reason, symbols in failures:
                failed = True
                print(f"  !! {dll}: {reason}")
                if symbols:
                    print(f"     exemples d'imports: {', '.join(symbols)}")
        if failed:
            print(
                '\nAudit des dépendances Windows échoué.\n'
                'Les exécutables distribués ne doivent pas exiger le Visual C++ Redistributable:\n'
                '  * CRT dynamique détecté -> vérifier .cargo/config.toml '
                '([target.x86_64-pc-windows-msvc] rustflags = ["-C", "target-feature=+crt-static"])\n'
                '    et reconstruire sans RUSTFLAGS contraire dans l\'environnement.\n'
                '  * dépendance inconnue -> l\'ajouter au MSI/ZIP et à la liste attendue, '
                'ou expliquer pourquoi elle appartient à Windows.',
                file=sys.stderr,
            )
            return 1
        print(f'\n{len(targets)} executable(s) audités: aucune dépendance CRT dynamique ni non système.')
        return 0


if __name__ == '__main__':
    raise SystemExit(main())

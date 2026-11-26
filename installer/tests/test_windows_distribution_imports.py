"""Distribution-visible failures: runtime imports, malformed PE and wrong payloads.

Minimal PE fixtures exercise the parser only; they are never executed or claimed
as clean-Windows qualification. The CI gate separately inspects real release PEs.
"""
import importlib.util
from pathlib import Path
import struct
import tempfile
import unittest
import zipfile

spec = importlib.util.spec_from_file_location('distribution_imports', Path(__file__).with_name('windows_distribution_imports.py'))
imports = importlib.util.module_from_spec(spec)
spec.loader.exec_module(imports)


def pe_fixture(dll, *, delay=False, legacy=False, machine=0x8664):
    data = bytearray(0x600)
    data[:2] = b'MZ'
    struct.pack_into('<I', data, 0x3c, 0x80)
    data[0x80:0x84] = b'PE\0\0'
    struct.pack_into('<HH', data, 0x84, machine, 1)
    struct.pack_into('<H', data, 0x94, 240)
    optional = 0x98
    struct.pack_into('<H', data, optional, 0x20b)
    struct.pack_into('<Q', data, optional + 24, 0x400000)
    struct.pack_into('<I', data, optional + 60, 0x200)
    struct.pack_into('<I', data, optional + 108, 16)
    struct.pack_into('<IIII', data, optional + 240 + 8, 0x400, 0x1000, 0x400, 0x200)
    directory, rva, size = (13, 0x1060, 64) if delay else (1, 0x1000, 40)
    struct.pack_into('<II', data, optional + 112 + directory * 8, rva, size)
    if delay:
        struct.pack_into('<II', data, 0x260, 0 if legacy else 1, 0x401180 if legacy else 0x1180)
    else:
        struct.pack_into('<I', data, 0x200 + 12, 0x1180)
    name = dll.encode('ascii') + b'\0'
    data[0x380:0x380 + len(name)] = name
    return data


class ImportPolicyTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def image(self, data):
        path = self.root / 'input.exe'
        path.write_bytes(data)
        return path

    def test_ordinary_and_delay_loaded_runtime_families_are_rejected(self):
        for dll in ('VCRUNTIME140.dll', 'vcruntime140_1D.dll', 'MSVCP140_ATOMIC_WAIT.dll',
                    'msvcr120.dll', 'CONCRT140.dll', 'VCCORLIB140.dll', 'mfc140ud.dll',
                    'ucrtbase.dll', 'api-ms-win-crt-runtime-l1-1-0.dll'):
            for delay in (False, True):
                with self.subTest(dll=dll, delay=delay):
                    _, rows, failures = imports.audit(self.image(pe_fixture(dll, delay=delay)))
                    self.assertEqual(rows[0]['verdict'], 'dynamic-crt')
                    self.assertEqual(len(failures), 1)

    def test_reviewed_imports_are_case_insensitive_and_delays_are_read(self):
        for delay, legacy in ((False, False), (True, False), (True, True)):
            _, rows, failures = imports.audit(self.image(pe_fixture('KeRnEl32.dll', delay=delay, legacy=legacy)))
            self.assertEqual([(r['dll'], r['verdict']) for r in rows], [('KeRnEl32.dll', 'os')])
            self.assertEqual(failures, [])

    def test_local_presence_does_not_make_an_unknown_dll_an_os_component(self):
        (self.root / 'thirdparty_crypto.dll').write_bytes(b'locally installed')
        _, rows, failures = imports.audit(self.image(pe_fixture('thirdparty_crypto.dll')))
        self.assertEqual(rows[0]['verdict'], 'unknown')
        self.assertEqual(len(failures), 1)
        self.assertEqual(imports.classify('api-ms-win-invented-feature-l1-1-0.dll')[0], 'unknown')

    def test_app_local_engine_requires_declared_bundle_and_actual_peer_file(self):
        runner = self.root / 'miccamwatch_desktop.exe'
        runner.write_bytes(pe_fixture('flutter_windows.dll'))
        self.assertEqual(len(imports.audit(runner)[2]), 1)
        self.assertEqual(len(imports.audit(runner, self.root)[2]), 1)
        (self.root / 'flutter_windows.dll').write_bytes(pe_fixture('kernel32.dll'))
        self.assertEqual(imports.audit(runner, self.root)[2], [])
        elsewhere = self.root / 'other'
        elsewhere.mkdir()
        self.assertEqual(len(imports.audit(runner, elsewhere)[2]), 1)

    def test_wrong_architecture_is_not_an_acceptable_distribution(self):
        self.assertEqual(len(imports.audit(self.image(pe_fixture('kernel32.dll', machine=0xaa64)))[2]), 1)

    def test_truncation_is_not_silently_treated_as_no_imports(self):
        data = pe_fixture('kernel32.dll')
        for length in (0, 63, 128, 270, 511, 1024):
            with self.subTest(length=length), self.assertRaises(imports.UnreadablePE):
                imports.audit(self.image(data[:length]))

    def test_unbacked_or_unterminated_descriptor_directory_is_rejected(self):
        for rva, size in ((0xffffffff, 40), (0x1000, 19), (0x1000, 20), (0, 40)):
            data = pe_fixture('kernel32.dll')
            struct.pack_into('<II', data, 0x98 + 112 + 8, rva, size)
            with self.subTest(rva=rva, size=size), self.assertRaises(imports.UnreadablePE):
                imports.audit(self.image(data))

    def test_dependency_name_cannot_escape_its_backed_section(self):
        data = pe_fixture('kernel32.dll')
        struct.pack_into('<I', data, 0x200 + 12, 0x13f8)
        data[0x5f8:] = b'no-null!'
        with self.assertRaises(imports.UnreadablePE):
            imports.audit(self.image(data))

    def test_package_requires_both_executables_and_exact_build_bytes(self):
        originals = {}
        for name in sorted(imports.RUST_EXECUTABLES):
            path = self.root / name
            path.write_bytes(name.encode('ascii'))
            originals[name] = path
        valid = [('bin/' + name, path.read_bytes()) for name, path in originals.items()]
        cases = [('valid', valid, True), ('missing', valid[:1], False),
                 ('duplicate', valid + valid[:1], False),
                 ('mismatch', [(valid[0][0], b'wrong-build'), valid[1]], False),
                 ('escape', [('../mcw.exe', b'bad'), valid[1]], False)]
        for name, members, accepted in cases:
            archive = self.root / (name + '.zip')
            with zipfile.ZipFile(archive, 'w') as z:
                for member, data in members:
                    z.writestr(member, data)
            work = self.root / name
            work.mkdir()
            with self.subTest(name=name):
                if accepted:
                    self.assertEqual({p.name for p in imports.collect_from_zip(archive, work, originals)}, imports.RUST_EXECUTABLES)
                else:
                    with self.assertRaises(imports.UnreadablePE):
                        imports.collect_from_zip(archive, work, originals)


if __name__ == '__main__':
    unittest.main()

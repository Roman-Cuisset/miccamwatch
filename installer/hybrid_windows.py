#!/usr/bin/env python3
"""Build the per-user Windows hybrid MSI and verify its complete extraction."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import sys
import tempfile
import uuid
import xml.etree.ElementTree as xml

from hybrid_payload import MANIFEST, REPO, check, expected_files, paths, sha, verify

sys.path.insert(0, str(REPO / "installer/tests"))
from windows_distribution_imports import audit

TARGET = "windows-x86_64"
NAMESPACE = "{http://wixtoolset.org/schemas/v4/wxs}"


def wix(parent, tag, **attributes):
    return xml.SubElement(parent, NAMESPACE + tag, attributes)


def make_wxs(root, output, ipc):
    xml.register_namespace("", NAMESPACE[1:-1])
    tree = xml.parse(REPO / "installer/miccamwatch.wxs")
    package = tree.find(NAMESPACE + "Package")
    # Remove the previous product inside the installer transaction, so MSI can
    # restore it if installing the candidate fails before InstallFinalize.
    package.find(NAMESPACE + "MajorUpgrade").set("Schedule", "afterInstallInitialize")
    wix(package, "Property", Id="MCW_BUNDLE_IPC", Value=str(ipc))
    folder = package.find(f".//{NAMESPACE}Directory[@Id='INSTALLFOLDER']")
    feature = package.find(NAMESPACE + "Feature")
    directories = {Path("."): folder}
    for relative in sorted((expected_files(TARGET) | {MANIFEST}) - {"mcw.exe", "mcw-tray.exe"}):
        path = Path(relative)
        for directory in reversed(path.parents):
            if directory not in directories:
                key = hashlib.sha256(directory.as_posix().encode()).hexdigest()[:24]
                directories[directory] = wix(directories[directory.parent], "Directory",
                                             Id="D_" + key, Name=directory.name)
        key = hashlib.sha256(relative.encode()).hexdigest()[:24]
        component_id = "C_" + key
        guid = uuid.uuid5(uuid.UUID("8fc9317b-4517-4e83-9f52-cf3257584bf6"), relative)
        component = wix(directories[path.parent], "Component", Id=component_id, Guid=str(guid))
        wix(component, "File", Id="F_" + key, Name=path.name, Source=str(root / path))
        wix(component, "RegistryValue", Root="HKCU", Key=r"Software\MicCamWatch\BundleComponents",
            Name=component_id, Value="1", Type="integer", KeyPath="yes")
        wix(feature, "ComponentRef", Id=component_id)
    tree.write(output, encoding="utf-8", xml_declaration=True)


def audit_pe(root):
    desktop = root / "desktop"
    aot = desktop / "data/app.so"
    with aot.open("rb") as payload:
        header = payload.read(20)
    check(len(header) == 20 and header[:6] == b"\x7fELF\x02\x01"
          and header[16:20] == b"\x03\x00\x3e\x00", "Dart AOT payload is not x64 ELF")
    for file in (root / "mcw.exe", root / "mcw-tray.exe",
                 desktop / "miccamwatch_desktop.exe", desktop / "flutter_windows.dll"):
        _, rows, failures = audit(file, desktop)
        check(not failures, f"{file.name}: {'; '.join(failures)}")
        print(f"{file.name}: x64 PE, {len(rows)} ordinary/delay imports verified")


def check_extracted(msi, bundle, manifest, staging):
    extraction = staging / "unpacked"
    extraction.mkdir()
    log = msi.with_suffix(".extract.log")
    command = ["msiexec.exe", "/a", str(msi), "/qn", f"TARGETDIR={extraction}",
               "/l*v", str(log)]
    result = subprocess.run(command, timeout=120)
    check(result.returncode == 0, f"MSI administrative extraction failed: {result.returncode}")
    candidates = list(extraction.rglob(MANIFEST))
    check(len(candidates) == 1 and candidates[0].parent.name == "MicCamWatch",
          "MSI administrative image does not contain one MicCamWatch installation")
    installed = candidates[0].parent
    check(verify(installed, TARGET, manifest["revision"]) == manifest, "MSI manifest differs")
    check(sha(installed / MANIFEST) == sha(bundle / MANIFEST), "MSI metadata bytes differ")
    for path in extraction.rglob("*"):
        if path.is_file() and not path.is_relative_to(installed):
            check(path.name == msi.name, f"unexpected administrative image file: {path.name}")
    audit_pe(installed)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    output = args.output.resolve(strict=True)
    bundle = output / "bundle"
    try:
        manifest = verify(bundle, TARGET)
        audit_pe(bundle)
        msi = output / f"miccamwatch-hybrid-{TARGET}.msi"
        check(not msi.exists(), f"existing MSI: {msi}")
        with tempfile.TemporaryDirectory(prefix="mcw-wix-", dir=output) as directory:
            staging = Path(directory)
            source = staging / "hybrid.wxs"
            make_wxs(bundle, source, manifest["ipc"])
            subprocess.run(["dotnet", "wix", "build", "-arch", "x64", str(source),
                            "-d", "Version=" + manifest["version"],
                            "-d", "Executable=" + str(bundle / "mcw.exe"),
                            "-d", "TrayExecutable=" + str(bundle / "mcw-tray.exe"),
                            "-pdbtype", "none", "-o", str(msi)], cwd=REPO, check=True)
            check_extracted(msi, bundle, manifest, staging)
        sums = output / "SHA256SUMS"
        with sums.open("a", encoding="ascii") as stream:
            stream.write(f"{sha(msi)}  {msi.name}\n")
        print(msi)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    main()

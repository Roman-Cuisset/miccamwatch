"""Record embedded runtime components in the lockfile-based candidate SBOM."""

import argparse
import json
from pathlib import Path
import re
from hybrid_payload import release_metadata


FRAMEWORK = "737ba8f885dc774b53b442759999a312c9461e4e"
ENGINE = "cf636b1dfb818dd6a3ff16e390e93b1a036969c8"


def enrich(path, revision):
    if not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise ValueError("Candidate source must be a full Git SHA")
    document = json.loads(path.read_text(encoding="utf-8"))
    if not document["spdxVersion"].startswith("SPDX-2."):
        raise ValueError("Expected SPDX 2.x JSON")
    packages = document["packages"]
    refs = [ref.get("referenceLocator", "") for package in packages
            for ref in package.get("externalRefs", [])]
    for kind in ("pkg:cargo/", "pkg:pub/"):
        if not any(ref.startswith(kind) for ref in refs):
            raise ValueError(f"Missing lockfile package catalog: {kind}")

    source = f"https://github.com/Roman-Cuisset/miccamwatch/tree/{revision}"
    package_version, _ = release_metadata()
    components = (
        ("rust", "MicCamWatch Rust monitor and CLI", package_version, source, "MIT",
         "mcw.exe on Windows and mcw on GNU/Linux and macOS"),
        ("win-tray", "MicCamWatch native Windows tray", package_version, source, "MIT",
         "mcw-tray.exe in Windows MSI and ZIP"),
        ("mac-native", "MicCamWatch native macOS launcher and notification helper", package_version,
         source, "MIT", "AppKit launcher and MicCamWatchHelper.app in the sealed macOS bundle"),
        ("linux-helper", "MicCamWatch Linux camera helper", package_version, source, "MIT",
         "mcw-camera-helper and its install script and Polkit policy"),
        ("framework", "Flutter framework", "3.47.8",
         f"https://github.com/flutter/flutter/tree/{FRAMEWORK}", "NOASSERTION",
         "Framework compiled into the desktop frontend"),
        ("engine", "Flutter desktop engine", ENGINE,
         f"https://github.com/flutter/flutter/tree/{FRAMEWORK}/engine", "NOASSERTION",
         "flutter_windows.dll, libflutter_linux_gtk.so or FlutterMacOS.framework by platform"),
        ("dart", "Dart AOT runtime", "3.13.7", "https://github.com/dart-lang/sdk/tree/3.13.7",
         "NOASSERTION", "Runtime in the Flutter engine. SDK and development tools are excluded."),
    )
    ids = {package["SPDXID"] for package in packages}
    for suffix, name, version, upstream, license_id, details in components:
        spdx_id = f"SPDXRef-MCW-Release-{suffix}"
        if spdx_id in ids:
            raise ValueError(f"Duplicate SPDX ID: {spdx_id}")
        packages.append({
            "SPDXID": spdx_id,
            "name": name,
            "versionInfo": version,
            "downloadLocation": upstream,
            "filesAnalyzed": False,
            "licenseConcluded": "NOASSERTION",
            "licenseDeclared": license_id,
            "copyrightText": "NOASSERTION",
            "comment": details,
        })
        document.setdefault("relationships", []).append({
            "spdxElementId": document["SPDXID"], "relationshipType": "DESCRIBES",
            "relatedSpdxElement": spdx_id,
        })
    document["comment"] = (
        f"Package build revision: {revision}. The source scan includes lockfile "
        "dependencies, including development dependencies. The added entries describe "
        "runtime components in the five platform packages. Source LICENSE and the Flutter "
        "third-party notices shipped with each bundle remain the licence references. "
        "This source scan does not inventory the host OS or assign transitive licences."
    )
    path.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("spdx", type=Path)
    parser.add_argument("revision")
    args = parser.parse_args()
    enrich(args.spdx, args.revision)

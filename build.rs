use std::{env, fs, path::PathBuf, process::Command};

/// Embeds the Windows application manifest so the tray renders with per-monitor DPI
/// awareness (crisp text instead of bitmap-stretched) and themed Common Controls v6 menus.
const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity type="win32" name="MicCamWatch" version="0.0.0.0" processorArchitecture="*"/>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
    </application>
  </compatibility>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
      <longPathAware xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">true</longPathAware>
    </windowsSettings>
  </application>
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
    </dependentAssembly>
  </dependency>
</assembly>
"#;

fn main() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        compile_macos_helper();
    }
    println!("cargo:rerun-if-changed=build.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
        || env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc")
    {
        return;
    }
    let version = env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".to_owned());
    let Ok(out_dir) = env::var("OUT_DIR").map(PathBuf::from) else {
        return;
    };
    // Cargo splits `rustc-link-arg` values on whitespace, so a spaced OUT_DIR would
    // truncate the manifest path and break the link.
    if out_dir.to_string_lossy().contains(char::is_whitespace) {
        println!("cargo:warning=skipping manifest embedding: build path contains whitespace");
        return;
    }
    let path = out_dir.join("mcw.manifest");
    if fs::write(&path, MANIFEST.replace("0.0.0.0", &format!("{version}.0"))).is_err() {
        return;
    }
    println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
    println!(
        "cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}",
        path.display()
    );
}

fn compile_macos_helper() {
    let sources = [
        "native/macos_capture.swift",
        "native/macos_controls.swift",
        "native/macos_process.swift",
        "native/macos_trust.swift",
        "native/macos_desktop.swift",
    ];
    for source in sources {
        println!("cargo:rerun-if-changed={source}");
    }
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo must set OUT_DIR"));
    // Swift permits top-level statements in a multi-file executable only in main.swift.
    let entrypoint = out_dir.join("main.swift");
    fs::copy(sources[0], &entrypoint).expect("cannot copy macOS helper entrypoint");
    let bundle = out_dir.join("MicCamWatchHelper.app");
    let contents = bundle.join("Contents");
    fs::create_dir_all(contents.join("MacOS")).expect("cannot create macOS helper bundle");
    let bridge = out_dir.join("macos-helper.h");
    fs::write(&bridge, "#include <libproc.h>\n#include <bsm/libbsm.h>\n")
        .expect("cannot create public libproc/libbsm Swift bridge");
    let version = env::var("CARGO_PKG_VERSION").expect("Cargo must set package version");
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>com.roman-cuisset.miccamwatch.helper</string>
<key>CFBundleExecutable</key><string>MicCamWatchHelper</string>
<key>CFBundleName</key><string>MicCamWatch</string>
<key>CFBundleDisplayName</key><string>MicCamWatch</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleVersion</key><string>{version}</string>
<key>CFBundleShortVersionString</key><string>{version}</string>
<key>LSMinimumSystemVersion</key><string>15.0</string>
<key>LSUIElement</key><true/>
<key>NSHighResolutionCapable</key><true/>
<key>NSPrincipalClass</key><string>NSApplication</string>
<key>NSCameraUsageDescription</key><string>MicCamWatch passively discovers cameras and reports capture by other applications. Status and watch never request camera access or create a capture session.</string>
<key>NSMicrophoneUsageDescription</key><string>MicCamWatch passively observes CoreAudio input clients and inventories input devices. Status and watch never request microphone access or record audio.</string>
</dict></plist>
"#
    );
    fs::write(contents.join("Info.plist"), plist)
        .expect("cannot create macOS helper application metadata");
    let helper = contents.join("MacOS/MicCamWatchHelper");
    let target = env::var("TARGET").expect("Cargo must set TARGET");
    let arch = match target.split('-').next() {
        Some("aarch64") => "arm64",
        Some("x86_64") => "x86_64",
        _ => panic!("unsupported macOS helper architecture: {target}"),
    };
    let mut compiler = Command::new("swiftc");
    compiler
        .args(["-target", &format!("{arch}-apple-macosx15.0"), "-O"])
        .arg(&entrypoint)
        .args(&sources[1..])
        .arg("-import-objc-header")
        .arg(&bridge)
        .arg("-o")
        .arg(&helper)
        .args(["-lproc", "-lbsm"]);
    for framework in [
        "AppKit",
        "AVFoundation",
        "CoreAudio",
        "CoreGraphics",
        "Foundation",
        "Security",
        "UserNotifications",
    ] {
        compiler.args(["-framework", framework]);
    }
    let output = compiler
        .output()
        .expect("Swift compiler required to build the embedded macOS capture helper");
    assert!(
        output.status.success(),
        "macOS capture helper did not compile: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    // Ad-hoc signing seals this local bundle and gives notification/AppKit modes
    // a stable code identifier. It is not Developer ID signing or notarization.
    let signed = Command::new("codesign")
        .args([
            "--force",
            "--sign",
            "-",
            "--identifier",
            "com.roman-cuisset.miccamwatch.helper",
            "--timestamp=none",
        ])
        .arg(&bundle)
        .output()
        .expect("codesign required to seal the embedded macOS helper application");
    assert!(
        signed.status.success(),
        "cannot ad-hoc sign macOS helper application: {}",
        String::from_utf8_lossy(&signed.stderr)
    );
}

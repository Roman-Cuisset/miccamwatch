param(
    [Parameter(Mandatory = $true)][string]$Version,
    [Parameter(Mandatory = $true)][ValidatePattern('^[0-9a-fA-F]{64}$')][string]$InstallerSha256,
    [ValidatePattern('^\d+\.\d+\.\d+$')][string]$ManifestVersion = '1.28.0',
    # Where to write manifests/r/RomanCuisset/MicCamWatch/<version>/. Defaults to
    # this script's directory; pass a scratch directory to validate a draft
    # without touching the manifests committed to this repository.
    [string]$OutputRoot = $PSScriptRoot,
    # Pass this for releases whose executables still import VCRUNTIME140.dll
    # (0.21.0 and every earlier release). Releases built with the static CRT must
    # NOT declare it, so a missing runtime can never be silently assumed.
    [switch]$RuntimeDependency
)

$ErrorActionPreference = 'Stop'
$identifier = 'RomanCuisset.MicCamWatch'
$baseUrl = "https://github.com/Roman-Cuisset/miccamwatch/releases/download/v$Version"
$output = Join-Path $OutputRoot "manifests/r/RomanCuisset/MicCamWatch/$Version"
New-Item -ItemType Directory -Force -Path $output | Out-Null

$dependencies = ''
if ($RuntimeDependency) {
    $dependencies = @"

Dependencies:
  PackageDependencies:
    - PackageIdentifier: Microsoft.VCRedist.2015+.x64
      MinimumVersion: 14.0.0.0
"@
}

@"
# yaml-language-server: `$schema=https://aka.ms/winget-manifest.version.$ManifestVersion.schema.json
PackageIdentifier: $identifier
PackageVersion: $Version
DefaultLocale: en-US
ManifestType: version
ManifestVersion: $ManifestVersion
"@ | Set-Content -Encoding utf8 (Join-Path $output "$identifier.yaml")

@"
# yaml-language-server: `$schema=https://aka.ms/winget-manifest.installer.$ManifestVersion.schema.json
PackageIdentifier: $identifier
PackageVersion: $Version
InstallerLocale: en-US
InstallerType: wix
Scope: user
InstallModes:
  - interactive
  - silent
  - silentWithProgress
UpgradeBehavior: install
Commands:
  - mcw
Installers:
  - Architecture: x64
    InstallerUrl: $baseUrl/miccamwatch-windows-x86_64.msi
    InstallerSha256: $($InstallerSha256.ToUpperInvariant())
$dependencies
ManifestType: installer
ManifestVersion: $ManifestVersion
"@ | Set-Content -Encoding utf8 (Join-Path $output "$identifier.installer.yaml")

@"
# yaml-language-server: `$schema=https://aka.ms/winget-manifest.defaultLocale.$ManifestVersion.schema.json
PackageIdentifier: $identifier
PackageVersion: $Version
PackageLocale: en-US
Publisher: Roman Cuisset
PublisherUrl: https://github.com/Roman-Cuisset
PublisherSupportUrl: https://github.com/Roman-Cuisset/miccamwatch/issues
PackageName: MicCamWatch
PackageUrl: https://github.com/Roman-Cuisset/miccamwatch
License: MIT
LicenseUrl: https://github.com/Roman-Cuisset/miccamwatch/blob/v$Version/LICENSE
ShortDescription: Monitor and control Windows microphone and camera privacy.
Description: MicCamWatch attributes microphone and camera activity to local processes and provides a tray-based privacy control center.
Tags:
  - camera
  - microphone
  - privacy
  - security
  - windows
ReleaseNotesUrl: https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v$Version
ManifestType: defaultLocale
ManifestVersion: $ManifestVersion
"@ | Set-Content -Encoding utf8 (Join-Path $output "$identifier.locale.en-US.yaml")

Write-Output $output

param(
    [Parameter(Mandatory = $true)][string]$Version,
    [Parameter(Mandatory = $true)][ValidatePattern('^[0-9a-fA-F]{64}$')][string]$InstallerSha256
)

$ErrorActionPreference = 'Stop'
$identifier = 'RomanCuisset.MicCamWatch'
$baseUrl = "https://github.com/Roman-Cuisset/miccamwatch/releases/download/v$Version"
$output = Join-Path $PSScriptRoot "manifests/r/RomanCuisset/MicCamWatch/$Version"
New-Item -ItemType Directory -Force -Path $output | Out-Null

@"
PackageIdentifier: $identifier
PackageVersion: $Version
DefaultLocale: en-US
ManifestType: version
ManifestVersion: 1.9.0
"@ | Set-Content -Encoding utf8 (Join-Path $output "$identifier.yaml")

@"
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
ManifestType: installer
ManifestVersion: 1.9.0
"@ | Set-Content -Encoding utf8 (Join-Path $output "$identifier.installer.yaml")

@"
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
ManifestVersion: 1.9.0
"@ | Set-Content -Encoding utf8 (Join-Path $output "$identifier.locale.en-US.yaml")

Write-Output $output

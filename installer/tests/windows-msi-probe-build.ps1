[CmdletBinding()]
param([Parameter(Mandatory)][string]$Output)
$ErrorActionPreference = 'Stop'
if (Test-Path -LiteralPath $Output) { throw 'MSI probe output directory must be new' }
$root = (New-Item -ItemType Directory -Path $Output).FullName
$cases = @(
    @{ Name='x64-early-error'; Arch='x64'; Schedule='afterInstallInitialize'; Failure='type19' },
    @{ Name='x64-early-media'; Arch='x64'; Schedule='afterInstallInitialize'; Failure='native-media' },
    @{ Name='arm64-early-media'; Arch='arm64'; Schedule='afterInstallInitialize'; Failure='native-media' },
    @{ Name='x64-late-error'; Arch='x64'; Schedule='afterInstallExecute'; Failure='type19' }
)

function New-Package($Case, [string]$Folder, [string]$Variant, [string]$Version, [string]$Code) {
    $dir = (New-Item -ItemType Directory -Path (Join-Path $Folder $Variant)).FullName
    $payload = Join-Path $dir 'marker.txt'
    [IO.File]::WriteAllText($payload, "MicCamWatch MSI registry probe $($Case.Name) version $Version")
    $xml = Join-Path $dir 'probe.wxs'
    $embed = 'yes'
    $errorAction = ''
    if ($Variant -eq 'fault' -and $Case.Failure -eq 'native-media') { $embed = 'no' }
    if ($Variant -eq 'fault' -and $Case.Failure -eq 'type19') {
        $errorAction = @'
    <CustomAction Id="ProbeFailure" Error="MicCamWatch MSI registry probe: expected Type 19 failure" />
    <InstallExecuteSequence>
      <Custom Action="ProbeFailure" After="RemoveExistingProducts" Condition="1" />
    </InstallExecuteSequence>
'@
    }
    [IO.File]::WriteAllText($xml, @"
<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs">
  <Package Name="MicCamWatch MSI Probe $($Case.Name)" Manufacturer="MicCamWatch qualification" Version="$Version"
           ProductCode="$Code" UpgradeCode="$($Case.UpgradeCode)" Scope="perUser">
    <MajorUpgrade Schedule="$($Case.Schedule)" DowngradeErrorMessage="A newer probe is already installed." />
    <Media Id="1" Cabinet="probe-media.cab" EmbedCab="$embed" />
    <SetProperty Id="ARPINSTALLLOCATION" Value="[INSTALLFOLDER]" After="CostFinalize" />
    $errorAction
    <StandardDirectory Id="LocalAppDataFolder">
      <Directory Id="INSTALLFOLDER" Name="$($Case.TargetFolder)">
        <Component Id="Marker" Guid="$($Case.ComponentCode)">
          <RegistryValue Root="HKCU" Key="Software\MicCamWatch-MSI-Probe\$($Case.TargetFolder)"
                         Name="Marker" Type="integer" Value="1" KeyPath="yes" />
          <File Id="ProbeFile" Name="marker.txt" Source="$([Security.SecurityElement]::Escape($payload))" />
        </Component>
      </Directory>
    </StandardDirectory>
    <Feature Id="Probe" Title="Registry probe" Level="1"><ComponentRef Id="Marker" /></Feature>
  </Package>
</Wix>
"@)
    $msi = Join-Path $dir 'probe.msi'
    & dotnet wix build -arch $Case.Arch -pdbtype none -o $msi $xml | Out-Host
    if ($LASTEXITCODE -ne 0) { throw "WiX failed for $($Case.Name)/$Variant" }
    if ($embed -eq 'no' -and !(Test-Path -LiteralPath (Join-Path $dir 'probe-media.cab') -PathType Leaf)) {
        throw 'Native failure probe must have a WiX-generated external cabinet'
    }
    return (Get-FileHash -LiteralPath $msi -Algorithm SHA256).Hash
}

foreach ($case in $cases) {
    $case.UpgradeCode = [Guid]::NewGuid().ToString('B').ToUpperInvariant()
    $case.ComponentCode = [Guid]::NewGuid().ToString('B').ToUpperInvariant()
    $case.OldCode = [Guid]::NewGuid().ToString('B').ToUpperInvariant()
    $case.NewCode = [Guid]::NewGuid().ToString('B').ToUpperInvariant()
    $case.TargetFolder = 'McwMsiProbe-' + ([Guid]$case.UpgradeCode).ToString('N')
    $folder = (New-Item -ItemType Directory -Path (Join-Path $root $case.Name)).FullName
    $case.OldHash = New-Package $case $folder 'baseline' '1.0.0' $case.OldCode
    $case.FaultHash = New-Package $case $folder 'fault' '2.0.0' $case.NewCode
    $case.GoodHash = New-Package $case $folder 'good' '2.0.0' $case.NewCode
}
$cases | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $root 'probes.json') -Encoding utf8

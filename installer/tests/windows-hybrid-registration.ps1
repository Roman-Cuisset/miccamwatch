# Read-only observer for the other account's per-user unmanaged MSI context.
if (-not ('McwMsiRegistration' -as [type])) {
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;

public static class McwMsiRegistration {
    [DefaultDllImportSearchPaths(DllImportSearchPath.System32)]
    [DllImport("msi.dll", CharSet = CharSet.Unicode, ExactSpelling = true)]
    public static extern uint MsiGetProductInfoExW(string code, string sid, uint context,
        string property, StringBuilder value, ref uint length);
    [DefaultDllImportSearchPaths(DllImportSearchPath.System32)]
    [DllImport("msi.dll", CharSet = CharSet.Unicode, ExactSpelling = true)]
    public static extern uint MsiEnumProductsExW(string code, string sid, uint context, uint index,
        StringBuilder product, out uint actualContext, StringBuilder actualSid, ref uint sidLength);
}
'@
}

function Get-McwMsiProperty([string]$Code, [string]$Sid, [string]$Property) {
    $buffer = [Text.StringBuilder]::new(512)
    [uint32]$length = $buffer.Capacity
    $status = [McwMsiRegistration]::MsiGetProductInfoExW($Code, $Sid, 2, $Property, $buffer, [ref]$length)
    if ($status -eq 234) {
        $buffer = [Text.StringBuilder]::new([int]$length + 1)
        $length++
        $status = [McwMsiRegistration]::MsiGetProductInfoExW($Code, $Sid, 2, $Property, $buffer, [ref]$length)
    }
    if ($status -eq 0) { return @{ status = $status; value = $buffer.ToString() } }
    return @{ status = $status; value = $null }
}

function ConvertTo-McwPackedGuid([string]$Code) {
    $fields = ([Guid]$Code).ToString('D').Split('-')
    $packed = foreach ($field in $fields[0..2]) {
        $chars = $field.ToCharArray()
        [Array]::Reverse($chars)
        -join $chars
    }
    (-join $packed) + [regex]::Replace(($fields[3] + $fields[4]), '(.)(.)', '$2$1')
}

function Get-McwMsiContexts([string]$Code, [string]$Sid) {
    foreach ($context in 1,2,4) {
        $requestedSid = $Sid
        if ($context -eq 4) { $requestedSid = [System.Management.Automation.Language.NullString]::Value }
        $product = [Text.StringBuilder]::new(39)
        $user = [Text.StringBuilder]::new(256)
        [uint32]$sidLength = $user.Capacity
        [uint32]$actual = 0
        $status = [McwMsiRegistration]::MsiEnumProductsExW($Code, $requestedSid, $context, 0,
            $product, [ref]$actual, $user, [ref]$sidLength)
        @{ requested=$context; status=$status; context=$actual; product=$product.ToString(); sid=$user.ToString() }
    }
}

function Get-McwMsiRegistration([string]$Code, [string]$Sid, [string]$UpgradeCode = '{8FC9317B-4517-4E83-9F52-CF3257584BF6}') {
    if ($Code -cnotmatch '^\{[0-9A-Fa-f]{8}(?:-[0-9A-Fa-f]{4}){3}-[0-9A-Fa-f]{12}\}$') {
        throw 'Invalid product code in observation request'
    }
    $entry = [ordered]@{ productCode = $Code; installContext = 'per-user-unmanaged'; properties = [ordered]@{} }
    $upgrade = ConvertTo-McwPackedGuid $UpgradeCode
    try {
        $key = [Microsoft.Win32.Registry]::Users.OpenSubKey("$Sid\Software\Microsoft\Installer\UpgradeCodes\$upgrade", $false)
        $entry.userUpgradeIndexPresent = $null -ne $key
        $entry.productInUserUpgradeIndex = $false
        if ($key) {
            try { $entry.productInUserUpgradeIndex = $key.GetValueNames() -contains (ConvertTo-McwPackedGuid $Code) }
            finally { $key.Dispose() }
        }
    } catch { $entry.upgradeIndexError = $_.ToString() }
    foreach ($name in 'State','VersionString','InstallLocation','LocalPackage') {
        $entry.properties[$name] = Get-McwMsiProperty $Code $Sid $name
    }
    $cache = $entry.properties.LocalPackage.value
    $cacheRoot = Join-Path $env:WINDIR 'Installer'
    if ($entry.properties.LocalPackage.status -ne 0) { return $entry }
    if (!$cache) { $entry.cacheError = 'The registered local package path is empty.'; return $entry }
    if ((Split-Path -Parent $cache) -ine $cacheRoot -or
        (Split-Path -Leaf $cache) -notmatch '^[a-f0-9]+\.msi$') {
        $entry.cacheError = 'The registered local package is outside the controlled MSI cache.'
        return $entry
    }
    $file = Get-Item -LiteralPath $cache -Force -ErrorAction SilentlyContinue
    if (!$file -or $file.PSIsContainer -or ($file.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        $entry.cacheError = 'The registered local package is missing or redirected.'
        return $entry
    }
    $service = New-Object -ComObject WindowsInstaller.Installer
    try {
        $database = $service.OpenDatabase($file.FullName, 0)
        try {
            $entry.cacheIdentity = [ordered]@{}
            foreach ($property in 'ProductCode','UpgradeCode','ProductVersion') {
                $view = $database.OpenView("SELECT ``Value`` FROM ``Property`` WHERE ``Property``='$property'")
                try {
                    [void]$view.Execute()
                    $row = $view.Fetch()
                    if ($null -ne $row) {
                        try { $entry.cacheIdentity[$property] = $row.StringData(1) }
                        finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($row) }
                    }
                } finally {
                    [void]$view.Close()
                    [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($view)
                }
            }
        } finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($database) }
    } catch { $entry.cacheError = $_.ToString() }
    finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($service) }
    return $entry
}

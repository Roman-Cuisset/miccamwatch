param([Parameter(Mandatory=$true)][string]$OutputDirectory)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
# This exercises context observation/admission only. It never runs an MCW
# binary, installer, uninstaller, account helper or device-control command.
$output=[IO.Path]::GetFullPath($OutputDirectory)
$temporaryRoots=@($env:TEMP,$env:RUNNER_TEMP) | Where-Object { $_ } | ForEach-Object { [IO.Path]::GetFullPath($_).TrimEnd('\')+'\' }
if (-not @($temporaryRoots | Where-Object { $output.StartsWith($_,[StringComparison]::OrdinalIgnoreCase) }).Count -or (Test-Path -LiteralPath $output)) {
    throw 'Smoke evidence requires a new directory beneath the actual TEMP or RUNNER_TEMP.'
}
$ancestor=$output
while ($ancestor) {
    if ((Test-Path -LiteralPath $ancestor) -and ((Get-Item -LiteralPath $ancestor -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Reparse-point smoke ancestry refused.' }
    $ancestor=Split-Path -Parent $ancestor
}
[void](New-Item -ItemType Directory -Path $output)
$report=[ordered]@{safetySmokeStatus='running';qualificationStatus='not_executed';productActionsInvoked=$false;parsedScripts=@();compiledTypes=0;actualStandardContextAccepted=$false;actualHostAdmissionAccepted=$false;error=$null}
try {
    foreach ($name in @('windows-fixture-context.ps1','windows-application-native.ps1','windows-standard-user.ps1','windows-context-smoke.ps1')) {
        $path=Join-Path $PSScriptRoot $name
        $tokens=$null; $errors=$null
        $ast=[Management.Automation.Language.Parser]::ParseFile($path,[ref]$tokens,[ref]$errors)
        if ($errors.Count) { throw "PowerShell parse failed for ${name}: $($errors | Out-String)" }
        $report.parsedScripts+= $name
        # Our own compiler call uses an AST-derived variable, not a fixture
        # definition. Parse this script but do not recursively compile itself.
        if ($name -ceq 'windows-context-smoke.ps1') { continue }
        # Compile the actual literal C# definitions, not the application script.
        # No constructors or native application lifecycle methods are invoked.
        foreach ($command in $ast.FindAll({param($node) $node -is [Management.Automation.Language.CommandAst] -and $node.GetCommandName() -eq 'Add-Type'},$true)) {
            $definition=$null; $drawing=$false
            for ($index=0;$index -lt $command.CommandElements.Count;$index++) {
                $element=$command.CommandElements[$index]
                if ($element -is [Management.Automation.Language.CommandParameterAst]) {
                    if ($element.ParameterName -eq 'TypeDefinition') { $definition=$command.CommandElements[$index+1] }
                    if ($element.ParameterName -eq 'ReferencedAssemblies') { $drawing=$true }
                }
            }
            if ($null -eq $definition) { continue }
            if ($definition -isnot [Management.Automation.Language.StringConstantExpressionAst]) { throw 'Only actual literal C# definitions are admitted by the readonly smoke.' }
            if ($drawing) { Add-Type -AssemblyName System.Drawing; Add-Type -TypeDefinition $definition.Value -ReferencedAssemblies System.Drawing }
            else { Add-Type -TypeDefinition $definition.Value }
            $report.compiledTypes++
        }
    }
    . (Join-Path $PSScriptRoot 'windows-fixture-context.ps1')
    $context=Get-McwStandardUserContext
    $context | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $output 'private-actual-context.json') -Encoding UTF8
    try { Assert-McwStandardUserContext -Context $context; $report.actualStandardContextAccepted=$true }
    catch { $report['standardContextRefusal']=$_.Exception.Message }
    # Ground expected rejection in actual native account/session observations.
    $actuallyUnsuitable=$context.elevated -or $context.accountPrivilege -ne 1 -or
        $context.accountLocalGroupSids -contains 'S-1-5-32-544' -or $context.tokenGroupSids -contains 'S-1-5-32-544' -or
        $context.sessionId -eq 0 -or $context.windowStation -cne 'WinSta0'
    if ($actuallyUnsuitable -and $report.actualStandardContextAccepted) { throw 'An actually administrative/noninteractive context was incorrectly admitted as standard.' }
    try { $null=Assert-McwDisposableFixtureHost -RequireStandardUser; $report.actualHostAdmissionAccepted=$true }
    catch { $report['hostAdmissionRefusal']=$_.Exception.Message }
    if (($env:GITHUB_ACTIONS -cne 'true' -or $actuallyUnsuitable) -and $report.actualHostAdmissionAccepted) {
        throw 'An actual personal or unsuitable runner context was incorrectly admitted.'
    }
    $report['observedStation']=$context.windowStation
    $report['observedSessionId']=$context.sessionId
    $report['observedLogonType']=$context.logonType
    $report.safetySmokeStatus='passed'
} catch {
    $report.safetySmokeStatus='failed'; $report.error=$_.Exception.Message
    throw
} finally {
    $report | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding UTF8
}
Write-Output ('PASS readonly context/parsing/C# safety smoke; GUI/MSI/uninstall qualification NOT EXECUTED. Evidence: '+$output)

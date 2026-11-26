[CmdletBinding()]
param([Parameter(Mandatory)]$Session, [Parameter(Mandatory)]$Request)
# Host controller: every guest operation is bound to this explicitly authorized VM.
# Credentials are supplied by the caller's in-memory PowerShell Direct session.
$ErrorActionPreference = 'Stop'
$vmId = [guid]'cb46ea19-9415-423a-9b89-2459da960886'
$uuid = [guid]'4B01C3A6-C913-41AC-B759-66148DFCAB64'
$sid = 'S-1-5-21-1745080491-1157776599-3695790684-1002'
$adminSid = 'S-1-5-21-1745080491-1157776599-3695790684-1001'
$localRoot = 'C:\Users\stcui\Documents\miccamwatch\target\stage4-qualification'
$runId = [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss')
$proof = Join-Path $localRoot "lab-consolidated-$runId"
$inputRoot = "C:\MCWLab\Stage4-Input-$runId"
$guestRoot = "C:\Users\mcw_qa_lab01\AppData\Local\MCWStage4-$runId"
$report = [ordered]@{status='starting'; vmId=$vmId; uuid=$uuid; source=$Request.source; guestRoot=$guestRoot; operations=[ordered]@{}}
$report.harnessHashes=@(foreach($file in 'windows-stage4-lab.ps1','windows-hybrid-install.ps1','windows-stage4-observer.ps1','windows-fixture-context.ps1','windows-hybrid-registration.ps1') {
    $path=Join-Path $PSScriptRoot $file
    @{file=$file;sha256=(Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()}
})
New-Item -ItemType Directory -Path $proof | Out-Null
function Save-LabReport {
    $report.utc=[DateTimeOffset]::UtcNow.ToString('o')
    $report | ConvertTo-Json -Depth 18 | Set-Content -LiteralPath (Join-Path $proof 'controller.json') -Encoding utf8
}
function Assert-Lab {
    $vm=Get-VM -Id $vmId
    if ($vm.Name -cne 'MCW-Win11-x64-Lab' -or $vm.State -ne 'Running' -or
        $vm.Generation -ne 2 -or (Get-VMNetworkAdapter -VM $vm).SwitchName -or
        !(Get-VMSnapshot -VM $vm | Where-Object Id -eq ([guid]'04798105-5553-41a8-90c1-06930f1319c7')) -or
        $Session.State -ne 'Opened' -or $Session.ComputerName -ine $vm.Name) { throw 'Lab identity or isolation changed.' }
}
function Invoke-Lab([scriptblock]$Command, [object[]]$Parameters=@()) {
    Assert-Lab
    Invoke-Command -Session $Session -ScriptBlock {
        if ([guid](Get-CimInstance Win32_ComputerSystemProduct).UUID -ne [guid]'4B01C3A6-C913-41AC-B759-66148DFCAB64' -or
            $env:PROCESSOR_ARCHITECTURE -cne 'AMD64' -or
            [Security.Principal.WindowsIdentity]::GetCurrent().User.Value -cne 'S-1-5-21-1745080491-1157776599-3695790684-1001') {
            throw 'Not the admitted x64 laboratory administrator.'
        }
    } -ErrorAction Stop
    Invoke-Command -Session $Session -ScriptBlock $Command -ArgumentList $Parameters -ErrorAction Stop
}
function Run-Standard([string]$Phase, [string]$File) {
    $name="MCW-Stage4-$runId-$Phase"
    $errorFile="C:\Users\mcw_qa_lab01\AppData\Local\Temp\MCW-Stage4-$runId-$Phase.txt"
    $wrapper="try { & '$File' } catch { `$_ | Out-String | Set-Content -LiteralPath '$errorFile' -Encoding utf8; exit 1 }"
    # Explicit owner approval on 2026-11-26: QA test processes only. No persistent
    # execution policy, GPO or Windows Installer policy is modified.
    $arguments='-NoProfile -NonInteractive -WindowStyle Hidden -ExecutionPolicy RemoteSigned -EncodedCommand '+[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($wrapper))
    Invoke-Lab {
        param($name,$arguments)
        if ((Get-CimInstance Win32_ComputerSystem).UserName -ine "$env:COMPUTERNAME\mcw_qa_lab01" -or
            (Get-LocalUser mcw_qa_lab01).SID.Value -cne 'S-1-5-21-1745080491-1157776599-3695790684-1002') { throw 'QA desktop is not logged in.' }
        if (Get-ScheduledTask -TaskName $name -ErrorAction SilentlyContinue) { throw 'Unexpected existing task.' }
        $principal=New-ScheduledTaskPrincipal -UserId "$env:COMPUTERNAME\mcw_qa_lab01" -LogonType Interactive -RunLevel Limited
        $action=New-ScheduledTaskAction -Execute "$env:WINDIR\System32\WindowsPowerShell\v1.0\powershell.exe" -Argument $arguments
        Register-ScheduledTask -TaskName $name -Principal $principal -Action $action | Out-Null
        Start-ScheduledTask -TaskName $name
    } @($name,$arguments)
    $deadline=[DateTime]::UtcNow.AddMinutes(35)
    do {
        Start-Sleep -Seconds 1
        $state=Invoke-Lab {
            param($name)
            $task=Get-ScheduledTask -TaskName $name
            $info=Get-ScheduledTaskInfo -TaskName $name
            @{state=[string]$task.State; result=$info.LastTaskResult; last=$info.LastRunTime.ToUniversalTime().ToString('o')}
        } @($name)
        $report.operations[$Phase]=$state
        Save-LabReport
        if ($state.state -eq 'Ready' -and $state.result -ne 267009) { break }
    } while ([DateTime]::UtcNow -lt $deadline)
    if ($state.state -ne 'Ready') { throw "Task $name did not finish; preserve it and inspect the guest, never start another MSI." }
    Invoke-Lab {param($name) Unregister-ScheduledTask -TaskName $name -Confirm:$false} @($name)
    if ($state.result -ne 0) { throw "Standard-account phase $Phase failed with $($state.result)." }
}
try {
    Assert-Lab
    $package=[IO.Path]::GetFullPath([string]$Request.package)
    if (!$package.StartsWith($localRoot+'\',[StringComparison]::OrdinalIgnoreCase) -or
        $Request.source -cnotmatch '^[0-9a-f]{40}$' -or $Request.msiSha256 -cnotmatch '^[0-9a-f]{64}$') { throw 'Unexpected local package identity.' }
    $msi=Join-Path $package 'miccamwatch-hybrid-windows-x86_64.msi'
    $bundle=Get-Content -LiteralPath (Join-Path $package 'bundle\miccamwatch-bundle.json') -Raw | ConvertFrom-Json
    if ((Get-FileHash -LiteralPath $msi).Hash -ine $Request.msiSha256 -or $bundle.revision -cne $Request.source -or $bundle.ipc -ne 10) { throw 'Exact candidate provenance mismatch.' }
    $baseline='C:\Users\stcui\Documents\miccamwatch\target\mcw-hyperv-lab-inventory\product-inputs\legacy-v0.21.0\miccamwatch-windows-x86_64.msi'
    if ((Get-FileHash -LiteralPath $baseline).Hash -ine '4e6145a4e48bff47f08c8341c24c14e7fab563146e36b8338f46441f7f4da3df') { throw 'Pinned baseline digest mismatch.' }
    $report.before=Invoke-Lab {
        $service=Get-CimInstance Win32_Service -Filter "Name='msiserver'"
        $clients=@(Get-CimInstance Win32_Process -Filter "Name='msiexec.exe'" | Where-Object ProcessId -ne $service.ProcessId)
        $applications=@(Get-Process mcw,mcw-tray,miccamwatch_desktop -ErrorAction SilentlyContinue)
        $tasks=@(Get-ScheduledTask -TaskName 'MCW-*' -ErrorAction SilentlyContinue | Where-Object State -eq Running)
        if ($clients.Count -or $applications.Count -or $tasks.Count) { throw 'A guest installer, application or previous test remains active.' }
        if ((Get-PSDrive C).Free -lt 10GB) { throw 'Guest disk space is insufficient.' }
        @{freeGiB=[math]::Round((Get-PSDrive C).Free/1GB,2); loggedIn=(Get-CimInstance Win32_ComputerSystem).UserName; noMsiClients=$true}
    }
    Invoke-Lab {
        param($path)
        if (Test-Path -LiteralPath $path) {throw 'Input root already exists.'}
        New-Item -ItemType Directory -Path $path | Out-Null
        $acl=[Security.AccessControl.DirectorySecurity]::new()
        $acl.SetAccessRuleProtection($true,$false)
        foreach($identity in 'S-1-5-18','S-1-5-32-544','S-1-5-21-1745080491-1157776599-3695790684-1001','S-1-5-21-1745080491-1157776599-3695790684-1002') {
            $rights=if($identity -eq 'S-1-5-21-1745080491-1157776599-3695790684-1002'){'ReadAndExecute'}else{'FullControl'}
            $rule=[Security.AccessControl.FileSystemAccessRule]::new([Security.Principal.SecurityIdentifier]::new($identity),$rights,'ContainerInherit,ObjectInherit','None','Allow')
            $acl.AddAccessRule($rule)
        }
        Set-Acl -LiteralPath $path -AclObject $acl
    } @($inputRoot)
    foreach ($file in 'windows-hybrid-install.ps1','windows-fixture-context.ps1','windows-hybrid-registration.ps1','windows-stage4-observer.ps1') {
        Assert-Lab
        Copy-Item -LiteralPath (Join-Path $PSScriptRoot $file) -Destination "$inputRoot\$file" -ToSession $Session
    }
    Assert-Lab
    Copy-Item -LiteralPath $msi -Destination "$inputRoot\candidate.msi" -ToSession $Session
    Copy-Item -LiteralPath $baseline -Destination "$inputRoot\baseline.msi" -ToSession $Session
    Copy-Item -LiteralPath (Join-Path $package 'bundle') -Destination "$inputRoot\bundle" -ToSession $Session -Recurse
    $prepare=@'
$ErrorActionPreference='Stop'
if ([guid](Get-CimInstance Win32_ComputerSystemProduct).UUID -ne [guid]'4B01C3A6-C913-41AC-B759-66148DFCAB64' -or
    [Security.Principal.WindowsIdentity]::GetCurrent().User.Value -cne 'S-1-5-21-1745080491-1157776599-3695790684-1002') { throw 'Wrong standard guest.' }
. '__INPUT__\windows-fixture-context.ps1'
$context=Get-McwStandardUserContext
Assert-McwStandardUserContext $context
$installer=New-Object -ComObject WindowsInstaller.Installer
$related=$installer.RelatedProducts('{8FC9317B-4517-4E83-9F52-CF3257584BF6}')
$count=$related.GetType().InvokeMember('Count',[Reflection.BindingFlags]::GetProperty,$null,$related,$null)
[void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($related)
if ($count -isnot [int] -or $count -ne 0) { throw 'A related MSI is already registered; inspect rather than reinstall.' }
$root='__ROOT__'
if (Test-Path -LiteralPath $root) {throw 'Standard fixture root already exists.'}
$base=Join-Path $env:LOCALAPPDATA 'Programs\MicCamWatch'
$run=Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -ErrorAction SilentlyContinue
if (@($run.PSObject.Properties | Where-Object { $_.Name -like '*MicCamWatch*' }).Count -or
    (Get-ScheduledTask -TaskName 'MicCamWatch Tray' -ErrorAction SilentlyContinue)) {throw 'Existing guest autostart needs inspection.'}
New-Item -ItemType Directory -Path $root | Out-Null
Copy-Item -Path '__INPUT__\*' -Destination $root -Recurse
New-Item -ItemType Directory -Path "$root\observations","$root\preserved-before" | Out-Null
$moves=@()
foreach ($entry in @(@{path=$base;name='installation'},@{path=(Join-Path $env:APPDATA 'MicCamWatch');name='configuration'},
    @{path=(Join-Path $env:LOCALAPPDATA 'MicCamWatch');name='data'},
    @{path=(Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\MicCamWatch.lnk');name='shortcut.lnk'})) {
    if (!(Test-Path -LiteralPath $entry.path)) {continue}
    $item=Get-Item -LiteralPath $entry.path -Force
    $destination=[IO.Path]::GetFullPath((Join-Path "$root\preserved-before" $entry.name))
    if (!$item.FullName.StartsWith('C:\Users\mcw_qa_lab01\',[StringComparison]::OrdinalIgnoreCase) -or
        !$destination.StartsWith($root+'\preserved-before\',[StringComparison]::OrdinalIgnoreCase) -or
        ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -or
        @(Get-ChildItem -LiteralPath $item.FullName -Recurse -Force | Where-Object { $_.Attributes -band [IO.FileAttributes]::ReparsePoint }).Count) {throw 'Unsafe guest preservation path.'}
    $hashes=@(Get-ChildItem -LiteralPath $item.FullName -File -Recurse -Force | Get-FileHash -Algorithm SHA256 | Select-Object Path,Hash)
    Move-Item -LiteralPath $item.FullName -Destination $destination
    $moves+=@{from=$item.FullName;to=$destination;hashes=$hashes}
}
@{context=$context;moves=$moves} | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath "$root\preparation.json" -Encoding utf8
'@.Replace('__INPUT__',$inputRoot).Replace('__ROOT__',$guestRoot)
    Invoke-Lab {param($path,$text) [IO.File]::WriteAllText($path,$text,[Text.UTF8Encoding]::new($true))} @("$inputRoot\prepare.ps1",$prepare)
    Run-Standard 'prepare' "$inputRoot\prepare.ps1"
    if ($Request.prepareOnly) { $report.status='prepared-without-msi'; return }
    $report.observer=Invoke-Lab {
        param($inputRoot,$guestRoot,$preflight)
        $installer=New-Object -ComObject WindowsInstaller.Installer
        $codes=@(foreach($file in 'candidate.msi','baseline.msi') {
            $db=$installer.OpenDatabase((Join-Path $inputRoot $file),0)
            $view=$db.OpenView("SELECT ``Value`` FROM ``Property`` WHERE ``Property``='ProductCode'")
            [void]$view.Execute();$record=$view.Fetch();$record.StringData(1)
            [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($record)
            [void]$view.Close();[void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($view)
            [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($db)
        })
        $arguments='-NoProfile -NonInteractive -WindowStyle Hidden -ExecutionPolicy RemoteSigned -File "'+$inputRoot+'\windows-stage4-observer.ps1" -FixtureRoot "'+$guestRoot+'" -CandidateCode "'+$codes[0]+'" -BaselineCode "'+$codes[1]+'"'
        if($preflight){$arguments+=' -Preflight'}
        $process=Start-Process -FilePath "$env:WINDIR\System32\WindowsPowerShell\v1.0\powershell.exe" -ArgumentList $arguments -WindowStyle Hidden -PassThru
        if($preflight -and (!$process.WaitForExit(30000) -or $process.ExitCode -ne 0)){throw 'Read-only observer preflight failed.'}
        @{pid=$process.Id;candidateCode=$codes[0];baselineCode=$codes[1];role='read-only MSI observer; process-scoped RemoteSigned explicitly authorized'}
    } @($inputRoot,$guestRoot,[bool]$Request.observerOnly)
    if($Request.observerOnly){$report.status='observer-verified-without-msi';return}
    $work=@'
$ErrorActionPreference='Stop'
$root='__ROOT__'
try {
    if ([guid](Get-CimInstance Win32_ComputerSystemProduct).UUID -ne [guid]'4B01C3A6-C913-41AC-B759-66148DFCAB64') {throw 'Wrong guest.'}
    & "$root\windows-hybrid-install.ps1" -Candidate "$root\candidate.msi" -Proof "$root\proof" -SourceRevision '__SOURCE__' -LocalDisposableVm -FixtureRoot $root -ExpectedMsiSha256 '__HASH__' -BaselineMsi "$root\baseline.msi" -PositiveLifecycleOnly -ObserveRegistration -ObservationRoot '__INPUT__\observations' -SampleDesktopResources
} catch {
    $_ | Out-String | Set-Content -LiteralPath "$root\workload-error.txt" -Encoding utf8
    exit 1
}
'@.Replace('__ROOT__',$guestRoot).Replace('__INPUT__',$inputRoot).Replace('__SOURCE__',[string]$Request.source).Replace('__HASH__',[string]$Request.msiSha256)
    Invoke-Lab {param($path,$text) [IO.File]::WriteAllText($path,$text,[Text.UTF8Encoding]::new($true))} @("$guestRoot\run.ps1",$work)
    $report.status='qualifying'; Save-LabReport
    Run-Standard 'lifecycle' "$guestRoot\run.ps1"
    $report.result=Invoke-Lab {param($path) Get-Content -LiteralPath $path -Raw | ConvertFrom-Json} @("$guestRoot\proof\result.json")
    if ($report.result.status -cne 'passed' -or !$report.result.standardUserQualified -or !$report.result.installedIpcReady -or
        !$report.result.installedFlutterClosedTrayContinued -or !$report.result.userDataPreserved -or !$report.result.msiUninstall) {throw 'Consolidated lifecycle receipt is incomplete.'}
    $report.status='passed'
} catch {
    $report.status='failed'; $report.error=$_.ToString()
    throw
} finally {
    Save-LabReport
    if ($Session.State -eq 'Opened') {
        try {
            Invoke-Lab {param($path) if(Test-Path -LiteralPath $path){[IO.File]::WriteAllText((Join-Path $path 'observer.stop'),'stop')}} @($inputRoot)
            if(Invoke-Lab {param($path) Test-Path -LiteralPath $path} @("$inputRoot\observations")) {
                Copy-Item -LiteralPath "$inputRoot\observations" -Destination (Join-Path $proof 'independent-observations') -FromSession $Session -Recurse
            }
        } catch {$report.observerCopyError=$_.ToString();Save-LabReport}
        try {
            $report.guestScriptPolicy=Invoke-Lab {Get-ExecutionPolicy -List | ForEach-Object { @{scope=[string]$_.Scope;policy=[string]$_.ExecutionPolicy} }}
            foreach ($phase in 'prepare','lifecycle') {
                $file="C:\Users\mcw_qa_lab01\AppData\Local\Temp\MCW-Stage4-$runId-$phase.txt"
                if (Invoke-Lab {param($path) Test-Path -LiteralPath $path} @($file)) {Copy-Item -LiteralPath $file -Destination $proof -FromSession $Session}
            }
            $errors=Invoke-Lab {param($path) @(Get-ChildItem -LiteralPath $path -Filter 'task-error-*.txt' -ErrorAction SilentlyContinue | Select-Object -ExpandProperty FullName)} @($inputRoot)
            foreach ($file in $errors) { Copy-Item -LiteralPath $file -Destination $proof -FromSession $Session }
            Save-LabReport
        } catch { $report.diagnosticCopyError=$_.ToString(); Save-LabReport }
        foreach ($name in 'proof','observations','preparation.json','workload-error.txt') {
            try {
                if (Invoke-Lab {param($path) Test-Path -LiteralPath $path} @("$guestRoot\$name")) {
                    Assert-Lab
                    Copy-Item -LiteralPath "$guestRoot\$name" -Destination (Join-Path $proof $name) -FromSession $Session -Recurse
                }
            } catch { $report.evidenceCopyError=$_.ToString(); Save-LabReport }
        }
    }
}

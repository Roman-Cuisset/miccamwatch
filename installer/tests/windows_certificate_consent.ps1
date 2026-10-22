# Temporary hosted-only consent to the exact already-validated local RDP leaf.
# No certificate checks, store protections or Windows security policy are disabled.
param([Parameter(Mandatory=$true)][string]$StageDirectory,[Parameter(Mandatory=$true)][int]$ProviderProcessId)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') { throw 'Disposable hosted runner required' }
$stage=[IO.Path]::GetFullPath($StageDirectory)
if (-not $stage.StartsWith(([IO.Path]::GetFullPath($env:PUBLIC).TrimEnd('\')+'\'),[StringComparison]::OrdinalIgnoreCase)) { throw 'Public disposable stage required' }
Add-Type -AssemblyName UIAutomationClient,UIAutomationTypes,System.Windows.Forms,System.Drawing
$deadline=[DateTime]::UtcNow.AddSeconds(120)
while ([DateTime]::UtcNow -lt $deadline) {
    $reportPath=Join-Path $stage 'interactive-provider.json'
    if (Test-Path -LiteralPath $reportPath) {
        $report=Get-Content -LiteralPath $reportPath -Raw | ConvertFrom-Json
        if ($report.stage -eq 'provision-exact-client-leaf') {
            $thumbprint=[string]$report.server_certificate_thumbprint
            $hostname=[string]$report.server_hostname
            if ($report.trust_scope -cne 'CurrentUser Root exact local RDP leaf only' -or $thumbprint -notmatch '^[A-F0-9]{40}$' -or $hostname -cne [Environment]::MachineName) { throw 'Consent report is outside the exact authorized enrollment scope' }
            $condition=New-Object Windows.Automation.AndCondition @(
                (New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::ProcessIdProperty,$ProviderProcessId)),
                (New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::NameProperty,'Security Warning'))
            )
            $dialog=[Windows.Automation.AutomationElement]::RootElement.FindFirst([Windows.Automation.TreeScope]::Children,$condition)
            if ($null -ne $dialog) {
                $elements=$dialog.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)
                $names=@($elements|ForEach-Object{$_.Current.Name})
                $text=$names -join "`n"
                $names | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $stage 'native-certificate-dialog-text.json') -Encoding UTF8
                $checks=@{
                    fingerprint=(($text -replace '[\s\u200e\u200f]','').ToUpperInvariant().IndexOf($thumbprint,[StringComparison]::Ordinal) -ge 0)
                    hostname=([regex]::IsMatch($text,'(?<![A-Za-z0-9-])'+[regex]::Escape($hostname)+'(?![A-Za-z0-9-])'))
                    enrollment_question=$text.Contains('Do you want to install this certificate?')
                }
                $checks | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $stage 'native-certificate-dialog-checks.json') -Encoding UTF8
                if (-not $checks.fingerprint -or -not $checks.hostname -or -not $checks.enrollment_question) { throw 'Native certificate dialog does not identify the exact validated server leaf; no consent given' }
                $bounds=[Windows.Forms.SystemInformation]::VirtualScreen
                $image=New-Object Drawing.Bitmap($bounds.Width,$bounds.Height)
                $graphics=[Drawing.Graphics]::FromImage($image)
                try { $graphics.CopyFromScreen($bounds.Location,[Drawing.Point]::Empty,$bounds.Size);$image.Save((Join-Path $stage 'exact-certificate-consent.png'),[Drawing.Imaging.ImageFormat]::Png) }
                finally { $graphics.Dispose();$image.Dispose() }
                $yes=$dialog.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::NameProperty,'Yes')))
                if ($null -eq $yes -or $yes.Current.ControlType -ne [Windows.Automation.ControlType]::Button) { throw 'Exact certificate consent button unavailable' }
                $invoke=$yes.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern)
                $invoke.Invoke()
                @{status='exact-leaf-consent-given';provider_pid=$ProviderProcessId;server_hostname=$hostname;thumbprint=$thumbprint;scope='CurrentUser Root only';no_policy_or_validation_disabled=$true} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $stage 'certificate-consent.json') -Encoding UTF8
                exit 0
            }
        }
    }
    Start-Sleep -Milliseconds 100
}
throw 'Exact certificate enrollment dialog was not observed; no consent given'

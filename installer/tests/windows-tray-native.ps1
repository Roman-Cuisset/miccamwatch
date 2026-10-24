param(
    [string]$OutputDirectory = 'artifacts/native-windows-tray',
    [switch]$IsolatedDesktop
)
$ErrorActionPreference = 'Stop'
if (-not $IsolatedDesktop -or $env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
    throw 'Shell tooltip proof is restricted to an explicitly isolated GitHub-hosted Windows runner (-IsolatedDesktop). Never run against an installed user tray.'
}
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class McwNativeProof {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct NOTIFYICONIDENTIFIER { public uint cbSize; public IntPtr hWnd; public uint uID; public Guid guidItem; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(POINT point);
    [StructLayout(LayoutKind.Sequential)] public struct MOUSEINPUT { public int dx, dy; public uint mouseData, dwFlags, time; public UIntPtr extra; }
    [StructLayout(LayoutKind.Sequential)] public struct INPUT { public uint type; public MOUSEINPUT mouse; }
    [DllImport("user32.dll", SetLastError=true)] public static extern uint SendInput(uint count, INPUT[] input, int size);
    [DllImport("user32.dll")] private static extern int GetSystemMetrics(int index);
    public static void MovePointerTo(int x, int y) {
        int left = GetSystemMetrics(76), top = GetSystemMetrics(77);
        int width = GetSystemMetrics(78), height = GetSystemMetrics(79);
        if (width <= 0 || height <= 0 || x < left || x >= (long)left + width ||
            y < top || y >= (long)top + height)
            throw new InvalidOperationException("Native mouse target is outside the virtual desktop.");
        // Inject the entry itself, not SetCursorPos followed by a same-batch
        // right/left jiggle. That path moved/clicked the owned Win32 probe in
        // CI39467142185 but never produced an Explorer overflow hover popup.
        // Normalize to the pixel center on the entire physical virtual desktop.
        var move = new INPUT { type = 0, mouse = new MOUSEINPUT {
            dx = (int)((((long)x - left) * 65536 + 32768) / width),
            dy = (int)((((long)y - top) * 65536 + 32768) / height),
            dwFlags = 0xE001 // MOVE | MOVE_NOCOALESCE | VIRTUALDESK | ABSOLUTE
        } };
        if (SendInput(1, new[] { move }, Marshal.SizeOf(typeof(INPUT))) != 1)
            throw new InvalidOperationException("Native absolute mouse input was rejected.");
    }
    public static void ClickLeft() {
        var down = new INPUT { type = 0, mouse = new MOUSEINPUT { dwFlags = 0x0002 } };
        var up = new INPUT { type = 0, mouse = new MOUSEINPUT { dwFlags = 0x0004 } };
        if (SendInput(2, new[] { down, up }, Marshal.SizeOf(typeof(INPUT))) != 2)
            throw new InvalidOperationException("Native overflow click input was rejected.");
    }
    [DllImport("shell32.dll")] public static extern int Shell_NotifyIconGetRect(ref NOTIFYICONIDENTIFIER icon, out RECT rect);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string cls, string title);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out RECT rect);
    [DllImport("user32.dll")] public static extern bool ScreenToClient(IntPtr hwnd, ref POINT point);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hwnd, IntPtr hdc, uint flags);
    [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr hwnd, uint msg, IntPtr wparam, IntPtr lparam);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd, uint msg, IntPtr wparam, IntPtr lparam);
    [DllImport("user32.dll")] public static extern IntPtr OpenInputDesktop(uint flags, bool inherit, uint access);
    [DllImport("user32.dll")] public static extern bool CloseDesktop(IntPtr desktop);
    [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
    [DllImport("user32.dll")] public static extern IntPtr GetThreadDesktop(uint thread);
    [DllImport("user32.dll", CharSet=CharSet.Unicode, SetLastError=true)] private static extern bool GetUserObjectInformation(IntPtr value, int index, System.Text.StringBuilder text, uint bytes, out uint needed);
    public static string DesktopName(IntPtr desktop) {
        var name = new System.Text.StringBuilder(256);
        uint needed;
        if (!GetUserObjectInformation(desktop, 2, name, 512, out needed))
            throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
        return name.ToString();
    }
    [DllImport("user32.dll", SetLastError=true)] private static extern bool SystemParametersInfo(uint action, uint parameter, out uint value, uint flags);
    public static uint MouseHoverTime() {
        uint value;
        if (!SystemParametersInfo(0x66, 0, out value, 0))
            throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
        return value;
    }
    [DllImport("user32.dll", SetLastError=true)] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
}
'@
if (-not [Environment]::UserInteractive) { throw 'Native proof requires an interactive, unlocked Windows desktop.' }
$desktop = [McwNativeProof]::OpenInputDesktop(0, $false, 1)
if ($desktop -eq [IntPtr]::Zero) { throw 'No accessible input desktop: native geometry/screenshots cannot be verified.' }
try { $inputDesktop = [McwNativeProof]::DesktopName($desktop) }
finally { [void][McwNativeProof]::CloseDesktop($desktop) }
$threadDesktop = [McwNativeProof]::DesktopName([McwNativeProof]::GetThreadDesktop([McwNativeProof]::GetCurrentThreadId()))
$directory = [IO.Path]::GetFullPath((Join-Path $OutputDirectory ((Get-Date -Format 'yyyyMMdd-HHmmss') + '-' + $PID)))
[void][IO.Directory]::CreateDirectory($directory)
# Frozen baseline: CI39467142185/job118781365240, artifact12286652966
# (SHA256 a466841a4bd5e036361925aca2291339c4e4b2eb5385c4d1552a96e6a7b39dda).
# Its tooltip-qualification.json failed all eight cases with zero real popups,
# despite successful owned pointer/click routing. Keep that failure separate
# from this absolute-entry qualification; an accessible icon is never a pass.
# https://github.com/Roman-Cuisset/miccamwatch/actions/runs/39467142185/artifacts/12286652966
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
try {
    $principal = New-Object Security.Principal.WindowsPrincipal -ArgumentList $identity
    @{ elevated=$principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator); interactive=[Environment]::UserInteractive; input_desktop=$inputDesktop; thread_desktop=$threadDesktop; mouse_hover_milliseconds=[McwNativeProof]::MouseHoverTime(); pointer_entry='SendInput absolute virtual-desktop pixel center' } |
        ConvertTo-Json | Set-Content -Encoding UTF8 (Join-Path $directory 'driver-context.json')
} finally { $identity.Dispose() }
if ($inputDesktop -ne $threadDesktop -or $inputDesktop -eq 'Winlogon') {
    throw 'The current thread is not on an unlocked input desktop; no input, desktop switch or unlock attempted.'
}
$previous = $env:MCW_WINDOWS_NATIVE_PROOF_DIR
$env:MCW_WINDOWS_NATIVE_PROOF_DIR = $directory
$process = $null
$fixtureProcess = $null
$fixtureWindow = [IntPtr]::Zero
$previousCursor = [System.Windows.Forms.Cursor]::Position
$shellMeasurements = @()
$tooltipFailures = @()
function Wait-ProofFile([string]$name) {
    $deadline = (Get-Date).AddMinutes(10)
    $path = Join-Path $directory $name
    while (-not (Test-Path $path)) {
        if ($process.HasExited) { throw "Native fixture exited ($($process.ExitCode)); see $directory/cargo-stderr.log and cargo-stdout.log" }
        if ((Get-Date) -gt $deadline) { throw "Native fixture timed out waiting for $name" }
        Start-Sleep -Milliseconds 50
    }
    # The producer writes small JSON atomically from its single UI thread, but
    # the file can be visible before its write completes.
    $deadline = (Get-Date).AddSeconds(10)
    while ($true) {
        try { return (Get-Content -Raw -Encoding UTF8 $path | ConvertFrom-Json) }
        catch { if ((Get-Date) -gt $deadline) { throw }; Start-Sleep -Milliseconds 20 }
    }
}
function Capture-NativeWindow([IntPtr]$window, [string]$name, [switch]$RenderWindow) {
    $rect = New-Object McwNativeProof+RECT
    if (-not [McwNativeProof]::GetWindowRect($window, [ref]$rect)) { throw "Cannot measure $name" }
    $width = $rect.Right - $rect.Left
    $height = $rect.Bottom - $rect.Top
    if ($width -le 0 -or $height -le 0) { throw "Empty native $name rectangle" }
    $bitmap = New-Object System.Drawing.Bitmap -ArgumentList $width, $height
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        if ($RenderWindow) {
            $dc = $graphics.GetHdc()
            try {
                if (-not [McwNativeProof]::PrintWindow($window, $dc, 2)) { throw "Native PrintWindow failed for $name" }
            } finally { $graphics.ReleaseHdc($dc) }
        } else {
            $graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bitmap.Size)
        }
        $bitmap.Save((Join-Path $directory "$name.png"), [System.Drawing.Imaging.ImageFormat]::Png)
        $first = $bitmap.GetPixel(0, 0).ToArgb()
        $different = $false
        for ($x = 0; $x -lt $width; $x += 7) {
            for ($y = 0; $y -lt $height; $y += 7) {
                if ($bitmap.GetPixel($x, $y).ToArgb() -ne $first) { $different = $true; break }
            }
            if ($different) { break }
        }
        if (-not $different) { throw "Blank native screenshot for $name; desktop is unavailable/locked." }
    } finally { $graphics.Dispose(); $bitmap.Dispose() }
    return @{ left=$rect.Left; top=$rect.Top; right=$rect.Right; bottom=$rect.Bottom; width=$width; height=$height }
}
function Capture-NativeDesktop([string]$name) {
    $screen = [System.Windows.Forms.SystemInformation]::VirtualScreen
    $frame = New-Object System.Drawing.Bitmap -ArgumentList $screen.Width, $screen.Height
    $graphics = [System.Drawing.Graphics]::FromImage($frame)
    try {
        $graphics.CopyFromScreen($screen.Left, $screen.Top, 0, 0, $frame.Size)
        $frame.Save((Join-Path $directory "$name.png"), [System.Drawing.Imaging.ImageFormat]::Png)
    } finally { $graphics.Dispose(); $frame.Dispose() }
}
function Get-ShellWindows {
    # Microsoft UIA guidance: enumerate desktop Children, never its entire
    # Descendants tree (hosted terminals contain thousands of unrelated nodes).
    foreach ($hostProcess in @(Get-Process -Name explorer,ShellExperienceHost -ErrorAction SilentlyContinue)) {
        $condition = New-Object System.Windows.Automation.PropertyCondition -ArgumentList ([System.Windows.Automation.AutomationElement]::ProcessIdProperty), ([int]$hostProcess.Id)
        foreach ($window in [System.Windows.Automation.AutomationElement]::RootElement.FindAll([System.Windows.Automation.TreeScope]::Children, $condition)) {
            $window
        }
    }
}
function Get-FixtureShellButton([string]$expected, $identity) {
    $owned = New-Object McwNativeProof+RECT
    $registered = [McwNativeProof]::Shell_NotifyIconGetRect([ref]$identity, [ref]$owned) -eq 0
    $condition = New-Object System.Windows.Automation.PropertyCondition -ArgumentList ([System.Windows.Automation.AutomationElement]::ControlTypeProperty), ([System.Windows.Automation.ControlType]::Button)
    $buttons = @(foreach ($window in @(Get-ShellWindows)) {
        foreach ($element in $window.FindAll([System.Windows.Automation.TreeScope]::Subtree, $condition)) { $element }
    })
    $matches = @()
    foreach ($button in $buttons) {
        if ($button.Current.Name.StartsWith('miccamwatch', [StringComparison]::Ordinal)) {
            $hostProcess = Get-Process -Id $button.Current.ProcessId -ErrorAction SilentlyContinue
            $bounds = $button.Current.BoundingRectangle
            if ($registered -and $hostProcess -and $hostProcess.ProcessName -eq 'explorer' -and
                $bounds.Contains((($owned.Left + $owned.Right) / 2), (($owned.Top + $owned.Bottom) / 2))) {
                $matches += $button
            }
        }
    }
    if ($matches.Count -gt 1) { throw 'More than one miccamwatch shell button exists on the isolated runner.' }
    if ($matches.Count -eq 1) {
        $name = $matches[0].Current.Name
        # Shell may retain NIM_ADD's accessible identity even after NIM_MODIFY.
        # A neutral product identity is fine; an old idle/status assertion is not.
        # Explorer also composes the original neutral title and the current tip.
        $composed = 'miccamwatch ' + $expected
        if ($name -ne 'miccamwatch' -and
            -not [string]::Equals($name, $expected, [StringComparison]::Ordinal) -and
            -not [string]::Equals($name, $composed, [StringComparison]::Ordinal)) {
            throw "Shell accessible identity contradicts current fixture status: $name (expected current status, optionally prefixed by neutral product name)"
        }
        # UIA can expose the hidden overflow contents with nonempty off-screen
        # bounds. Do not hover a stale/hidden rectangle; open overflow first.
        $bounds = $matches[0].Current.BoundingRectangle
        $screen = [System.Windows.Forms.SystemInformation]::VirtualScreen
        if (-not $matches[0].Current.IsOffscreen -and -not $bounds.IsEmpty -and
            $bounds.Right -gt $screen.Left -and $bounds.Left -lt $screen.Right -and
            $bounds.Bottom -gt $screen.Top -and $bounds.Top -lt $screen.Bottom) {
            return $matches[0]
        }
    }
    # Hosted English Windows images may hide newly added notification icons.
    # Invoke only Explorer's overflow button, never a product/control command.
    foreach ($button in $buttons) {
        if ($button.Current.Name -in @('Show hidden icons', 'Show hidden icons.', 'Hidden icons menu')) {
            $hostProcess = Get-Process -Id $button.Current.ProcessId -ErrorAction SilentlyContinue
            if ($hostProcess -and $hostProcess.ProcessName -eq 'explorer') {
                $bounds = $button.Current.BoundingRectangle
                if ($button.Current.IsOffscreen -or $bounds.IsEmpty -or $bounds.Width -le 0 -or $bounds.Height -le 0) { continue }
                # Open the actual flyout through pointer input so Explorer
                # enters its native mouse/focus path, not UIA keyboard mode.
                [McwNativeProof]::MovePointerTo([int]($bounds.X + $bounds.Width / 2), [int]($bounds.Y + $bounds.Height / 2))
                [McwNativeProof]::ClickLeft()
                return $null
            }
        }
    }
    return $null
}
function Confirm-FixtureOwner([IntPtr]$window) {
    [uint32]$ownerPid = 0
    [void][McwNativeProof]::GetWindowThreadProcessId($window, [ref]$ownerPid)
    if ($ownerPid -eq 0) { throw 'Fixture owner is no longer alive.' }
    if ($null -eq $script:fixtureProcess) {
        $candidate = Get-Process -Id $ownerPid -ErrorAction Stop
        $null = $candidate.Handle
        [uint32]$confirmedPid = 0
        [void][McwNativeProof]::GetWindowThreadProcessId($window, [ref]$confirmedPid)
        if ($confirmedPid -ne $ownerPid -or $candidate.HasExited) {
            $candidate.Dispose()
            throw 'Fixture owner changed while retaining its process handle.'
        }
        $script:fixtureProcess = $candidate
    } elseif ($script:fixtureProcess.Id -ne $ownerPid -or $script:fixtureProcess.HasExited) {
        throw 'Fixture owner changed between observations.'
    }
    $script:fixtureWindow = $window
}
function Confirm-FixturePointerRouting {
    $probe = Wait-ProofFile 'pointer-probe.json'
    $window = [IntPtr][long]$probe.owner
    Confirm-FixtureOwner $window
    $rect = New-Object McwNativeProof+RECT
    if (-not [McwNativeProof]::GetWindowRect($window, [ref]$rect)) { throw 'Owned pointer probe has no rectangle.' }
    [McwNativeProof]::MovePointerTo(0, 0)
    Start-Sleep -Milliseconds 200
    [McwNativeProof]::MovePointerTo([int](($rect.Left + $rect.Right) / 2), [int](($rect.Top + $rect.Bottom) / 2))
    Start-Sleep -Milliseconds 1000
    Capture-NativeDesktop 'pointer-probe-desktop'
    $point = New-Object McwNativeProof+POINT
    $point.X = [System.Windows.Forms.Cursor]::Position.X
    $point.Y = [System.Windows.Forms.Cursor]::Position.Y
    $hit = [McwNativeProof]::WindowFromPoint($point)
    if ($hit -ne $window) { throw 'Pointer probe is not the actual window under the cursor; refusing an unrelated input target.' }
    [McwNativeProof]::ClickLeft()
    Start-Sleep -Milliseconds 250
    if (-not [McwNativeProof]::ScreenToClient($window, [ref]$point)) { throw 'Cannot measure pointer coordinates in owned probe.' }
    [IO.File]::WriteAllText((Join-Path $directory 'pointer-probe-inspect'), 'collect actual WM_MOUSEMOVE')
    $observed = Wait-ProofFile 'pointer-probe-observed.json'
    $script:ownedPointerRouted = $observed.move_count -gt $probe.before_move_count -and $observed.x -eq $point.X -and $observed.y -eq $point.Y
    $script:ownedClickRouted = $observed.click_count -gt $probe.before_click_count
    @{ routed=$script:ownedPointerRouted; click_routed=$script:ownedClickRouted; observed=$observed; expected_client_point=@{ x=$point.X; y=$point.Y }; owned_window=$window.ToInt64() } |
        ConvertTo-Json -Depth 6 | Set-Content -Encoding UTF8 (Join-Path $directory 'pointer-routing.json')
    [McwNativeProof]::MovePointerTo(0, 0)
}
function Get-FixtureHoverTarget([string]$expected, $identity) {
    try {
        $button = Get-FixtureShellButton $expected $identity
        if (-not $button -or $button.Current.IsOffscreen) { return $null }
        $rect = New-Object McwNativeProof+RECT
        if ([McwNativeProof]::Shell_NotifyIconGetRect([ref]$identity, [ref]$rect) -ne 0) {
            throw 'The owned notification icon disappeared while acquiring its hover target.'
        }
        $bounds = $button.Current.BoundingRectangle
        $x = [int](($rect.Left + $rect.Right) / 2)
        $y = [int](($rect.Top + $rect.Bottom) / 2)
        if ($bounds.IsEmpty -or -not $bounds.Contains($x, $y) -or
            -not [System.Windows.Forms.SystemInformation]::VirtualScreen.Contains($x, $y)) { return $null }
        return @{ button=$button; bounds=$bounds; rect=$rect; x=$x; y=$y }
    } catch [System.Windows.Automation.ElementNotAvailableException] {
        # Explorer may move an icon between overflow and the visible taskbar.
        return $null
    }
}
function Confirm-FixtureShellTooltip([string]$name) {
    $shell = Wait-ProofFile "$name-shell.json"
    Confirm-FixtureOwner ([IntPtr][long]$shell.owner)
    $identity = New-Object McwNativeProof+NOTIFYICONIDENTIFIER
    $identity.cbSize = [Runtime.InteropServices.Marshal]::SizeOf($identity)
    $identity.hWnd = [IntPtr][long]$shell.owner
    $identity.uID = $shell.id
    $rect = New-Object McwNativeProof+RECT
    if ([McwNativeProof]::Shell_NotifyIconGetRect([ref]$identity, [ref]$rect) -ne 0) {
        throw 'Explorer did not register the actual fixture notification icon.'
    }
    if ($shell.capacity -ne 128 -or $shell.utf16_units -gt 127) { throw 'Invalid native tooltip capacity.' }
    # Leave the previous hover before locating the visible icon. Leaving an
    # overflow flyout after lookup can dismiss it and invalidate its bounds.
    [McwNativeProof]::MovePointerTo(0, 0)
    Start-Sleep -Milliseconds 200
    $deadline = (Get-Date).AddSeconds(30)
    $target = $null
    while (-not $target -and (Get-Date) -lt $deadline) {
        $target = Get-FixtureHoverTarget $shell.tooltip $identity
        if (-not $target) { Start-Sleep -Milliseconds 200 }
    }
    if (-not $target) { throw 'Fixture shell icon is not UIA-accessible, including overflow. Requires an unlocked English hosted Explorer desktop.' }
    $button = $target.button
    $bounds = $target.bounds
    $rect = $target.rect
    $hoverX = $target.x
    $hoverY = $target.y
    $hoverTargets = @(@{ x=$hoverX; y=$hoverY })
    [McwNativeProof]::MovePointerTo($hoverX, $hoverY)
    Start-Sleep -Milliseconds 1000
    Capture-NativeDesktop "$name-shell-hover"
    [IO.File]::WriteAllText((Join-Path $directory "$name-shell-inspect"), 'inspect native callbacks')
    $inputEvidence = Wait-ProofFile "$name-shell-input.json"
    $hitPoint = New-Object McwNativeProof+POINT
    $hitPoint.X = $hoverX
    $hitPoint.Y = $hoverY
    $hitWindow = [McwNativeProof]::WindowFromPoint($hitPoint)
    [uint32]$hitPid = 0
    [void][McwNativeProof]::GetWindowThreadProcessId($hitWindow, [ref]$hitPid)
    $hitProcess = Get-Process -Id $hitPid -ErrorAction SilentlyContinue
    $inputEvidence | Add-Member -NotePropertyName hit_window -NotePropertyValue $hitWindow.ToInt64()
    $inputEvidence | Add-Member -NotePropertyName hit_process -NotePropertyValue $hitProcess.ProcessName
    $registered = [McwNativeProof]::Shell_NotifyIconGetRect([ref]$identity, [ref]$rect) -eq 0
    $inputEvidence | Add-Member -NotePropertyName owned_rect -NotePropertyValue @{
        registered=$registered; left=$rect.Left; top=$rect.Top; right=$rect.Right; bottom=$rect.Bottom
    }
    $condition = New-Object System.Windows.Automation.PropertyCondition -ArgumentList ([System.Windows.Automation.AutomationElement]::NameProperty), ([string]$shell.tooltip)
    $deadline = (Get-Date).AddSeconds(20)
    $observed = @()
    $tooltip = $null
    $tipWindow = [IntPtr]::Zero
    while (-not $tooltip -and (Get-Date) -lt $deadline) {
        # A real NIM_MODIFY/hover can relocate the icon out of overflow.
        # Follow only a fresh UIA target containing the same owned native icon;
        # keep the original deadline and require the actual tooltip popup.
        $fresh = Get-FixtureHoverTarget $shell.tooltip $identity
        if (-not $fresh) { Start-Sleep -Milliseconds 100; continue }
        $button = $fresh.button
        $bounds = $fresh.bounds
        $rect = $fresh.rect
        if ($fresh.x -ne $hoverX -or $fresh.y -ne $hoverY -or
            [System.Windows.Forms.Cursor]::Position.X -ne $fresh.x -or
            [System.Windows.Forms.Cursor]::Position.Y -ne $fresh.y) {
            $hoverX = $fresh.x
            $hoverY = $fresh.y
            $hoverTargets += @{ x=$hoverX; y=$hoverY }
            [McwNativeProof]::MovePointerTo($hoverX, $hoverY)
        }
        foreach ($window in @(Get-ShellWindows)) {
            try {
                if ($window.Current.IsOffscreen) { continue }
                $tips = $window.FindAll([System.Windows.Automation.TreeScope]::Subtree, $condition)
                foreach ($tip in $tips) {
                    $text = $tip.Current.Name
                    $observed += $text
                    if (-not $tip.Current.IsOffscreen -and
                        $tip.Current.ControlType -ne [System.Windows.Automation.ControlType]::Button -and
                        [string]::Equals($text, $shell.tooltip, [StringComparison]::Ordinal)) {
                        # Win32 and XAML may expose different UIA roles/parents.
                        # Require a real popup HWND, never the icon host's label.
                        $ancestor = $tip
                        $nativeWindow = [IntPtr]::Zero
                        for ($depth = 0; $ancestor -and $depth -lt 16; $depth++) {
                            $nativeWindow = [IntPtr]$ancestor.Current.NativeWindowHandle
                            if ($nativeWindow -ne [IntPtr]::Zero) { break }
                            $ancestor = [System.Windows.Automation.TreeWalker]::RawViewWalker.GetParent($ancestor)
                        }
                        if ($nativeWindow -eq [IntPtr]::Zero) { continue }
                        $popup = New-Object McwNativeProof+RECT
                        if (-not [McwNativeProof]::GetWindowRect($nativeWindow, [ref]$popup)) { continue }
                        $x = $hoverX
                        $y = $hoverY
                        if ($x -ge $popup.Left -and $x -lt $popup.Right -and
                            $y -ge $popup.Top -and $y -lt $popup.Bottom) { continue }
                        $tooltip = $tip
                        $tipWindow = $nativeWindow
                        break
                    }
                }
                if ($tooltip) { break }
            } catch [System.Windows.Automation.ElementNotAvailableException] {
                # Hover popups are transient; reacquire fresh Shell roots.
            }
        }
        if (-not $tooltip) { Start-Sleep -Milliseconds 100 }
    }
    if (-not $tooltip) {
        $diagnostic = @{
            expected=$shell.tooltip; accessible_name=$button.Current.Name;
            button_offscreen=$button.Current.IsOffscreen;
            button=@{left=$bounds.Left; top=$bounds.Top; right=$bounds.Right; bottom=$bounds.Bottom};
            cursor=@{x=[System.Windows.Forms.Cursor]::Position.X; y=[System.Windows.Forms.Cursor]::Position.Y};
            native_input=$inputEvidence;
            hover_targets=$hoverTargets;
            observed=@($observed | Select-Object -Unique)
        }
        ConvertTo-Json -InputObject $diagnostic -Depth 6 | Set-Content -Encoding UTF8 (Join-Path $directory "$name-shell-observed.json")
        Capture-NativeDesktop "$name-shell-desktop"
        # Diagnostic only: a native click on this exact inert fixture icon.
        # It is not a substitute for a tooltip or permission to control devices.
        Confirm-FixtureOwner ([IntPtr][long]$shell.owner)
        $cursorPoint = New-Object McwNativeProof+POINT
        $cursorPoint.X = [System.Windows.Forms.Cursor]::Position.X
        $cursorPoint.Y = [System.Windows.Forms.Cursor]::Position.Y
        if ([McwNativeProof]::Shell_NotifyIconGetRect([ref]$identity, [ref]$rect) -ne 0 -or
            $cursorPoint.X -lt $rect.Left -or $cursorPoint.X -ge $rect.Right -or
            $cursorPoint.Y -lt $rect.Top -or $cursorPoint.Y -ge $rect.Bottom) {
            throw 'Native diagnostic click no longer targets the owned icon.'
        }
        [McwNativeProof]::ClickLeft()
        Start-Sleep -Milliseconds 250
        [IO.File]::WriteAllText((Join-Path $directory "$name-shell-click-inspect"), 'inspect callback after native click')
        $null = Wait-ProofFile "$name-shell-click.json"
        $script:tooltipFailures += $name
        [McwNativeProof]::MovePointerTo(0, 0)
        [IO.File]::WriteAllText((Join-Path $directory "$name-shell-inspected"), 'failed: actual popup not observed')
        return
    }
    if ($tipWindow -eq [IntPtr]::Zero) { throw 'Explorer tooltip has no native window for screenshot proof.' }
    $tipBounds = Capture-NativeWindow $tipWindow "$name-shell-tooltip"
    $script:shellMeasurements += @{
        case=$name; accessible_name=$button.Current.Name; tooltip=$tooltip.Current.Name;
        tooltip_bounds=$tipBounds; native_utf16_units=$shell.utf16_units;
        native_input=$inputEvidence;
        hover_targets=$hoverTargets;
        shell_rect=@{left=$rect.Left; top=$rect.Top; right=$rect.Right; bottom=$rect.Bottom}
    }
    [McwNativeProof]::MovePointerTo(0, 0)
    [IO.File]::WriteAllText((Join-Path $directory "$name-shell-inspected"), 'passed: actual Explorer tooltip and screenshot')
}
try {
    # GetWindowRect must return physical pixels, matching CopyFromScreen.
    # A DPI-unaware PowerShell host otherwise crops wallpaper at scaled offsets.
    $previousDpi = [McwNativeProof]::SetThreadDpiAwarenessContext([IntPtr](-4))
    if ($previousDpi -eq [IntPtr]::Zero) { throw 'Cannot establish per-monitor-aware native measurement coordinates.' }
    $process = Start-Process cargo -ArgumentList @('test', '--locked', '--features', 'windows-tray', 'frontends::tray::tests::native_windows_tray_visual_smoke', '--', '--ignored', '--exact', '--nocapture', '--test-threads=1') -PassThru -NoNewWindow -RedirectStandardOutput (Join-Path $directory 'cargo-stdout.log') -RedirectStandardError (Join-Path $directory 'cargo-stderr.log')
    # Cache the native process handle while it is alive. Windows PowerShell
    # otherwise can report a null ExitCode for a completed Start-Process.
    $null = $process.Handle
    $measurements = @()
    Confirm-FixturePointerRouting
    Confirm-FixtureShellTooltip 'startup'
    foreach ($name in @('camera-guard-missing', 'partial-active', 'error-active', 'recovered-active', 'long-error', 'unicode-controls', 'idle-protected')) {
        Confirm-FixtureShellTooltip $name
        $menu = Wait-ProofFile "$name-menu.json"
        $owner = [IntPtr][long]$menu.owner
        [uint32]$ownerPid = 0
        $ownerThread = [McwNativeProof]::GetWindowThreadProcessId($owner, [ref]$ownerPid)
        # Explorer may return the overflow-chevron rectangle for a hidden icon.
        # Rasterize the actual native HICON; do not mislabel the chevron as our glyph.
        $nativeIcon = [System.Drawing.Icon]::FromHandle([IntPtr][long]$menu.icon)
        $icon = $nativeIcon.ToBitmap()
        try {
            $icon.Save((Join-Path $directory "$name-native-icon.png"), [System.Drawing.Imaging.ImageFormat]::Png)
        } finally { $icon.Dispose() }
        $deadline = (Get-Date).AddSeconds(30)
        $popup = [IntPtr]::Zero
        while ((Get-Date) -lt $deadline) {
            $candidate = [McwNativeProof]::FindWindow('#32768', $null)
            [uint32]$popupPid = 0
            $popupThread = [McwNativeProof]::GetWindowThreadProcessId($candidate, [ref]$popupPid)
            if ($candidate -ne [IntPtr]::Zero -and $popupThread -eq $ownerThread -and $popupPid -eq $ownerPid) { $popup = $candidate; break }
            Start-Sleep -Milliseconds 20
        }
        if ($popup -eq [IntPtr]::Zero) { throw 'Real Windows popup did not appear on the input desktop.' }
        Start-Sleep -Milliseconds 300
        $bounds = Capture-NativeWindow $popup "$name-menu"
        $limit = [Math]::Min(400 * [double]$menu.dpi / 96, [double]$menu.work_width * 0.75)
        if ($bounds.width -gt $limit) { throw "Native popup width $($bounds.width) exceeds compact bound $limit" }
        [void][McwNativeProof]::PostMessage($owner, 0x001f, [IntPtr]::Zero, [IntPtr]::Zero) # WM_CANCELMODE; no privacy action
        $measurements += @{ case=$name; menu=$bounds; menu_width_limit=$limit; text_width_limit=$menu.text_width_limit; dpi=$menu.dpi }
    }
    if (-not $process.WaitForExit(30000)) { throw 'Native shell/menu fixture did not finish after dismissing menus.' }
    if ($process.ExitCode -ne 0 -or -not (Test-Path (Join-Path $directory 'complete'))) { throw "Native fixture failed ($($process.ExitCode)); see fixture logs." }
    $measurements | ConvertTo-Json -Depth 8 | Set-Content -Encoding UTF8 (Join-Path $directory 'measurements.json')
    ConvertTo-Json -InputObject @($shellMeasurements) -Depth 8 | Set-Content -Encoding UTF8 (Join-Path $directory 'shell-measurements.json')
    @{ passed=$tooltipFailures.Count -eq 0; failed_cases=@($tooltipFailures); observed_popup_count=$shellMeasurements.Count; owned_pointer_routed=$script:ownedPointerRouted; owned_click_routed=$script:ownedClickRouted } |
        ConvertTo-Json -Depth 6 | Set-Content -Encoding UTF8 (Join-Path $directory 'tooltip-qualification.json')
    if ($tooltipFailures.Count -ne 0) {
        throw "Actual Explorer popup qualification failed for: $($tooltipFailures -join ', '). All reachable menu cases were still exercised; see tooltip-qualification.json."
    }
    Write-Output "Verified actual Explorer tooltip/UIA updates and native popup bounds; production desktop has its own windows-desktop-native.ps1 proof: $directory"
} finally {
    $env:MCW_WINDOWS_NATIVE_PROOF_DIR = $previous
    [System.Windows.Forms.Cursor]::Position = $previousCursor
    if ($fixtureProcess) {
        if (-not $fixtureProcess.HasExited) {
            # Close only HWNDs already obtained from this retained fixture.
            # Never kill the process or target a product/SDK process by name.
            $windows = @($fixtureWindow)
            foreach ($window in $windows) {
                [uint32]$windowPid = 0
                [void][McwNativeProof]::GetWindowThreadProcessId($window, [ref]$windowPid)
                if ($window -ne [IntPtr]::Zero -and $windowPid -eq $fixtureProcess.Id -and -not $fixtureProcess.HasExited) {
                    [void][McwNativeProof]::PostMessage($window, 0x001f, [IntPtr]::Zero, [IntPtr]::Zero) # WM_CANCELMODE
                    [void][McwNativeProof]::PostMessage($window, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) # WM_CLOSE
                }
            }
            if (-not $fixtureProcess.WaitForExit(5000)) {
                Write-Warning "Owned fixture PID $($fixtureProcess.Id) remains alive after bounded WM_CLOSE cleanup; preserving it and failed evidence at $directory. No force-kill attempted."
            }
        }
        $fixtureProcess.Dispose()
    }
    if ($process) {
        if (-not $process.HasExited -and -not $process.WaitForExit(5000)) {
            Write-Warning "Cargo PID $($process.Id) remains alive after bounded cleanup; preserving it and failed evidence at $directory. No force-kill attempted."
        }
        $process.Dispose()
    }
    if ($previousDpi -and $previousDpi -ne [IntPtr]::Zero) {
        [void][McwNativeProof]::SetThreadDpiAwarenessContext($previousDpi)
    }
}

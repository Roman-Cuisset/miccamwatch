$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'windows-test-image.ps1')
$cases=@(
    @('debug\deps\miccamwatch-ab12.exe',$true),
    @('release/deps/miccamwatch-ab12.exe',$true),
    @('debug\build\miccamwatch\ab12\out\miccamwatch-ab12.exe',$true),
    @('release/build/miccamwatch/ab12/out/miccamwatch-ab12.exe',$true),
    @('debug\build\miccamwatch\ab12\out\miccamwatch-cd34.exe',$false),
    @('debug\build\other\ab12\out\miccamwatch-ab12.exe',$false),
    @('debug\deps\mcw.exe',$false),
    @('debug\deps\mcw-tray.exe',$false),
    @('debug\deps\miccamwatch-ab12.dll',$false),
    @('..\debug\deps\miccamwatch-ab12.exe',$false),
    @('debug\deps\miccamwatch-ab12.exe\child.exe',$false)
)
foreach($case in $cases) {
    if((Test-McwCargoTestRelativePath $case[0]) -ne $case[1]){throw "Unexpected image admission: $($case[0])"}
}
try {
    $null=Assert-McwRepositoryTestImage 'C:\Program Files\MicCamWatch\mcw.exe'
    throw 'An external installed image was admitted.'
} catch {
    if($_.ToString() -notlike '*Only an admitted repository library-test image*'){throw}
}
"Native qualification image admission: $($cases.Count + 1) cases passed."

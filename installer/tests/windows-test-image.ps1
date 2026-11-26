# Shared admission for an owned repository library-test image. No process is
# launched here; callers independently retain and verify PID/birth/window owner.
function Test-McwCargoTestRelativePath([string]$Relative) {
    $value=$Relative.Replace('/','\')
    return $value -match '^(?:debug|release)\\deps\\miccamwatch-[0-9a-f]+\.exe$' -or
        $value -match '^(?:debug|release)\\build\\miccamwatch\\(?<hash>[0-9a-f]+)\\out\\miccamwatch-\k<hash>\.exe$'
}
function Assert-McwRepositoryTestImage([string]$Executable) {
    $image=[IO.Path]::GetFullPath($Executable)
    $target=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../target')) + [IO.Path]::DirectorySeparatorChar
    if (!$image.StartsWith($target,[StringComparison]::OrdinalIgnoreCase) -or
        !(Test-McwCargoTestRelativePath $image.Substring($target.Length))) {
        throw 'Only an admitted repository library-test image can grant native qualification.'
    }
    $item=Get-Item -LiteralPath $image -Force
    if($item.PSIsContainer){throw 'Cargo test artifact must be a regular file.'}
    while($null -ne $item) {
        if($item.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Refusing a redirected Cargo test path.'}
        if($item.FullName.TrimEnd('\') -ieq $target.TrimEnd('\')){break}
        $item=if($item -is [IO.FileInfo]){$item.Directory}else{$item.Parent}
    }
    return $image
}

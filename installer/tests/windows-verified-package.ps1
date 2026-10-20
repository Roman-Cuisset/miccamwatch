function Get-VerifiedWindowsPackage(
    [string]$Repository,
    [string]$Tag,
    [string]$SourceDigest,
    [string]$Directory
) {
    if ($Tag -cnotmatch '^v[0-9]+\.[0-9]+\.[0-9]+$' -or $SourceDigest -cnotmatch '^[0-9a-f]{40}$') {
        throw 'Expected explicit stable tag and full source digest'
    }
    [void](New-Item -ItemType Directory $Directory)
    $releaseJson = gh api "repos/$Repository/releases/tags/$Tag"
    if ($LASTEXITCODE -ne 0) { throw "Cannot read release $Tag" }
    $release = $releaseJson | ConvertFrom-Json
    if ($release.draft -or $release.prerelease -or $release.tag_name -ne $Tag) {
        throw 'Expected immutable stable release'
    }
    $refJson = gh api "repos/$Repository/git/ref/tags/$Tag"
    if ($LASTEXITCODE -ne 0) { throw 'Cannot resolve tag' }
    $target = ($refJson | ConvertFrom-Json).object
    if ($target.type -eq 'tag') {
        $tagJson = gh api "repos/$Repository/git/tags/$($target.sha)"
        if ($LASTEXITCODE -ne 0) { throw 'Cannot resolve annotated tag' }
        $target = ($tagJson | ConvertFrom-Json).object
    }
    if ($target.type -ne 'commit' -or $target.sha -ne $SourceDigest) {
        throw 'Tag source differs from qualified source'
    }
    gh release download $Tag --repo $Repository --pattern miccamwatch-windows-x86_64.zip --pattern SHA256SUMS --dir $Directory | Write-Host
    if ($LASTEXITCODE -ne 0) { throw 'Public download failed' }
    foreach ($name in @('miccamwatch-windows-x86_64.zip', 'SHA256SUMS')) {
        $file = Get-Item (Join-Path $Directory $name)
        $asset = @($release.assets | Where-Object name -eq $name)
        $hash = (Get-FileHash $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($asset.Count -ne 1 -or $asset[0].size -ne $file.Length -or $asset[0].digest -ne "sha256:$hash") {
            throw "API digest/size mismatch: $name"
        }
    }
    $zip = Join-Path $Directory 'miccamwatch-windows-x86_64.zip'
    $digest = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLowerInvariant()
    $manifest = @(Get-Content (Join-Path $Directory 'SHA256SUMS') | Where-Object { $_ -cmatch '^([a-f0-9]{64})  miccamwatch-windows-x86_64\.zip$' })
    if ($manifest.Count -ne 1 -or $manifest[0].Substring(0,64) -cne $digest) {
        throw 'Manifest ZIP digest mismatch'
    }
    gh attestation verify $zip --repo $Repository --signer-workflow "$Repository/.github/workflows/release.yml" --source-ref "refs/tags/$Tag" --source-digest $SourceDigest --deny-self-hosted-runners | Write-Host
    if ($LASTEXITCODE -ne 0) { throw 'Exact release provenance verification failed' }
    $entries = [IO.Compression.ZipFile]::OpenRead($zip)
    try {
        $names = @($entries.Entries | ForEach-Object FullName | Sort-Object)
        $expected = @('LICENSE','README.md','mcw-tray.exe','mcw.exe')
        # Historical immutable releases precede the portable ownership receipt.
        # Only this one exact additional data entry is allowed in new packages.
        if ($names -ccontains '.miccamwatch-portable.json') { $expected += '.miccamwatch-portable.json' }
        $expected = $expected | Sort-Object
        if (Compare-Object $names $expected -CaseSensitive) {
            throw 'Unexpected ZIP entry set; refusing extraction'
        }
    } finally { $entries.Dispose() }
    $payload = Join-Path $Directory 'payload'
    Expand-Archive $zip $payload
    return $payload
}

function Assert-VerifiedWindowsPair([string]$Installed, [string]$Reference) {
    foreach ($name in @('mcw.exe','mcw-tray.exe')) {
        if ((Get-FileHash (Join-Path $Installed $name)).Hash -ne (Get-FileHash (Join-Path $Reference $name)).Hash) {
            throw "Installed pair differs from verified public payload: $name"
        }
    }
}

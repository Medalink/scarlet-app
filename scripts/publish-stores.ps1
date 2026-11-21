#Requires -Version 7
<#
.SYNOPSIS
Uploads the addon zip of a published Scarlet release to CurseForge and Wago.

.DESCRIPTION
  publish-stores.ps1 -Version 1.0.0            upload Scarlet-1.0.0.zip from the release v1.0.0 of Medalink/scarlet-app
  publish-stores.ps1 -Version 1.0.0 -DryRun    print each store's metadata JSON and upload nothing

release.ps1 runs this after publishing a release (not a beta unless -Stores yes). It can also run alone for any
published version. The zip is downloaded from the GitHub release and checked against SHA256SUMS.txt, so what the
stores get is what the app installs.

  Project IDs   `## X-Curse-Project-ID:` and `## X-Wago-ID:` in addon/Scarlet/Scarlet.toc on main of the addon repo
                (the zip never carries them). -CurseProjectId and -WagoId override them.
  Tokens        CF_API_KEY / WAGO_API_TOKEN, else <SecretsDir>\curseforge-token / wago-token. Never printed.
  Game version  The TOC's `## Interface:` (16001 is 1.60.1). CurseForge's version id is looked up with the token
                at upload time; Wago's patch is checked against https://addons.wago.io/api/data/game.
  Release type  v<x> is a release (Wago: stable), v<x>-beta.N a beta.
  Changelog     The "Addon changes" section of the release notes, or the file given to -Changelog.
  Skipping      A store with no ID or no token is skipped with one line. A version already uploaded to a store
                from this PC (recorded in <Root>\stores.json) is skipped; neither store has an API that lists
                files. -Force uploads it again.

See docs/RELEASING.md.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidatePattern('^v?\d+\.\d+\.\d+(-beta\.\d+)?$')][string]$Version,
    [switch]$DryRun,
    [switch]$Force,
    [ValidateSet('curseforge', 'wago')][string[]]$Store = @('curseforge', 'wago'),
    [string]$Changelog,
    [string]$CurseProjectId,
    [string]$WagoId,
    [string]$GameVersion,
    [string]$TocPath,
    [string]$AppRepo = 'Medalink/scarlet-app',
    [string]$AddonRepo = 'Medalink/scarlet',
    [string]$Root = 'E:\Dev\scratch\scarlet-release',
    [string]$SecretsDir = 'E:\Dev\secrets\scarlet-addon-stores',
    [string]$CurseSite = 'https://wow.curseforge.com',
    [int]$CurseForeverTypeId = 88568
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$Version = $Version.TrimStart('v')
$Tag = "v$Version"
$ZipName = "Scarlet-$Version.zip"
$IsBeta = $Version -like '*-beta.*'
$LedgerPath = Join-Path $Root 'stores.json'
$StoreNames = @{ curseforge = 'CurseForge'; wago = 'Wago' }

function Invoke-Tool([string]$Exe, [string[]]$Arguments) {
    $errPath = (New-TemporaryFile).FullName
    try {
        $out = & $Exe @Arguments 2>$errPath
        if ($LASTEXITCODE -ne 0) {
            $why = (Get-Content -Raw -LiteralPath $errPath)
            throw "$Exe $($Arguments -join ' ') exited $LASTEXITCODE`: $($why ?? '')".Trim()
        }
        return , @($out)
    }
    finally { Remove-Item -LiteralPath $errPath -Force -ErrorAction SilentlyContinue }
}

function Read-StoreToken([string]$EnvName, [string]$FileName) {
    $fromEnv = [Environment]::GetEnvironmentVariable($EnvName)
    if ($fromEnv -and $fromEnv.Trim()) { return [pscustomobject]@{ Value = $fromEnv.Trim(); Source = "`$env:$EnvName" } }
    $path = Join-Path $SecretsDir $FileName
    if (Test-Path -LiteralPath $path) {
        $text = (Get-Content -Raw -LiteralPath $path)
        if ($text -and $text.Trim()) { return [pscustomobject]@{ Value = $text.Trim(); Source = $path } }
    }
    return $null
}

function Get-TocText {
    if ($TocPath) { return Get-Content -Raw -LiteralPath $TocPath }
    $lines = Invoke-Tool 'gh' @('api', "repos/$AddonRepo/contents/addon/Scarlet/Scarlet.toc", '-H', 'Accept: application/vnd.github.raw')
    return ($lines -join "`n")
}

function Get-TocValue([string]$Toc, [string]$Key) {
    $m = [regex]::Match($Toc, "(?mi)^##[ \t]*$([regex]::Escape($Key))[ \t]*:[ \t]*(\S+)[ \t]*\r?$")
    if ($m.Success) { return $m.Groups[1].Value }
    return $null
}

function ConvertTo-GameVersionName([string]$Interface) {
    $n = [int]$Interface
    return '{0}.{1}.{2}' -f [math]::Floor($n / 10000), ([math]::Floor($n / 100) % 100), ($n % 100)
}

function Get-ZipToc([string]$ZipPath) {
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [IO.Compression.ZipFile]::OpenRead($ZipPath)
    try {
        $entry = $zip.GetEntry('Scarlet/Scarlet.toc')
        if (-not $entry) { throw "$ZipPath has no Scarlet/Scarlet.toc." }
        $reader = [IO.StreamReader]::new($entry.Open())
        try { return $reader.ReadToEnd() } finally { $reader.Dispose() }
    }
    finally { $zip.Dispose() }
}

function Get-AddonChangelog([string]$Notes) {
    $items = @()
    $inside = $false
    foreach ($line in ($Notes -split '\r?\n')) {
        if (-not $inside) { $inside = $line -match '^Addon changes'; continue }
        if (-not $line.Trim()) { break }
        $items += ($line -replace ' \([0-9a-f]{7,40}\)$', '')
    }
    $items = @($items | Where-Object { $_ -ne '- no commits' })
    if ($items.Count -eq 0) { return 'No changes to the addon since the previous release.' }
    return ($items -join "`n")
}

function Get-Ledger {
    if (-not (Test-Path -LiteralPath $LedgerPath)) { return @{} }
    return Get-Content -Raw -LiteralPath $LedgerPath | ConvertFrom-Json -AsHashtable
}

function Add-LedgerEntry([string]$StoreKey, [string]$ProjectId, [string]$Result) {
    $ledger = Get-Ledger
    if (-not $ledger.ContainsKey($StoreKey)) { $ledger[$StoreKey] = @{} }
    $ledger[$StoreKey][$Version] = [ordered]@{ project = $ProjectId; at = [datetime]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ssZ'); result = $Result }
    New-Item -ItemType Directory -Force (Split-Path $LedgerPath) | Out-Null
    $ledger | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $LedgerPath -Encoding utf8NoBOM
}

function Test-AlreadyUploaded([string]$StoreKey, [string]$ProjectId) {
    $ledger = Get-Ledger
    return $ledger.ContainsKey($StoreKey) -and $ledger[$StoreKey].ContainsKey($Version) -and $ledger[$StoreKey][$Version].project -eq $ProjectId
}

function Resolve-CurseVersionIds([string]$Token, [string[]]$Names) {
    $versions = @(Invoke-RestMethod -Uri "$CurseSite/api/game/wow/versions" -Headers @{ 'X-Api-Token' = $Token } | ForEach-Object { $_ })
    $ids = @()
    foreach ($name in $Names) {
        $hits = @($versions | Where-Object { $_.name -eq $name })
        $forever = @($hits | Where-Object { $_.gameVersionTypeID -eq $CurseForeverTypeId })
        if ($forever.Count -eq 1) { $ids += [int]$forever[0].id }
        elseif ($hits.Count -eq 1) {
            Write-Host "CurseForge: game version $name is type $($hits[0].gameVersionTypeID), not $CurseForeverTypeId; using it."
            $ids += [int]$hits[0].id
        }
        else { throw "CurseForge lists $($hits.Count) game versions named $name ($($forever.Count) of type $CurseForeverTypeId). Cannot pick one." }
    }
    return , $ids
}

function Send-Multipart([string]$Uri, [hashtable]$Headers, [string]$Json, [string]$ZipPath) {
    $response = Invoke-WebRequest -Uri $Uri -Method Post -Headers $Headers -Form @{ metadata = $Json; file = (Get-Item -LiteralPath $ZipPath) } -SkipHttpErrorCheck
    $status = [int]$response.StatusCode
    if ($status -lt 200 -or $status -ge 300) { throw "HTTP $status from $Uri`: $("$($response.Content)".Trim())" }
    return "$($response.Content)".Trim()
}

function Publish-Store([string]$Key, [string]$ProjectId, $Token, [string]$ZipPath, [string]$Notes, [string[]]$GameVersions) {
    $label = $StoreNames[$Key]
    if (-not $ProjectId) {
        Write-Host "${label}: skipped, no project ID. Put it in addon/Scarlet/Scarlet.toc ($(if ($Key -eq 'curseforge') { '## X-Curse-Project-ID' } else { '## X-Wago-ID' })) or pass $(if ($Key -eq 'curseforge') { '-CurseProjectId' } else { '-WagoId' })."
        return $true
    }
    if (-not $Token -and -not $DryRun) {
        Write-Host "${label}: skipped, no token. Set $(if ($Key -eq 'curseforge') { 'CF_API_KEY' } else { 'WAGO_API_TOKEN' }) or write $(Join-Path $SecretsDir $(if ($Key -eq 'curseforge') { 'curseforge-token' } else { 'wago-token' }))."
        return $true
    }
    if (-not $DryRun -and -not $Force -and (Test-AlreadyUploaded $Key $ProjectId)) {
        Write-Host "${label}: skipped, $Version was already uploaded to project $ProjectId from this PC (-Force uploads again)."
        return $true
    }

    $tokenValue = if ($Token) { $Token.Value } else { '' }
    if ($Key -eq 'curseforge') {
        $ids = @()
        $note = $null
        if ($Token) { $ids = Resolve-CurseVersionIds $Token.Value $GameVersions }
        else { $note = "no token, so gameVersions is empty here; a real upload looks up the ids of $($GameVersions -join ', ') in $CurseSite/api/game/wow/versions" }
        $metadata = [ordered]@{
            displayName   = $Version
            gameVersions  = $ids
            releaseType   = $(if ($IsBeta) { 'beta' } else { 'release' })
            changelog     = $Notes
            changelogType = 'markdown'
        }
        $uri = "$CurseSite/api/projects/$ProjectId/upload-file"
        $headers = @{ 'X-Api-Token' = $tokenValue }
    }
    else {
        $metadata = [ordered]@{
            label                    = $Version
            supported_forever_patches = @($GameVersions)
            stability                = $(if ($IsBeta) { 'beta' } else { 'stable' })
            changelog                = $Notes
        }
        $note = $null
        $uri = "https://addons.wago.io/api/projects/$ProjectId/version"
        $headers = @{ Authorization = "Bearer $tokenValue"; Accept = 'application/json' }
    }
    $json = $metadata | ConvertTo-Json -Depth 6

    if ($DryRun) {
        Write-Host "== $label dry run: POST $uri"
        Write-Host "file: $ZipName ($((Get-Item -LiteralPath $ZipPath).Length) bytes); token: $(if ($Token) { "found in $($Token.Source)" } else { 'missing' })"
        if ($note) { Write-Host "note: $note" }
        Write-Host $json
        return $true
    }

    Write-Host "${label}: uploading $ZipName to project $ProjectId ($(if ($IsBeta) { 'beta' } else { 'release' }), $($GameVersions -join ', '))"
    try { $result = Send-Multipart $uri $headers $json $ZipPath }
    catch { Write-Host "${label}: upload failed. $($_.Exception.Message)"; return $false }
    Add-LedgerEntry $Key $ProjectId ($result.Substring(0, [math]::Min(200, $result.Length)))
    Write-Host "${label}: uploaded. $result"
    return $true
}

$null = Invoke-Tool 'gh' @('auth', 'status')
if ($env:TEMP -like 'C:*' -or $env:TMP -like 'C:*') { throw "TEMP is on C: ($env:TEMP). Point it at E:\Temp first." }

$release = (Invoke-Tool 'gh' @('release', 'view', $Tag, '-R', $AppRepo, '--json', 'isDraft,isPrerelease,body')) -join "`n" | ConvertFrom-Json
if ($release.isDraft) { throw "Release $Tag on $AppRepo is a draft. Publish it first." }
if ($release.isPrerelease -ne $IsBeta) { throw "Release $Tag is $(if ($release.isPrerelease) { 'a prerelease' } else { 'not a prerelease' }), which does not match its version." }

$toc = Get-TocText
$ids = [ordered]@{
    curseforge = $(if ($CurseProjectId) { $CurseProjectId } else { Get-TocValue $toc 'X-Curse-Project-ID' })
    wago       = $(if ($WagoId) { $WagoId } else { Get-TocValue $toc 'X-Wago-ID' })
}
$tokens = [ordered]@{
    curseforge = Read-StoreToken 'CF_API_KEY' 'curseforge-token'
    wago       = Read-StoreToken 'WAGO_API_TOKEN' 'wago-token'
}

$work = Join-Path $Root "work\stores-$Version"
New-Item -ItemType Directory -Force $work | Out-Null
$failed = $false
try {
    $null = Invoke-Tool 'gh' @('release', 'download', $Tag, '-R', $AppRepo, '-p', $ZipName, '-p', 'SHA256SUMS.txt', '-D', $work, '--clobber')
    $zipPath = Join-Path $work $ZipName
    $sum = @(Get-Content -LiteralPath (Join-Path $work 'SHA256SUMS.txt') | Where-Object { $_ -match "\s$([regex]::Escape($ZipName))$" })
    if ($sum.Count -ne 1) { throw "SHA256SUMS.txt of $Tag has no line for $ZipName." }
    $want = ($sum[0] -split '\s+')[0]
    $got = (Get-FileHash -Algorithm SHA256 -LiteralPath $zipPath).Hash.ToLower()
    if ($got -ne $want) { throw "$ZipName downloaded with SHA-256 $got, not $want." }

    $zipToc = Get-ZipToc $zipPath
    if ((Get-TocValue $zipToc 'Version') -ne $Version) { throw "The TOC in $ZipName does not say version $Version." }
    $gameVersions = if ($GameVersion) { @($GameVersion) }
    else {
        $interface = [regex]::Match($zipToc, '(?mi)^##[ \t]*Interface[ \t]*:[ \t]*([^\r\n]+)').Groups[1].Value
        if (-not $interface) { throw "The TOC in $ZipName has no ## Interface line." }
        @($interface -split '[,\s]+' | Where-Object { $_ } | ForEach-Object { ConvertTo-GameVersionName $_ })
    }

    $wagoGame = Invoke-RestMethod -Uri 'https://addons.wago.io/api/data/game'
    foreach ($patch in $gameVersions) {
        if ($patch -notin @($wagoGame.patches.forever)) { throw "Wago has no Forever patch $patch (it lists $(@($wagoGame.patches.forever) -join ', '))." }
    }

    $notes = if ($Changelog) { (Get-Content -Raw -LiteralPath $Changelog).Trim() } else { Get-AddonChangelog $release.body }

    Write-Host "Scarlet $Version ($(if ($IsBeta) { 'beta' } else { 'release' })) for Forever $($gameVersions -join ', ')$(if ($DryRun) { ' [dry run]' })"
    foreach ($key in $Store) {
        if (-not (Publish-Store $key $ids[$key] $tokens[$key] $zipPath $notes $gameVersions)) { $failed = $true }
    }
}
finally {
    if (Test-Path -LiteralPath $work) { Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue }
}
if ($failed) { exit 1 }

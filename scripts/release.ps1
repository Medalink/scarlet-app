#Requires -Version 7
<#
.SYNOPSIS
Builds, signs and publishes Scarlet (the Windows app and the addon zip) to GitHub releases on Medalink/scarlet-app.

.DESCRIPTION
  release.ps1 -Lane beta                      build remote main of both repos as <next>-beta.<N>, publish a prerelease
  release.ps1 -Lane release [-From v0.4.0-beta.2]
                                              rebuild the SAME two commits as that beta (default: the newest beta
                                              for the next version) at <x> and publish the latest release v<x>
  -DryRun                                     build and sign into <Root>\<version>; publish nothing

Versions come from tags only; no committed file holds one. The app version is passed to tauri with --config at
build time and the addon version is stamped into the TOC inside the zip. The next version is the latest stable tag
plus one minor (the first is 0.4.0); -Version x.y.z sets it by hand for the beta lane.

The installer and the app exe are signed with the Azure Artifact Signing setup that signs the owner's other
products: the sign command comes from -SignCommand (default: the file the Whisper release lane uses) and
authenticates through the cached `az login`. Nothing is published unless the installer and the app exe verify as
Valid with a timestamp, both before upload and again after downloading the published installer.

A release is created as a draft, filled, then published, so a failed upload never leaves a half release.
See docs/RELEASING.md.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('beta', 'release')][string]$Lane,
    [string]$From,
    [string]$Version,
    [switch]$DryRun,
    [string]$Root = 'E:\Dev\scratch\scarlet-release',
    [string]$AppRepo = 'Medalink/scarlet-app',
    [string]$AddonRepo = 'Medalink/scarlet',
    [string]$SignCommand = $(if ($env:SCARLET_SIGN_COMMAND) { $env:SCARLET_SIGN_COMMAND } else { 'C:\Users\medal\.whisper-release-keys\rewrite\windows\sign-command.json' }),
    [string]$TargetDir = 'E:\Dev\caches\scarlet\target-release',
    [string]$ToolCache = 'E:\Dev\caches\tauri',
    [switch]$AllowToolCacheOnC
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$FirstVersion = '0.4.0'
$TimestampUrl = 'http://timestamp.acs.microsoft.com'
$BetaPattern = '^v(\d+\.\d+\.\d+)-beta\.(\d+)$'
$StablePattern = '^v(\d+\.\d+\.\d+)$'

function Write-Step([string]$Text) { Write-Host "==> $Text" -ForegroundColor Cyan }

# Runs a native tool; returns its stdout lines and throws with its stderr on a non-zero exit.
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

function Get-Gh([string[]]$Arguments) { Invoke-Tool 'gh' $Arguments }

function ConvertTo-VersionKey([string]$Text) { [version]$Text }

# ---- tags -> versions ---------------------------------------------------------------------------------------

function Get-Releases {
    $json = (Get-Gh @('release', 'list', '-R', $AppRepo, '--limit', '1000', '--json', 'tagName,isDraft,isPrerelease,createdAt')) -join "`n"
    if (-not $json) { return , @() }
    return , @($json | ConvertFrom-Json | Where-Object { $_ })
}

function Get-LatestStable([object[]]$Releases) {
    $stable = @($Releases | Where-Object { -not $_.isDraft -and -not $_.isPrerelease -and $_.tagName -match $StablePattern })
    if ($stable.Count -eq 0) { return $null }
    return ($stable | Sort-Object { ConvertTo-VersionKey $_.tagName.TrimStart('v') } -Descending | Select-Object -First 1)
}

function Get-NextVersion([object[]]$Releases) {
    if ($Version) {
        if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw "-Version must look like 0.5.0, not '$Version'" }
        return $Version
    }
    $latest = Get-LatestStable $Releases
    if (-not $latest) { return $FirstVersion }
    $v = ConvertTo-VersionKey ($latest.tagName.TrimStart('v'))
    return "$($v.Major).$($v.Minor + 1).0"
}

# The highest beta number already used for a version, from releases (drafts too) and from bare tags.
function Get-MaxBeta([object[]]$Releases, [string]$Next) {
    $max = 0
    foreach ($r in $Releases) {
        if ($r.tagName -match $BetaPattern -and $Matches[1] -eq $Next) { $max = [Math]::Max($max, [int]$Matches[2]) }
    }
    $refs = Get-Gh @('api', "repos/$AppRepo/git/matching-refs/tags/v$Next-beta.", '--jq', '.[].ref')
    foreach ($ref in $refs) {
        if ($ref -match '-beta\.(\d+)$') { $max = [Math]::Max($max, [int]$Matches[1]) }
    }
    return $max
}

function Get-NewestBeta([object[]]$Releases) {
    $latest = Get-LatestStable $Releases
    $floor = if ($latest) { ConvertTo-VersionKey ($latest.tagName.TrimStart('v')) } else { [version]'0.0.0' }
    $betas = foreach ($r in $Releases) {
        if ($r.isDraft -or $r.tagName -notmatch $BetaPattern) { continue }
        if ((ConvertTo-VersionKey $Matches[1]) -le $floor) { continue }
        [pscustomobject]@{ Tag = $r.tagName; Version = ConvertTo-VersionKey $Matches[1]; Number = [int]$Matches[2] }
    }
    $pick = @($betas | Sort-Object Version, Number -Descending | Select-Object -First 1)
    if ($pick.Count -eq 0) { throw 'There is no published beta newer than the latest release. Cut one with -Lane beta first.' }
    return $pick[0].Tag
}

# The two commits a release was built from, read back from its notes (fixed lines, see New-Notes).
function Get-ReleaseShas([string]$Tag) {
    $body = (Get-Gh @('release', 'view', $Tag, '-R', $AppRepo, '--json', 'body', '--jq', '.body')) -join "`n"
    if ($body -notmatch '(?m)^App commit: ([0-9a-f]{40})\s*$') { return $null }
    $app = $Matches[1]
    if ($body -notmatch '(?m)^Addon commit: ([0-9a-f]{40})\s*$') { return $null }
    return [pscustomobject]@{ App = $app; Addon = $Matches[1] }
}

# ---- sources ------------------------------------------------------------------------------------------------

function Sync-Repo([string]$Repo, [string]$Name) {
    $dir = Join-Path $Root "repos\$Name"
    if (-not (Test-Path -LiteralPath (Join-Path $dir '.git'))) {
        New-Item -ItemType Directory -Force (Split-Path $dir) | Out-Null
        Invoke-Tool 'git' @('clone', '--quiet', "https://github.com/$Repo.git", $dir) | Out-Null
    }
    Invoke-Tool 'git' @('-C', $dir, 'fetch', '--quiet', '--tags', '--force', 'origin', '+refs/heads/main:refs/remotes/origin/main') | Out-Null
    return $dir
}

function Get-MainHead([string]$Repo) { (Get-Gh @('api', "repos/$Repo/commits/main", '--jq', '.sha'))[0].Trim() }

# git archive of one commit (optionally one folder) extracted to $Dest.
function Export-Tree([string]$RepoDir, [string]$Sha, [string]$Dest, [string]$Subpath) {
    if (Test-Path -LiteralPath $Dest) { Remove-Item -LiteralPath $Dest -Recurse -Force }
    New-Item -ItemType Directory -Force $Dest | Out-Null
    $zip = [IO.Path]::GetFullPath((Join-Path $Dest '..\tree.zip'))
    $gitArgs = @('-C', $RepoDir, 'archive', '--format=zip', '-o', $zip, $Sha)
    if ($Subpath) { $gitArgs += $Subpath }
    Invoke-Tool 'git' $gitArgs | Out-Null
    Expand-Archive -LiteralPath $zip -DestinationPath $Dest -Force
    Remove-Item -LiteralPath $zip -Force
}

function Get-CommitTitles([string]$RepoDir, [string]$Base, [string]$Tip) {
    $range = if ($Base) { "$Base..$Tip" } else { $Tip }
    try { $lines = Invoke-Tool 'git' @('-C', $RepoDir, 'log', '--no-merges', '--max-count=60', '--format=- %s (%h)', $range) }
    catch { $lines = Invoke-Tool 'git' @('-C', $RepoDir, 'log', '--no-merges', '--max-count=30', '--format=- %s (%h)', $Tip) }
    if ($lines.Count -eq 0) { return @('- no commits') }
    return $lines
}

# ---- signing ------------------------------------------------------------------------------------------------

function Read-SignCommand {
    if (-not (Test-Path -LiteralPath $SignCommand)) {
        throw "Signing is not set up: $SignCommand does not exist. Set SCARLET_SIGN_COMMAND or -SignCommand to a sign-command.json ({cmd, args with one %1})."
    }
    $sign = Get-Content -Raw -LiteralPath $SignCommand | ConvertFrom-Json -AsHashtable
    if ($sign -isnot [hashtable] -or $sign['cmd'] -isnot [string] -or $sign['args'] -isnot [object[]] -or @($sign['args'] | Where-Object { $_ -ceq '%1' }).Count -ne 1) {
        throw "$SignCommand must be {`"cmd`": `"<tool>`", `"args`": [..., `"%1`"]} with %1 as one argument"
    }
    if (-not (Test-Path -LiteralPath $sign['cmd'])) { throw "The sign tool $($sign['cmd']) does not exist." }
    $signArgs = [string[]]$sign['args']
    if ($signArgs -notcontains $TimestampUrl) { throw "$SignCommand must timestamp with $TimestampUrl (Azure signing certificates last three days, so an untimestamped signature expires)." }
    for ($i = 0; $i -lt $signArgs.Count - 1; $i++) {
        if (($signArgs[$i] -in @('/dlib', '/dmdf')) -and -not (Test-Path -LiteralPath $signArgs[$i + 1])) { throw "Signing needs $($signArgs[$i + 1]) (named by $SignCommand), which does not exist." }
    }
    return $sign
}

# The dlib signs through the cached Azure CLI login. Check it before a long build, never print the token.
function Test-SignLogin {
    try {
        $expires = (Invoke-Tool 'az' @('account', 'get-access-token', '--resource', 'https://codesigning.azure.net', '--query', 'expiresOn', '-o', 'tsv'))[0]
    }
    catch { throw "Signing cannot get an Azure token: $($_.Exception.Message). Run `az login` as the account that owns the signing profile, then rerun." }
    if ([datetime]::Parse($expires) -lt (Get-Date).AddMinutes(30)) { throw "The Azure login expires at $expires, too soon for a build. Run `az login`, then rerun." }
    Write-Host "signing login valid until $expires"
}

# Throws unless the file carries a trusted, timestamped Authenticode signature; returns the signer.
function Assert-Signed([string]$Path) {
    $s = Get-AuthenticodeSignature -LiteralPath $Path
    if ($s.Status -ne 'Valid') { throw "$(Split-Path $Path -Leaf) is not signed with a certificate Windows trusts: $($s.Status), $($s.StatusMessage)" }
    if (-not $s.TimeStamperCertificate) { throw "$(Split-Path $Path -Leaf) is signed without a timestamp, so it stops verifying when the certificate expires." }
    return $s.SignerCertificate.Subject
}

# Tauri's NSIS and WiX downloads default to %LOCALAPPDATA%\tauri. On the owner's PC that must be a junction to E:.
function Assert-ToolCacheOffC {
    if ($AllowToolCacheOnC) { return }
    $dir = Join-Path $env:LOCALAPPDATA 'tauri'
    $item = Get-Item -LiteralPath $dir -Force -ErrorAction SilentlyContinue
    if ($item -and $item.LinkType -eq 'Junction' -and "$($item.Target)" -like "$ToolCache*") { return }
    throw "Tauri keeps its NSIS tools in $dir, which is not a junction to $ToolCache, so the build would write to C:. Run: pwsh -File E:\Dev\tools\evict-c\evict-c.ps1 apply -Only tauri (or pass -AllowToolCacheOnC on a PC where C: is fine)."
}

# ---- addon zip ----------------------------------------------------------------------------------------------

function New-AddonZip([string]$AddonDir, [string]$ZipPath, [string]$Stamp, [datetimeoffset]$When) {
    Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem
    $latin1 = [Text.Encoding]::Latin1
    $root = (Resolve-Path -LiteralPath $AddonDir).Path
    $files = @(Get-ChildItem -LiteralPath $root -Recurse -File)
    [Array]::Sort($files, [Comparison[IO.FileInfo]] { param($a, $b) [string]::CompareOrdinal($a.FullName, $b.FullName) })
    $stamped = 0
    $fs = [IO.File]::Create($ZipPath)
    try {
        $zip = [IO.Compression.ZipArchive]::new($fs, [IO.Compression.ZipArchiveMode]::Create)
        try {
            foreach ($f in $files) {
                $rel = $f.FullName.Substring($root.Length + 1).Replace('\', '/')
                if ($rel -ieq 'Digest.lua') { continue }
                $bytes = [IO.File]::ReadAllBytes($f.FullName)
                if ($rel -ieq 'Scarlet.toc') {
                    $text = $latin1.GetString($bytes)
                    if ($text -notmatch '(?m)^## Version:') { throw 'The TOC has no "## Version:" line to stamp.' }
                    $bytes = $latin1.GetBytes([regex]::Replace($text, '(?m)^## Version:[^\r\n]*', "## Version: $Stamp"))
                    $stamped++
                }
                $entry = $zip.CreateEntry("Scarlet/$rel", [IO.Compression.CompressionLevel]::Optimal)
                $entry.LastWriteTime = $When
                $out = $entry.Open()
                try { $out.Write($bytes, 0, $bytes.Length) } finally { $out.Dispose() }
            }
        }
        finally { $zip.Dispose() }
    }
    finally { $fs.Dispose() }
    if ($stamped -ne 1) { throw "Expected one Scarlet.toc in the addon, found $stamped." }

    # Read it back: the stamped version, the folder layout and no Digest.lua.
    $check = [IO.Compression.ZipFile]::OpenRead($ZipPath)
    try {
        $names = @($check.Entries | ForEach-Object FullName)
        if ($names -contains 'Scarlet/Digest.lua') { throw 'Digest.lua is in the addon zip.' }
        if (@($names | Where-Object { $_ -notlike 'Scarlet/*' }).Count) { throw 'The addon zip has entries outside Scarlet/.' }
        $toc = $check.GetEntry('Scarlet/Scarlet.toc')
        $reader = [IO.StreamReader]::new($toc.Open())
        try { $content = $reader.ReadToEnd() } finally { $reader.Dispose() }
        if ($content -notmatch "(?m)^## Version: $([regex]::Escape($Stamp))\s*$") { throw 'The TOC in the zip does not carry the stamped version.' }
    }
    finally { $check.Dispose() }
}

# ---- notes --------------------------------------------------------------------------------------------------

function New-Notes($Plan, [string]$AppTitles, [string]$AddonTitles) {
    $since = if ($Plan.Previous) { "since $($Plan.Previous)" } else { 'in the latest commits' }
    $lines = @()
    if ($Plan.Lane -eq 'beta') {
        $lines += "Beta of $($Plan.Next). Try it, then promote it with ``pwsh scripts/release.ps1 -Lane release -From $($Plan.Tag)``."
    }
    else {
        $lines += "Built from the same commits as $($Plan.PromotedFrom)."
    }
    $lines += ''
    $lines += "Install: run ``$($Plan.InstallerName)`` (per user, no administrator needed). Addon: unzip ``$($Plan.ZipName)`` into ``<WoW folder>\_forever_\Interface\AddOns``."
    $lines += ''
    $lines += "App commit: $($Plan.AppSha)"
    $lines += "Addon commit: $($Plan.AddonSha)"
    $lines += ''
    $lines += "App changes $since"
    $lines += $AppTitles
    $lines += ''
    $lines += "Addon changes $since"
    $lines += $AddonTitles
    return ($lines -join "`n") + "`n"
}

# ---- main ---------------------------------------------------------------------------------------------------

$null = Invoke-Tool 'gh' @('auth', 'status')
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw 'cargo is not on PATH.' }
$null = Invoke-Tool 'cargo' @('tauri', '--version')
if ($env:TEMP -like 'C:*' -or $env:TMP -like 'C:*') { throw "TEMP is on C: ($env:TEMP). Point it at E:\Temp first." }
Assert-ToolCacheOffC
$sign = Read-SignCommand
Test-SignLogin

Write-Step 'Reading releases'
$releases = [object[]](Get-Releases)
$plan = [ordered]@{ Lane = $Lane; Previous = $null; PromotedFrom = $null }
$latestStable = Get-LatestStable $releases

if ($Lane -eq 'beta') {
    $next = Get-NextVersion $releases
    $n = (Get-MaxBeta $releases $next) + 1
    $plan.Next = $next
    $plan.Version = "$next-beta.$n"
    $plan.AppSha = Get-MainHead $AppRepo
    $plan.AddonSha = Get-MainHead $AddonRepo
    $plan.PromotedFrom = $null
}
else {
    $from = if ($From) { $From } else { Get-NewestBeta $releases }
    if ($from -notmatch $BetaPattern) { throw "-From must be a beta tag like v0.4.0-beta.2, not '$from'" }
    $plan.Next = $Matches[1]
    $plan.Version = $Matches[1]
    $plan.PromotedFrom = $from
    $shas = Get-ReleaseShas $from
    if (-not $shas) { throw "The notes of $from do not list the app and addon commits (lines 'App commit:' and 'Addon commit:'), so it cannot be promoted." }
    $tagSha = (Get-Gh @('api', "repos/$AppRepo/git/ref/tags/$from", '--jq', '.object.sha'))[0].Trim()
    if ($tagSha -ne $shas.App) { throw "Tag $from points at $tagSha but its notes say the app commit is $($shas.App)." }
    $plan.AppSha = $shas.App
    $plan.AddonSha = $shas.Addon
    if (@($releases | Where-Object { $_.tagName -eq "v$($plan.Version)" }).Count) { throw "Release v$($plan.Version) already exists. Delete it on GitHub first if it must be redone." }
}
$plan.Tag = "v$($plan.Version)"
$plan.Previous = $null
$plan.InstallerName = "Scarlet_$($plan.Version)_x64-setup.exe"
$plan.ZipName = "Scarlet-$($plan.Version).zip"
Write-Host "$Lane $($plan.Version): app $($plan.AppSha.Substring(0, 12)), addon $($plan.AddonSha.Substring(0, 12))"

$out = Join-Path $Root $plan.Version
New-Item -ItemType Directory -Force $out | Out-Null
$work = Join-Path $Root "work\$($plan.Version)"
New-Item -ItemType Directory -Force $work | Out-Null

try {
    Write-Step 'Fetching sources'
    $appRepoDir = Sync-Repo $AppRepo 'scarlet-app'
    $addonRepoDir = Sync-Repo $AddonRepo 'scarlet'
    $src = Join-Path $work 'app-src'
    Export-Tree $appRepoDir $plan.AppSha $src $null
    $addonSrc = Join-Path $work 'addon-src'
    Export-Tree $addonRepoDir $plan.AddonSha $addonSrc 'addon/Scarlet'

    # Commit titles since the last release this lane's readers know: the latest published release for a beta,
    # the latest stable one for a release.
    $newest = @($releases | Where-Object { -not $_.isDraft } | Sort-Object createdAt -Descending | Select-Object -First 1)
    $baseTag = if ($Lane -eq 'release') { if ($latestStable) { $latestStable.tagName } else { $null } }
    elseif ($newest.Count) { $newest[0].tagName }
    else { $null }
    $baseShas = if ($baseTag) { Get-ReleaseShas $baseTag } else { $null }
    $plan.Previous = if ($baseShas) { $baseTag } else { $null }
    $baseApp = if ($baseShas) { $baseShas.App } else { $null }
    $baseAddon = if ($baseShas) { $baseShas.Addon } else { $null }
    $appTitles = (Get-CommitTitles $appRepoDir $baseApp $plan.AppSha) -join "`n"
    $addonTitles = (Get-CommitTitles $addonRepoDir $baseAddon $plan.AddonSha) -join "`n"

    Write-Step 'Packaging the addon'
    $zipPath = Join-Path $out $plan.ZipName
    if (Test-Path -LiteralPath $zipPath) { Remove-Item -LiteralPath $zipPath -Force }
    $when = [datetimeoffset]::Parse((Invoke-Tool 'git' @('-C', $addonRepoDir, 'show', '-s', '--format=%cI', $plan.AddonSha))[0])
    New-AddonZip (Join-Path $addonSrc 'addon\Scarlet') $zipPath $plan.Version $when

    Write-Step "Building the app $($plan.Version) (NSIS, signed)"
    $versionConfig = Join-Path $work 'version.json'
    @{ version = $plan.Version } | ConvertTo-Json | Set-Content -LiteralPath $versionConfig -Encoding utf8NoBOM
    $signConfig = Join-Path $work 'sign.json'
    @{ bundle = @{ windows = @{ signCommand = @{ cmd = $sign['cmd']; args = $sign['args'] } } } } | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $signConfig -Encoding utf8NoBOM
    $bundleDir = Join-Path $TargetDir 'release\bundle\nsis'
    if (Test-Path -LiteralPath $bundleDir) { Remove-Item -LiteralPath $bundleDir -Recurse -Force }
    $log = Join-Path $out 'build.log'
    $previousTarget = $env:CARGO_TARGET_DIR
    $env:CARGO_TARGET_DIR = $TargetDir
    Push-Location (Join-Path $src 'app')
    try {
        & cargo tauri build --ci --bundles nsis --config $versionConfig --config $signConfig 2>&1 | Tee-Object -FilePath $log | ForEach-Object { "$_" }
        if ($LASTEXITCODE -ne 0) { throw "cargo tauri build failed ($LASTEXITCODE); see $log. If the log mentions signing, the signature is the reason." }
    }
    finally {
        Pop-Location
        $env:CARGO_TARGET_DIR = $previousTarget
    }
    $built = Join-Path $bundleDir $plan.InstallerName
    if (-not (Test-Path -LiteralPath $built)) { throw "The build did not produce $($plan.InstallerName) in $bundleDir." }
    $installer = Join-Path $out $plan.InstallerName
    Copy-Item -LiteralPath $built -Destination $installer -Force

    Write-Step 'Verifying signatures'
    # Tauri signs the app exe before it goes into the installer (the build log shows it); the exe in the
    # installer is checked after installing it, see docs/RELEASING.md.
    $installerSigner = Assert-Signed $installer
    Write-Host "$($plan.InstallerName) signed by $installerSigner"

    $sums = Join-Path $out 'SHA256SUMS.txt'
    $lines = foreach ($f in @($installer, $zipPath)) { "$((Get-FileHash -Algorithm SHA256 -LiteralPath $f).Hash.ToLower())  $(Split-Path $f -Leaf)" }
    Set-Content -LiteralPath $sums -Value $lines -Encoding utf8NoBOM

    $notes = Join-Path $out 'notes.md'
    Set-Content -LiteralPath $notes -Value (New-Notes ([pscustomobject]$plan) $appTitles $addonTitles) -Encoding utf8NoBOM -NoNewline

    if ($DryRun) {
        Write-Step "Dry run done; nothing published. Files are in $out"
        Get-ChildItem -LiteralPath $out | Select-Object Name, Length | Format-Table -AutoSize | Out-Host
        return
    }

    Write-Step "Creating draft $($plan.Tag) on $AppRepo"
    $create = @('release', 'create', $plan.Tag, $installer, $zipPath, $sums, '-R', $AppRepo, '--draft', '--target', $plan.AppSha, '--title', "Scarlet $($plan.Version)", '--notes-file', $notes)
    if ($Lane -eq 'beta') { $create += '--prerelease' }
    $null = Get-Gh $create

    $drafts = ((Get-Gh @('api', "repos/$AppRepo/releases?per_page=100")) -join "`n") | ConvertFrom-Json
    $draft = @($drafts | Where-Object { $_.tag_name -eq $plan.Tag -and $_.draft })[0]
    if (-not $draft) { throw "The draft $($plan.Tag) is not on $AppRepo after creating it." }
    $names = @($draft.assets | ForEach-Object name)
    foreach ($want in @($plan.InstallerName, $plan.ZipName, 'SHA256SUMS.txt')) {
        if ($want -notin $names) { throw "The draft is missing $want; it stays a draft for inspection." }
    }

    Write-Step "Publishing $($plan.Tag)"
    $latest = if ($Lane -eq 'release') { 'true' } else { 'false' }
    $null = Get-Gh @('api', '-X', 'PATCH', "repos/$AppRepo/releases/$($draft.id)", '-F', 'draft=false', '-f', "make_latest=$latest")

    Write-Step 'Downloading it back'
    $back = Join-Path $out 'downloaded'
    if (Test-Path -LiteralPath $back) { Remove-Item -LiteralPath $back -Recurse -Force }
    try {
        $null = Get-Gh @('release', 'download', $plan.Tag, '-R', $AppRepo, '-D', $back)
        $tagSha = (Get-Gh @('api', "repos/$AppRepo/git/ref/tags/$($plan.Tag)", '--jq', '.object.sha'))[0].Trim()
        if ($tagSha -ne $plan.AppSha) { throw "Tag $($plan.Tag) points at $tagSha, not the built commit $($plan.AppSha)." }
        foreach ($line in (Get-Content -LiteralPath (Join-Path $back 'SHA256SUMS.txt'))) {
            $hash, $name = $line -split '\s+', 2
            $got = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $back $name)).Hash.ToLower()
            if ($got -ne $hash) { throw "$name downloaded with SHA-256 $got, not $hash." }
        }
        $downloadedSigner = Assert-Signed (Join-Path $back $plan.InstallerName)
        Write-Host "downloaded installer signed by $downloadedSigner"
    }
    catch {
        $null = Get-Gh @('api', '-X', 'PATCH', "repos/$AppRepo/releases/$($draft.id)", '-F', 'draft=true')
        throw "Published $($plan.Tag), but checking it after download failed, so it is a draft again: $($_.Exception.Message)"
    }

    Write-Step 'Done'
    Write-Host "https://github.com/$AppRepo/releases/tag/$($plan.Tag)"
    if ($Lane -eq 'beta') { Write-Host "Promote: pwsh scripts/release.ps1 -Lane release -From $($plan.Tag)" }
}
finally {
    if (Test-Path -LiteralPath $work) { Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue }
}

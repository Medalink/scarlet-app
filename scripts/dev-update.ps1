#Requires -Version 7
<#
.SYNOPSIS
Keeps the owner's dev copy of Scarlet on medaviper at the newest main: rebuilds it after a ship and swaps the exe.

.DESCRIPTION
  dev-update.ps1             one tick: fetch main, and when it moved, build it and put the new exe in place
  dev-update.ps1 -Install    run a tick every 5 minutes from the "Scarlet dev update" scheduled task
  dev-update.ps1 -Uninstall  delete that task

A tick builds only when the Overwatch game tracer (E:\Dev\tools\ow-tracer\state\owner.json) has seen no game for 10
minutes, the rule medaviper's fleet slots follow; a missing or stale tracer file counts as a game. It builds a clone of
main of its own (<Root>\src, whose target ws keeps as a main checkout's), never the checkout agents work in.

The swap renames the running exe aside and copies the new one in, so it works while Scarlet is open: the open window
shows "Restart to update" (updater::rebuilt) and the next launch runs the new build. The windowless --on-wow watcher is
stopped and started again from the new exe, and nothing else is started, so no window opens. Renamed copies are
deleted once nothing runs them. A failed build is not tried again until main moves. Log: <Root>\update.log.
#>
[CmdletBinding()]
param(
    [switch]$Install,
    [switch]$Uninstall,
    [string]$App = 'E:\Dev\scarlet-app\app\target\release',
    [string]$Root = 'E:\Dev\tools\scarlet-dev-update',
    [string]$Tracer = 'E:\Dev\tools\ow-tracer\state\owner.json',
    [string]$Repo = 'https://github.com/Medalink/scarlet-app'
)

$ErrorActionPreference = 'Stop'
$TaskName = 'Scarlet dev update'
$Src = Join-Path $Root 'src'
$StateFile = Join-Path $Root 'state.json'
$Log = Join-Path $Root 'update.log'
$Exe = Join-Path $App 'scarlet-app.exe'

function Write-Log([string]$Text) {
    $line = "$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') $Text"
    Add-Content -LiteralPath $Log -Value $line
    Write-Output $line
}

function Get-State {
    try { return Get-Content -LiteralPath $StateFile -Raw | ConvertFrom-Json -AsHashtable } catch { return @{} }
}

function Save-State([hashtable]$State) {
    $State | ConvertTo-Json | Set-Content -LiteralPath "$StateFile.tmp"
    Move-Item -LiteralPath "$StateFile.tmp" -Destination $StateFile -Force
}

function Test-OwnerBusy {
    try { $o = Get-Content -LiteralPath $Tracer -Raw | ConvertFrom-Json } catch { return 'no readable game tracer file' }
    if (((Get-Date).ToUniversalTime() - ([datetime]$o.at).ToUniversalTime()).TotalMinutes -gt 2) { return 'the game tracer has not written for over 2 min' }
    if ($o.gaming) { return "the owner is playing $($o.app)" }
    if (((Get-Date).ToUniversalTime() - ([datetime]$o.since).ToUniversalTime()).TotalMinutes -lt 10) { return 'the owner stopped a game under 10 min ago' }
    return $null
}

function Invoke-Git {
    & git -C $Src @args
    if ($LASTEXITCODE -ne 0) { throw "git $($args -join ' ') failed ($LASTEXITCODE)" }
}

function Get-Watchers {
    Get-CimInstance Win32_Process -Filter "Name = 'scarlet-app.exe'" |
        Where-Object { $_.CommandLine -like "*$Exe*--on-wow*" }
}

function Restart-Watchers([object[]]$Watchers) {
    if ($Watchers.Count -eq 0) { return }
    foreach ($w in $Watchers) { Stop-Process -Id $w.ProcessId -Force -ErrorAction SilentlyContinue }
    Wait-Process -Id ($Watchers.ProcessId) -Timeout 10 -ErrorAction SilentlyContinue
    Start-Process -FilePath $Exe -ArgumentList '--on-wow' -WorkingDirectory $App
}

function Remove-OldCopies {
    Get-ChildItem -LiteralPath $App -Filter 'scarlet-app.old-*.exe' -ErrorAction SilentlyContinue | ForEach-Object {
        try { Remove-Item -LiteralPath $_.FullName -Force } catch { }
    }
}

if ($Install -or $Uninstall) {
    if ((Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue)) {
        Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false
    }
    if ($Uninstall) { Write-Output "Deleted the $TaskName task."; return }
    New-Item -ItemType Directory -Force -Path $Root | Out-Null
    if (-not (Test-Path -LiteralPath (Join-Path $Src '.git'))) {
        & git clone -q $Repo $Src
        if ($LASTEXITCODE -ne 0) { throw "git clone $Repo failed" }
    }
    $pwsh = (Get-Command pwsh).Source
    $script = Join-Path $Src 'scripts\dev-update.ps1'
    $action = New-ScheduledTaskAction -Execute 'conhost.exe' -Argument "--headless `"$pwsh`" -NoProfile -NonInteractive -File `"$script`"" -WorkingDirectory $Root
    $trigger = New-ScheduledTaskTrigger -Once -At (Get-Date) -RepetitionInterval (New-TimeSpan -Minutes 5)
    $settings = New-ScheduledTaskSettingsSet -MultipleInstances IgnoreNew -StartWhenAvailable -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Hours 1)
    $principal = New-ScheduledTaskPrincipal -UserId ([Security.Principal.WindowsIdentity]::GetCurrent().Name) -LogonType Interactive -RunLevel Limited
    Register-ScheduledTask -TaskName $TaskName -Action $action -Trigger $trigger -Settings $settings -Principal $principal | Out-Null
    Write-Output "Scheduled $TaskName every 5 minutes: $script"
    return
}

[Diagnostics.Process]::GetCurrentProcess().PriorityClass = 'BelowNormal'
New-Item -ItemType Directory -Force -Path $Root | Out-Null
$lock = $null
try { $lock = [IO.File]::Open((Join-Path $Root 'update.lock'), 'OpenOrCreate', 'ReadWrite', 'None') } catch { return }
try {
    Remove-OldCopies
    $state = Get-State
    if (-not (Test-Path -LiteralPath (Join-Path $Src '.git'))) {
        & git clone -q $Repo $Src
        if ($LASTEXITCODE -ne 0) { throw "git clone $Repo failed" }
    }
    Invoke-Git fetch -q origin main
    $sha = (& git -C $Src rev-parse origin/main).Trim()
    $have = if (Test-Path -LiteralPath $Exe) { (Get-FileHash -LiteralPath $Exe).Hash } else { '' }
    if ($state.built -eq $sha -and $state.hash -eq $have) { return }
    if ($state.failed -eq $sha) { return }
    $busy = Test-OwnerBusy
    if ($busy) {
        if ($state.waiting -ne $sha) { Write-Log "main is at $($sha.Substring(0, 7)); waiting because $busy"; $state.waiting = $sha; Save-State $state }
        return
    }

    $built = Join-Path $Src 'app\target\release\scarlet-app.exe'
    if ($state.built -ne $sha -or -not (Test-Path -LiteralPath $built)) {
        Invoke-Git checkout -q --force --detach $sha
        Invoke-Git clean -q -fd
        Write-Log "building $($sha.Substring(0, 7))"
        Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
        $buildLog = Join-Path $Root 'build.log'
        $p = Start-Process -FilePath 'cargo' -ArgumentList 'build', '--release', '--locked' -WorkingDirectory (Join-Path $Src 'app') `
            -NoNewWindow -PassThru -RedirectStandardError $buildLog -RedirectStandardOutput "$buildLog.out"
        $p.WaitForExit()
        if ($p.ExitCode -ne 0) {
            Write-Log "build of $($sha.Substring(0, 7)) failed ($($p.ExitCode)); see $buildLog"
            $state.failed = $sha
            Save-State $state
            return
        }
    }

    $watchers = @(Get-Watchers)
    $aside = $null
    if (Test-Path -LiteralPath $Exe) {
        $aside = Join-Path $App "scarlet-app.old-$([DateTime]::UtcNow.Ticks).exe"
        Move-Item -LiteralPath $Exe -Destination $aside
    }
    try {
        Copy-Item -LiteralPath $built -Destination $Exe
    } catch {
        if ($aside) { Move-Item -LiteralPath $aside -Destination $Exe -Force }
        throw
    }
    Restart-Watchers $watchers
    $dark = Join-Path $Src 'app\target\release\dark'
    if (Test-Path -LiteralPath $dark) {
        & robocopy $dark (Join-Path $App 'dark') /MIR /NJH /NJS /NP /NFL /NDL | Out-Null
        if ($LASTEXITCODE -ge 8) { throw "robocopy of the dark map sheets failed ($LASTEXITCODE)" }
    }
    Remove-OldCopies
    $state.built = $sha
    $state.hash = (Get-FileHash -LiteralPath $Exe).Hash
    $state.Remove('failed')
    $state.Remove('waiting')
    Save-State $state
    Write-Log "installed $($sha.Substring(0, 7)) at $Exe$(if ($watchers.Count) { '; restarted the --on-wow watcher' })"
} catch {
    Write-Log "tick failed: $($_.Exception.Message)"
    exit 1
} finally {
    $lock.Dispose()
}

<#
  Deletes old builds of Tsundoku's own crates from Cargo's build folder. Every version bump gives each of our crates
  new build hashes (one each for the build, the tests and Clippy), and Cargo never deletes the old ones, so a build
  folder grows by gigabytes a day: one reached 231 GB on 2026-11-12. See AGENTS.md.
  The build folder is the one Cargo uses for the workspace (cargo metadata's target_directory, so a .cargo\config.toml
  or CARGO_TARGET_DIR is followed), or -TargetDir. In debug\ and release\, only deps, incremental, .fingerprint and
  build are looked at, and in them only entries named after a workspace crate followed by "-<hash>": third-party
  crates and the files at the top of debug\ and release\ are never touched. For each crate, the entries more than an
  hour older than its newest are deleted. A profile a build is using (Cargo's lock is held) is left alone, and so is
  anything in use. -WhatIf only says what would go. Check.cmd runs it after its checks.
#>
param(
    [string] $TargetDir = "",
    [switch] $WhatIf
)

$ErrorActionPreference = "Stop"
$workspace = Join-Path $PSScriptRoot "..\tsundoku"
$keepWithin = [TimeSpan]::FromHours(1)

Push-Location $workspace
try {
    $metadata = (& cargo metadata --format-version 1 --no-deps --offline) | Out-String | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw "cargo metadata failed." }
} finally {
    Pop-Location
}
if (-not $TargetDir) { $TargetDir = $metadata.target_directory }
$TargetDir = [IO.Path]::GetFullPath($TargetDir)
if (-not (Test-Path -LiteralPath $TargetDir -PathType Container)) {
    Write-Host "No build folder at $TargetDir."
    exit 0
}

# Each crate's names as entries spell them: packages (tsundoku-core) and their targets (tsundoku_core, tsundoku_lib),
# with "-" and "_" both, grouped under one name.
$crates = @{}
foreach ($package in $metadata.packages) {
    $names = @($package.name) + @($package.targets | Where-Object { $_.kind -notcontains "custom-build" } | ForEach-Object { $_.name })
    foreach ($name in $names) {
        $crate = $name.Replace("-", "_")
        $crates[$crate.Replace("_", "-")] = $crate
        $crates[$crate] = $crate
    }
}
$alternatives = ($crates.Keys | Sort-Object Length -Descending | ForEach-Object { [regex]::Escape($_) }) -join "|"
$patterns = @{
    # libtsundoku_core-0123456789abcdef.rlib, tsundoku_core-0123456789abcdef.d
    "deps"         = "^(lib)?(?<name>$alternatives)-[0-9a-f]{16}(\..*)?$"
    # tsundoku_core-2r3qeprt5lyl8
    "incremental"  = "^(?<name>$alternatives)-[0-9a-z]+$"
    # tsundoku-core-0123456789abcdef
    ".fingerprint" = "^(?<name>$alternatives)-[0-9a-f]{16}$"
    "build"        = "^(?<name>$alternatives)-[0-9a-f]{16}$"
}

function Get-Bytes($item) {
    if (-not $item.PSIsContainer) { return [int64] $item.Length }
    $sum = (Get-ChildItem -LiteralPath $item.FullName -Recurse -Force -File -ErrorAction SilentlyContinue |
        Measure-Object -Property Length -Sum).Sum
    if ($sum) { return [int64] $sum } else { return [int64] 0 }
}

# A folder's own time can be older than what was written in it, so its direct children count too.
function Get-LastWrite($item) {
    $newest = $item.LastWriteTimeUtc
    if ($item.PSIsContainer) {
        foreach ($child in Get-ChildItem -LiteralPath $item.FullName -Force -ErrorAction SilentlyContinue) {
            if ($child.LastWriteTimeUtc -gt $newest) { $newest = $child.LastWriteTimeUtc }
        }
    }
    return $newest
}

# Cargo holds these locks while it builds in a profile. Taking them keeps a build from starting while entries go.
function Lock-Profile([string] $profileFolder) {
    $held = @()
    foreach ($name in ".cargo-lock", ".cargo-build-lock", ".cargo-artifact-lock") {
        $path = Join-Path $profileFolder $name
        if (-not (Test-Path -LiteralPath $path)) { continue }
        try {
            $stream = [IO.File]::Open($path, "Open", "ReadWrite", "ReadWrite, Delete")
            $held += $stream
            $stream.Lock(0, 1)
        } catch {
            Unlock-Profile $held
            return $null
        }
    }
    return , $held
}

function Unlock-Profile($held) {
    foreach ($stream in $held) {
        try { $stream.Unlock(0, 1) } catch { }
        $stream.Dispose()
    }
}

function Format-Size([int64] $bytes) {
    if ($bytes -ge 1GB) { return "{0:N1} GB" -f ($bytes / 1GB) }
    return "{0:N0} MB" -f ($bytes / 1MB)
}

$verb = if ($WhatIf) { "Would delete" } else { "Deleted" }
$freed = [int64] 0
$inUse = 0
$busyProfiles = @()

foreach ($profileName in "debug", "release") {
    $profileFolder = Join-Path $TargetDir $profileName
    if (-not (Test-Path -LiteralPath $profileFolder -PathType Container)) { continue }
    $locks = Lock-Profile $profileFolder
    if ($null -eq $locks) {
        $busyProfiles += $profileName
        continue
    }
    try {
        $entries = @()
        foreach ($kind in $patterns.Keys) {
            $folder = Join-Path $profileFolder $kind
            if (-not (Test-Path -LiteralPath $folder -PathType Container)) { continue }
            foreach ($item in Get-ChildItem -LiteralPath $folder -Force) {
                $match = [regex]::Match($item.Name, $patterns[$kind])
                if (-not $match.Success) { continue }
                $entries += [pscustomobject]@{
                    Item      = $item
                    Crate     = $crates[$match.Groups["name"].Value]
                    LastWrite = Get-LastWrite $item
                }
            }
        }
        foreach ($group in $entries | Group-Object Crate | Sort-Object Name) {
            $newest = [DateTime]::MinValue
            foreach ($entry in $group.Group) { if ($entry.LastWrite -gt $newest) { $newest = $entry.LastWrite } }
            $old = @($group.Group | Where-Object { $_.LastWrite -lt $newest - $keepWithin })
            $kept = @($group.Group | Where-Object { $_.LastWrite -ge $newest - $keepWithin })
            $keptBytes = [int64] 0
            foreach ($entry in $kept) { $keptBytes += Get-Bytes $entry.Item }
            $goneBytes = [int64] 0
            $gone = 0
            foreach ($entry in $old) {
                $bytes = Get-Bytes $entry.Item
                if ($WhatIf) {
                    $goneBytes += $bytes
                    $gone++
                    continue
                }
                try {
                    Remove-Item -LiteralPath $entry.Item.FullName -Recurse -Force -ErrorAction Stop
                    $goneBytes += $bytes
                    $gone++
                } catch {
                    # A file in use stays; a folder may lose what wasn't in use.
                    $inUse++
                    if (Test-Path -LiteralPath $entry.Item.FullName) {
                        $goneBytes += $bytes - (Get-Bytes (Get-Item -LiteralPath $entry.Item.FullName -Force))
                    } else {
                        $goneBytes += $bytes
                    }
                }
            }
            $freed += $goneBytes
            Write-Host ("{0} {1}: {2} {3} entries ({4}), kept {5} ({6})" -f $profileName, $group.Name, $verb.ToLower(),
                $gone, (Format-Size $goneBytes), $kept.Count, (Format-Size $keptBytes))
        }
    } finally {
        Unlock-Profile $locks
    }
}

$summary = if ($WhatIf) { "Would free" } else { "Freed" }
Write-Host ("{0} {1} of old builds in {2}." -f $summary, (Format-Size $freed), $TargetDir)
if ($inUse) { Write-Host "$inUse entries were in use and stayed." }
foreach ($profileName in $busyProfiles) { Write-Host "Left $profileName alone: a build is using it." }

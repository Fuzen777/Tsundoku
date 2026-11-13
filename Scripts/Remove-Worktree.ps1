<#
  Removes a worktree whose work is on main, with its build folder. It changes nothing unless the path is a worktree
  of this repository other than the main checkout, its HEAD is already in main and git status shows nothing.
  Then it removes the links in it (the junction to the checkout's node_modules: only the link, never what it points
  to), deletes its build folder (cargo metadata's target_directory in it: only when that folder is inside the
  worktree, or a target-* folder beside the checkout's own build folder that no other worktree builds into; never
  the checkout's own), then runs git worktree remove and git worktree prune. See AGENTS.md.
#>
param(
    [Parameter(Mandatory = $true)] [string] $Path
)

$ErrorActionPreference = "Stop"

function Get-FullPath([string] $path) {
    return [IO.Path]::GetFullPath($path.Replace("/", "\")).TrimEnd("\")
}

function Test-SamePath([string] $first, [string] $second) {
    return [string]::Equals((Get-FullPath $first), (Get-FullPath $second), [StringComparison]::OrdinalIgnoreCase)
}

function Test-Inside([string] $path, [string] $folder) {
    return (Get-FullPath $path).StartsWith((Get-FullPath $folder) + "\", [StringComparison]::OrdinalIgnoreCase)
}

function Test-Link([IO.FileSystemInfo] $item) {
    return [bool] ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)
}

function Stop-Refused([string] $reason) {
    Write-Host "Refused, nothing changed: $reason"
    exit 1
}

# Cargo's build folder for the workspace in a checkout or worktree, as its .cargo\config.toml sets it. CARGO_TARGET_DIR
# is left out, so a variable set for one build doesn't make every worktree look like it builds there.
function Get-BuildFolder([string] $checkout) {
    $workspace = Join-Path $checkout "tsundoku"
    if (-not (Test-Path -LiteralPath (Join-Path $workspace "Cargo.toml"))) { return $null }
    $saved = $env:CARGO_TARGET_DIR, $env:CARGO_BUILD_TARGET_DIR
    $env:CARGO_TARGET_DIR = $null
    $env:CARGO_BUILD_TARGET_DIR = $null
    Push-Location $workspace
    try {
        $json = & cargo metadata --format-version 1 --no-deps --offline 2> $null | Out-String
        if ($LASTEXITCODE -ne 0) { return $null }
        return Get-FullPath ($json | ConvertFrom-Json).target_directory
    } finally {
        Pop-Location
        $env:CARGO_TARGET_DIR, $env:CARGO_BUILD_TARGET_DIR = $saved
    }
}

# Every file and link under a folder, never going through a link.
function Get-Contents([string] $folder) {
    $files = New-Object System.Collections.Generic.List[IO.FileInfo]
    $links = New-Object System.Collections.Generic.List[IO.FileSystemInfo]
    $pending = New-Object System.Collections.Generic.Stack[IO.DirectoryInfo]
    $pending.Push((New-Object IO.DirectoryInfo $folder))
    while ($pending.Count) {
        foreach ($item in $pending.Pop().EnumerateFileSystemInfos()) {
            if (Test-Link $item) { $links.Add($item) }
            elseif ($item -is [IO.DirectoryInfo]) { $pending.Push($item) }
            else { $files.Add($item) }
        }
    }
    return @{ Files = $files; Links = $links }
}

function Get-Bytes($files) {
    $sum = [int64] 0
    foreach ($file in $files) { $sum += $file.Length }
    return $sum
}

# Deletes a folder without going through links: a link inside is removed as a link. Files in use stay and are listed.
function Remove-Folder([string] $folder) {
    $contents = Get-Contents $folder
    $freed = [int64] 0
    $inUse = New-Object System.Collections.Generic.List[string]
    foreach ($link in $contents.Links) {
        try { if ($link -is [IO.DirectoryInfo]) { [IO.Directory]::Delete($link.FullName, $false) } else { $link.Delete() } }
        catch { $inUse.Add($link.FullName) }
    }
    foreach ($file in $contents.Files) {
        try {
            if ($file.IsReadOnly) { $file.IsReadOnly = $false }
            $length = $file.Length
            $file.Delete()
            $freed += $length
        } catch {
            $inUse.Add($file.FullName)
        }
    }
    $folders = @((New-Object IO.DirectoryInfo $folder).EnumerateDirectories("*", [IO.SearchOption]::AllDirectories)) +
        @(New-Object IO.DirectoryInfo $folder) | Sort-Object { $_.FullName.Length } -Descending
    foreach ($empty in $folders) { try { $empty.Delete($false) } catch { } }
    return @{ Freed = $freed; InUse = $inUse }
}

function Format-Size([int64] $bytes) {
    if ($bytes -ge 1GB) { return "{0:N1} GB" -f ($bytes / 1GB) }
    return "{0:N0} MB" -f ($bytes / 1MB)
}

if (-not (Test-Path -LiteralPath $Path -PathType Container)) { Stop-Refused "$Path isn't a folder." }
$worktree = Get-FullPath (Resolve-Path -LiteralPath $Path).ProviderPath

# The first worktree git lists is the main checkout.
$listed = @(& git -C $PSScriptRoot worktree list --porcelain | Where-Object { $_ -like "worktree *" } |
    ForEach-Object { Get-FullPath $_.Substring(9) })
if ($LASTEXITCODE -ne 0 -or -not $listed.Count) { Stop-Refused "git couldn't list this repository's worktrees." }
$checkout = $listed[0]
if (Test-SamePath $worktree $checkout) { Stop-Refused "$worktree is the main checkout." }
if (-not ($listed | Where-Object { Test-SamePath $_ $worktree })) { Stop-Refused "$worktree isn't a worktree of this repository." }
& git -C $worktree merge-base --is-ancestor HEAD main
if ($LASTEXITCODE -ne 0) { Stop-Refused "its HEAD isn't in main yet." }
$changes = @(& git -C $worktree status --porcelain)
if ($LASTEXITCODE -ne 0) { Stop-Refused "git status failed in it." }
if ($changes.Count) { Stop-Refused "it has uncommitted changes (git status --porcelain lists $($changes.Count))." }

# Which build folder goes, decided before anything changes.
$buildFolder = Get-BuildFolder $worktree
$deleteBuild = $false
if (-not $buildFolder) {
    Write-Host "No build folder found for it."
} elseif (-not (Test-Path -LiteralPath $buildFolder -PathType Container)) {
    Write-Host "Its build folder, $buildFolder, doesn't exist."
    $buildFolder = $null
} else {
    $checkoutBuild = Get-BuildFolder $checkout
    $sharedWith = @($listed | Where-Object { -not (Test-SamePath $_ $worktree) } |
        Where-Object { $other = Get-BuildFolder $_; $other -and (Test-SamePath $other $buildFolder) })
    if ((Test-Link (Get-Item -LiteralPath $buildFolder -Force))) {
        Write-Host "Kept its build folder, $buildFolder, which is a link."
    } elseif ($checkoutBuild -and (Test-SamePath $buildFolder $checkoutBuild)) {
        Write-Host "Kept its build folder, $buildFolder, which is the checkout's own."
    } elseif ($sharedWith.Count) {
        Write-Host "Kept its build folder, $buildFolder, which $($sharedWith -join ', ') builds into too."
    } elseif (Test-Inside $buildFolder $worktree) {
        $deleteBuild = $true
    } elseif ($checkoutBuild -and (Test-SamePath (Split-Path $buildFolder) (Split-Path $checkoutBuild)) -and
        (Split-Path $buildFolder -Leaf) -like "target-*") {
        $deleteBuild = $true
    } else {
        Write-Host "Kept its build folder, $buildFolder, which is neither inside it nor a target-* folder beside the checkout's."
    }
}

# Links first, so nothing after this can go through one: node_modules is the checkout's own.
$freed = [int64] 0
$contents = Get-Contents $worktree
foreach ($link in $contents.Links) {
    if ($link -is [IO.DirectoryInfo]) { [IO.Directory]::Delete($link.FullName, $false) } else { $link.Delete() }
    Write-Host "Removed the link $($link.FullName) (what it points to is untouched)."
}

if ($deleteBuild) {
    Write-Host "Deleting its build folder, $buildFolder..."
    $result = Remove-Folder $buildFolder
    $freed += $result.Freed
    Write-Host "Freed $(Format-Size $result.Freed) from the build folder."
    if ($result.InUse.Count) {
        Write-Host "$($result.InUse.Count) files are in use and stayed, so the worktree stays too:"
        $result.InUse | Select-Object -First 20 | ForEach-Object { Write-Host "  $_" }
        exit 1
    }
}

# What's left in it, which git worktree remove deletes.
$rest = Get-Bytes (Get-Contents $worktree).Files
Push-Location $checkout
try {
    & git worktree remove $worktree
    if ($LASTEXITCODE -ne 0) {
        Write-Host "git worktree remove failed; the worktree stays."
        exit 1
    }
    $freed += $rest
    & git worktree prune
} finally {
    Pop-Location
}
Write-Host "Removed the worktree $worktree. Freed $(Format-Size $freed) in all."

<#
  Saves the yt-dlp output the metadata tests read, in tsundoku\core\tests\fixtures\metadata:
  - <name>.json: yt-dlp -J for a test link, trimmed to the fields Tsundoku reads, run as the app runs it.
  - <name>.txt: what yt-dlp writes to stderr for a link that fails.
  - youtube-live.json: a copy of youtube-watch.json marked as a live stream, since none stays live.
  Run it again when yt-dlp's output changes. It uses the tools in the dev data folder, or those on PATH.
#>
param(
    [string] $ToolsFolder = "D:\Dev\tsundoku\data\tools"
)

$ErrorActionPreference = "Stop"
$fixtures = Join-Path $PSScriptRoot "..\tsundoku\core\tests\fixtures\metadata"
New-Item -ItemType Directory -Force $fixtures | Out-Null

function Find-Tool([string] $folder, [string] $program) {
    $managed = Join-Path $ToolsFolder "$folder\$program.exe"
    if (Test-Path $managed) { return $managed }
    return (Get-Command $program -ErrorAction Stop).Source
}

$ytDlp = Find-Tool "yt-dlp" "yt-dlp"
$deno = Find-Tool "deno" "deno"
$ffmpegFolder = Split-Path (Find-Tool "ffmpeg" "ffmpeg")
Write-Host "yt-dlp $(& $ytDlp --version) at $ytDlp"

# The same arguments as core\src\metadata.rs.
function Invoke-Metadata([string] $url) {
    $stderrFile = New-TemporaryFile
    $json = & $ytDlp --ignore-config --no-playlist --flat-playlist --dump-single-json --encoding utf-8 `
        --js-runtimes "deno:$deno" --ffmpeg-location $ffmpegFolder -- $url 2> $stderrFile
    $stderr = Get-Content $stderrFile -Raw -Encoding utf8
    Remove-Item $stderrFile
    return @{ Json = ($json -join "`n"); Stderr = $stderr; ExitCode = $LASTEXITCODE }
}

# Only the fields metadata.rs reads, so the fixtures stay small and readable.
$videoFields = @(
    "_type", "id", "title", "extractor", "extractor_key", "webpage_url", "webpage_url_domain", "uploader", "channel",
    "duration", "upload_date", "thumbnail", "live_status", "height", "vcodec", "playlist_count"
)
$formatFields = @("format_id", "ext", "height", "width", "fps", "vcodec", "acodec")

function Get-Trimmed($info) {
    $trimmed = [ordered]@{}
    foreach ($field in $videoFields) {
        if ($info.PSObject.Properties.Name -contains $field) { $trimmed[$field] = $info.$field }
    }
    if ($info.formats) {
        $trimmed["formats"] = @($info.formats | ForEach-Object {
            $format = $_
            $kept = [ordered]@{}
            foreach ($field in $formatFields) {
                if ($format.PSObject.Properties.Name -contains $field) { $kept[$field] = $format.$field }
            }
            $kept
        })
    }
    return $trimmed
}

function Save-Json([string] $name, $value) {
    $path = Join-Path $fixtures "$name.json"
    $value | ConvertTo-Json -Depth 6 | Set-Content -Path $path -Encoding utf8NoBOM
    Write-Host "saved $name.json"
}

$videos = [ordered]@{
    "youtube-watch"             = "https://www.youtube.com/watch?v=jNQXAC9IVRw"
    "youtube-short-link"        = "https://youtu.be/jNQXAC9IVRw?si=abc&t=3"
    "youtube-shorts"            = "https://www.youtube.com/shorts/jNQXAC9IVRw"
    "youtube-watch-in-playlist" = "https://www.youtube.com/watch?v=jNQXAC9IVRw&list=PL4lCao7KL_QFVb7Iudeipvc2BCavECqzc"
    "youtube-playlist"          = "https://www.youtube.com/playlist?list=PL4lCao7KL_QFVb7Iudeipvc2BCavECqzc"
    "youtube-channel"           = "https://www.youtube.com/@jawed"
    "archive-org"               = "https://archive.org/details/XD300-23_68HighlightsAResearchCntAugHumanIntellect"
    "soundcloud"                = "https://soundcloud.com/ethmusic/lostin-powers-she-so-heavy"
}
$failures = [ordered]@{
    "unavailable"  = "https://www.youtube.com/watch?v=aaaaaaaaaaa"
    "sign-in"      = "https://vimeo.com/76979871"
    "not-found"    = "https://example.com/nothing-here"
    "unsupported"  = "https://example.com/"
    "no-such-host" = "https://this-host-does-not-exist.invalid/video"
}

foreach ($name in $videos.Keys) {
    $result = Invoke-Metadata $videos[$name]
    if ($result.ExitCode -ne 0) { throw "yt-dlp failed for ${name}: $($result.Stderr)" }
    Save-Json $name (Get-Trimmed ($result.Json | ConvertFrom-Json))
}

$live = Get-Content (Join-Path $fixtures "youtube-watch.json") -Raw | ConvertFrom-Json
$live.live_status = "is_live"
Save-Json "youtube-live" $live

foreach ($name in $failures.Keys) {
    $result = Invoke-Metadata $failures[$name]
    if ($result.ExitCode -eq 0) { throw "yt-dlp didn't fail for $name" }
    Set-Content -Path (Join-Path $fixtures "$name.txt") -Value $result.Stderr.TrimEnd() -Encoding utf8NoBOM
    Write-Host "saved $name.txt"
}

# The failures above set $LASTEXITCODE on purpose; the script itself succeeded.
exit 0

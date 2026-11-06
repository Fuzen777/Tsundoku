<#
  Saves the yt-dlp output the metadata tests read, in tsundoku\core\tests\fixtures\metadata:
  - <name>.json: yt-dlp -J for a test link, trimmed to the fields Tsundoku reads, run as the app runs it.
  - <name>.txt: what yt-dlp writes to stderr for a link that fails.
  - youtube-live.json: a copy of youtube-watch.json marked as a live stream, since none stays live.
  And the listings the listing tests read, in tsundoku\core\tests\fixtures\listing:
  - <name>.json: yt-dlp's flat listing of a channel or playlist, as core\src\listing.rs asks for it, trimmed.
  - instagram-account.json: with -InstagramCookies, gallery-dl's answer for an Instagram account's newest 12 posts
    (the app asks for 50; fewer asks Instagram less), trimmed by Trim-Gallery-Fixtures.py. Instagram needs a signed-in
    browser's cookies, given as gallery-dl's --cookies-from-browser takes them ("firefox:<profile folder>").
  -Capture Metadata or Listings saves only those. Run it again when yt-dlp's or gallery-dl's output changes. It uses
  the tools in the dev data folder, or those on PATH.
#>
param(
    [string] $ToolsFolder = "D:\Dev\tsundoku\data\tools",
    [ValidateSet("All", "Metadata", "Listings")] [string] $Capture = "All",
    [string] $InstagramCookies = ""
)

$ErrorActionPreference = "Stop"
$fixtures = Join-Path $PSScriptRoot "..\tsundoku\core\tests\fixtures\metadata"
$listingFixtures = Join-Path $PSScriptRoot "..\tsundoku\core\tests\fixtures\listing"
New-Item -ItemType Directory -Force $fixtures, $listingFixtures | Out-Null

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

if ($Capture -ne "Listings") {
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
}

# Listings, with the arguments core\src\listing.rs adds: a channel's newest 50 from its Videos tab, a playlist whole,
# with YouTube's rough dates. youtube-channel-tabs is a channel's home page read as any link is, whose entries are its
# tabs (2 videos each here, to keep it small), which can't be listed.
$listings = [ordered]@{
    "youtube-channel"      = @("https://www.youtube.com/@NASA/videos", "--playlist-end", "50")
    "youtube-playlist"     = @("https://www.youtube.com/playlist?list=PLwP_SiAcdui0KVebT0mU9Apz359a4ubsC")
    "youtube-channel-tabs" = @("https://www.youtube.com/@NASA", "--playlist-end", "2")
    "bandcamp-album"       = @("https://nightbringer.bandcamp.com/album/hierophany-of-the-open-grave")
    "soundcloud-set"       = @("https://soundcloud.com/the-concept-band/sets/the-royal-concept-ep")
}
$listingFields = @(
    "_type", "id", "title", "extractor", "extractor_key", "webpage_url", "webpage_url_domain", "uploader", "channel",
    "playlist_count", "thumbnails"
)
$entryFields = @(
    "_type", "ie_key", "extractor_key", "id", "url", "webpage_url", "title", "duration", "timestamp", "upload_date",
    "uploader", "channel", "live_status", "thumbnails"
)
$thumbnailFields = @("id", "url", "width", "height")

function Select-Fields($object, [string[]] $fields) {
    $kept = [ordered]@{}
    foreach ($field in $fields) {
        if ($object.PSObject.Properties.Name -notcontains $field) { continue }
        $value = $object.$field
        if ($field -eq "thumbnails" -and $value) {
            $value = @($value | ForEach-Object { Select-Fields $_ $thumbnailFields })
        }
        $kept[$field] = $value
    }
    return $kept
}

function Save-Listing([string] $name, $value) {
    $path = Join-Path $listingFixtures "$name.json"
    $value | ConvertTo-Json -Depth 8 | Set-Content -Path $path -Encoding utf8NoBOM
    Write-Host "saved listing\$name.json"
}

if ($Capture -ne "Metadata") {
    foreach ($name in $listings.Keys) {
        $url, $rest = $listings[$name]
        $extra = @($rest | Where-Object { $_ })
        $stderrFile = New-TemporaryFile
        $json = & $ytDlp --ignore-config --no-playlist --flat-playlist --dump-single-json --encoding utf-8 `
            --js-runtimes "deno:$deno" --ffmpeg-location $ffmpegFolder `
            --extractor-args "youtubetab:approximate_date" @extra -- $url 2> $stderrFile
        $exitCode = $LASTEXITCODE
        Remove-Item $stderrFile
        if ($exitCode -ne 0) { throw "yt-dlp failed for listing $name" }
        $listing = ($json -join "`n") | ConvertFrom-Json
        $trimmed = Select-Fields $listing $listingFields
        $trimmed["entries"] = @($listing.entries | ForEach-Object { Select-Fields $_ $entryFields })
        Save-Listing $name $trimmed
    }

    if ($InstagramCookies) {
        $galleryDl = Find-Tool "gallery-dl" "gallery-dl"
        $json = & $galleryDl --config-ignore --dump-json --cookies-from-browser $InstagramCookies --post-range 1-12 `
            -- "https://www.instagram.com/instagram/posts/"
        if ($LASTEXITCODE -ne 0) { throw "gallery-dl failed for the Instagram account" }
        Set-Content -Path (Join-Path $listingFixtures "instagram-account.json") -Value ($json -join "`n") `
            -Encoding utf8NoBOM
        & python (Join-Path $PSScriptRoot "Trim-Gallery-Fixtures.py")
    }
}

# The failures above set $LASTEXITCODE on purpose; the script itself succeeded.
exit 0

---
type: Features
---
# Features

> [!abstract]
> What the app does today. The full v1 design is in [PROJECT](./PROJECT.md).

## Window

- **Mica**: Windows 11's backdrop shows through the window, which follows Windows' light or dark theme. Windows 10 gets a plain background.
- **Remembered placement**: the window reopens at its last size and position, maximized if it was.
- **One window**: opening Tsundoku while it is already running brings the open window to the front.

## Navigation

- **Sidebar**: Queue, History and Presets at the top, Settings at the bottom.
- **About**: Settings shows the app's version.

## Adding a Video

- **Add bar**: paste a link and press Enter or Add. A link without `https://` ("youtu.be/…") works too.
- **Drop a link**: dropping a link anywhere on the window, such as one dragged from a browser's address bar, puts it in the add bar and fetches it right away.
- **Preview card**: under the add bar, the video's thumbnail, title, uploader, site, length, upload date and best quality ("1080p", or Audio Only). A spinner shows while yt-dlp fetches it, with Cancel; a new link replaces the one being fetched.
- **Already downloaded**: when the history has the video, the card says when and with which preset, shows the file's path with Open File and Show in Folder, and Download becomes Download Again. If the file was moved or deleted, it says so. The check uses the site and the video's ID, so every link to a video is caught: `youtu.be`, Shorts, a timestamp, a playlist it's in.
- **Refused links**: playlists and channels, live streams, and premieres that haven't started each get a plain message. So do unsupported sites, unavailable videos, videos needing a sign-in or an age check, and network errors, with yt-dlp's own error underneath.
- **Without Deno**: a YouTube preview warns that some formats may be missing.
- **Download**: shown, but not available until the queue arrives.

## History

- **Kept for good**: every finished download is recorded (once downloads exist), with the video's details, the preset, the file's path, size and format, and the date.
- **yt-dlp's archive**: `archive.txt` lists every video in yt-dlp's own format (`youtube jNQXAC9IVRw`), kept up to date, so yt-dlp on the command line skips them too with `--download-archive`.

## Tools

Tsundoku downloads with three free tools: yt-dlp, ffmpeg (with ffprobe) and Deno, which yt-dlp needs for some YouTube formats.

### First-Run Setup

- **Found copies**: the first start looks for each tool on this PC (on PATH, such as winget's or scoop's, or Tsundoku's own from an earlier setup) and shows its version.
- **Use This Copy or Download Fresh**: for each tool found, use that copy or download Tsundoku's own; tools not found are downloaded. Each download shows its size and its progress.
- **Official builds, checked**: downloads come from each tool's official GitHub releases (yt-dlp's own FFmpeg builds for ffmpeg) and are checked against the release's published SHA-256 before they're installed.
- **Antivirus check**: after installing, Tsundoku runs the tool to make sure it's there and works, and says so clearly when Windows Security or an antivirus took it.
- **Deno is optional**: the app opens once yt-dlp and ffmpeg are ready. If Deno's download fails, Continue Without Deno leaves it for later.

### Settings → Tools

- **Each tool's copy**: its version and path, and whether it's Tsundoku's copy or yours.
- **Choose File…**: point a tool at any copy on this PC. Tsundoku checks it's the right program (and that ffprobe is beside ffmpeg).
- **Use Tsundoku's Copy / Download**: switch from your copy to Tsundoku's own, or set up a tool that isn't.
- **Cancel**: stops a download or update in progress, leaving the copy already installed as it was.

### Updates

- **yt-dlp**: Tsundoku's copy updates itself in the background at startup, at most once a day, and on demand with Update Now.
- **Update channel**: Stable or Nightly, which gets site fixes sooner. Switching updates Tsundoku's copy right away, including back from a nightly build to the latest stable one.
- **ffmpeg and Deno**: Check for Updates asks GitHub for a newer build, and Update installs it. They're never updated without asking.
- **Your own copies are never updated** by Tsundoku.

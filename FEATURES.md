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
- **Add bar**: the link field across the top. It can't fetch a link yet.
- **About**: Settings shows the app's version.

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

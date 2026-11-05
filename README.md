# Tsundoku

Tsundoku (積ん読: buying things and letting them pile up unread) is a Windows app that downloads videos with [yt-dlp](https://github.com/yt-dlp/yt-dlp) and remembers everything it has downloaded. Paste a link and it tells you whether you already have that video, when you got it and where the file is, before downloading it again.

## Features

- **Paste a link** (or drop one on the window) to see the video's title, uploader, length, upload date and best quality.
- **A warning when you already have it**, whatever the link: `youtu.be`, Shorts, a timestamp or a playlist it's in all count as the same video.
- **Presets**: Best Video, 1080p MP4, Best Audio and MP3 to start with, each with its own format, file name, folder, what to embed and SponsorBlock, and editable.
- **A queue** with progress, Stop and Resume, Cancel and Retry, two downloads at a time.
- **A searchable history** of every download, with Open File, Show in Folder and Download Again. yt-dlp's own `archive.txt` is kept in sync.
- **The tools taken care of**: yt-dlp, ffmpeg and Deno are found on your PC or downloaded, and kept up to date.
- **Cookies from your browser** for videos that need you signed in, and a speed limit.
- **Updates itself** from this repository's releases.

Single videos only for now: playlists, channels and live streams are planned. See [TODO](TODO.md).

## Install

Download the `-setup.exe` from the [latest release](https://github.com/Fuzen777/Tsundoku/releases/latest) and run it. It installs for your account only, without administrator rights. The installer isn't code-signed yet, so Windows warns about it: choose **More info**, then **Run anyway**.

Requirements: Windows 11, 64-bit. Windows 10 should work, but hasn't been tested.

## Building

Requirements: [Rust](https://rustup.rs/) with the MSVC toolchain, [Node.js](https://nodejs.org/), and Visual Studio's C++ build tools.

```
cd tsundoku
npm install
npm run tauri dev
```

`npm run tauri build` makes the installer. Pushing a version tag builds and publishes a signed release through GitHub Actions (`.github/workflows/release.yml`).

## License

[MIT](LICENSE)

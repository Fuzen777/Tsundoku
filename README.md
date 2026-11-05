# Tsundoku

Tsundoku (積ん読: buying things and letting them pile up unread) is a Windows app that downloads videos with [yt-dlp](https://github.com/yt-dlp/yt-dlp) and remembers everything it has downloaded. Paste a link and it tells you whether you already have that video, when you got it and where the file is, before downloading it again.

**Status:** early development. So far: the tool setup (finding or downloading yt-dlp, ffmpeg and Deno, and keeping them up to date), and pasting a link to preview the video with the warning when you already have it. Downloading comes next.

## Planned for Version 1

- Paste a link to see the video's title, uploader, length and best quality, with a warning if you already have it.
- Download with presets (best video, 1080p MP4, best audio, MP3), each with its own format, file name and folder.
- A queue with progress, stop and resume, cancel and retry.
- A searchable history of every download.
- yt-dlp, ffmpeg and Deno fetched and kept up to date by the app.

## Building

Requirements: Windows 10 or 11, [Rust](https://rustup.rs/) with the MSVC toolchain, [Node.js](https://nodejs.org/), and Visual Studio's C++ build tools. WebView2 is built into Windows 11.

```
cd tsundoku
npm install
npm run tauri dev
```

`npm run tauri build` makes the installer.

## License

[MIT](LICENSE)

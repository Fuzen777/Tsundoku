# Tsundoku

Tsundoku (積ん読: buying things and letting them pile up unread) is a Windows app that downloads videos, posts of images and shared files, and remembers everything it has downloaded. Paste a link and it tells you whether you already have it, when you got it and where the file is, before downloading it again.

## Features

### Adding Links

- **Paste a link** (or drop one on the window) to see what it is before downloading: a video's thumbnail, title, uploader, length, upload date and best quality; a post's caption, author and what it holds ("2 images, 4 videos"); a shared file or folder's name and size ("14 files, 1.2 GB").
- **Many sites, one add bar**: videos and audio from YouTube and well over a thousand other sites, posts of images from Instagram, X, Bluesky and other image sites, and files and shared folders from MEGA, MediaFire, Google Drive, Dropbox, OneDrive, pCloud, WeTransfer, the Internet Archive, Yandex Disk, Box, Icedrive, 1fichier, SwissTransfer, Files.fm and upload.ee. Each link is read by whatever suits it, so there's nothing to choose. Settings → Supported Sites lists every site in one searchable list.
- **Several links at once**: paste or drop text holding several links and they go to the **Pending** tab, where each is read in the background and shows what it is, whether you already have it and whether it's in the queue. **Start All** queues every one you don't have yet, or tick some and start those. Pending links are kept through a restart, and can be grouped by the folder they'll be saved in, their site or where they stand, and sorted by date, title, uploader or site.
- **A warning when you already have it**, whatever the link: `youtu.be`, Shorts, a timestamp or a playlist it's in all count as the same video.

### Downloading

- **Presets** for videos and audio: Best Video, 1080p MP4, Best Audio and Audio MP3 to start with, each with its own quality, format, file name, folder, what to embed and SponsorBlock, and all editable.
- **Posts of images**: every file of a post is saved as the site gives it, in a folder per site. Files already there are skipped, so downloading a post again only fetches what's missing. Instagram posts download one at a time, with a pause before each file so your account isn't blocked.
- **MEGA, MediaFire, Google Drive, Dropbox, OneDrive, pCloud, WeTransfer, the Internet Archive, Yandex Disk, Box, Icedrive, 1fichier, SwissTransfer, Files.fm and upload.ee**: files and shared folders download with their own folder layout, nothing to set up. Each file is checked once it arrives when the host gives a checksum, and a damaged one is downloaded again on Retry.
- **A queue** with progress, Stop and Resume, Cancel and Retry, two downloads at a time (up to five), and an optional speed limit, grouped and sorted as Pending is. It comes back after a restart, and closing while downloading asks first.

### Where Downloads Go

- **One download folder**: everything goes to a Tsundoku folder in your Downloads by default, and Settings can point it elsewhere.
- **Folder rules**: the **Rules** tab decides where downloads go: "when all of these match, save in this folder", checking the site, uploader, title, link, kind (video, audio, post or shared file) or preset. A condition can also match a regular expression. A folder can use placeholders such as `{site}`, `{uploader}`, `{account}`, `{playlist}`, `{title}`, `{year}` or `{added}`, such as `D:\Archive\{site}\{uploader}`, and the text a pattern captures, as `{1}`. Every install starts with a last rule that gives each site's downloads a folder of their own, unless another rule or a preset's own folder says otherwise.
- **Shown before you download**: the preview and each link in Pending show the folder a download will be saved in and what picked it: a rule, the preset or the download folder.

### History

- **Every download in a searchable list**, with site and preset filters and sorting. A post lists every file it saved.
- **Open File, Show in Folder, Download Again** and **Forget**, which lets a video be downloaded again without the warning. A file that was moved or deleted is marked as missing.
- **A standard archive file**: `archive.txt` lists every downloaded video in yt-dlp's archive format, so command-line downloads with `--download-archive` skip them too.

### Signed-In Sites

- **Cookies from your browser**, for videos and posts that need you signed in or an age check. Set to Automatic, Tsundoku finds the browser profiles on your PC and uses, for each link, the one signed in to its site. It only checks which cookies exist, never what they hold.
- **Plain advice when a site needs you signed in**: the preview says which site and whether a browser is signed in to it, with Try Again once you've signed in.

### Taken Care Of

- **The tools**: Tsundoku relies on free tools to download, which it finds on your PC or downloads from their official releases, checks against their published checksums and keeps up to date. Image downloads are optional and can be set up from Settings or straight from a post's preview.
- **Fixes on failures**: when a site changes and its links stop working, the error offers an update right there.
- **Updates itself** from this repository's releases.
- **Your data in one place**: the history, settings and tools live in one folder, which Settings → About opens.
- **Fits Windows**: Windows 11's Mica backdrop, light and dark themes that follow Windows unless you pick one, and the window's size and position remembered.

One video or post at a time for now: playlists, channels, profiles and live streams are refused with a plain message. Picking videos from a playlist is planned, along with a clipboard watcher. See [TODO](TODO.md) for what's next and the [changelog](CHANGELOG.md) for what each version added.

## Install

Download the `-setup.exe` from the [latest release](https://github.com/Fuzen777/Tsundoku/releases/latest) and run it. It installs for your account only, without administrator rights. The installer isn't code-signed yet, so Windows warns about it: choose **More info**, then **Run anyway**.

New features land in this repository before they reach a release, so the latest release may not have everything listed above yet.

Requirements: Windows 11, 64-bit. Windows 10 should work, but hasn't been tested.

## Building

Requirements: [Rust](https://rustup.rs/) with the MSVC toolchain, [Node.js](https://nodejs.org/), and Visual Studio's C++ build tools.

```
cd tsundoku
npm install
npm run tauri dev
```

`Scripts\Check.cmd` runs every check: Rust formatting, Clippy, the Rust tests, then TypeScript, ESLint and the UI tests. `npm run tauri build` makes the installer. Pushing a version tag builds and publishes a signed release through GitHub Actions (`.github/workflows/release.yml`).

## Thanks

Tsundoku is built with [Tauri](https://tauri.app/) and downloads with [yt-dlp](https://github.com/yt-dlp/yt-dlp), [gallery-dl](https://codeberg.org/mikf/gallery-dl), [FFmpeg](https://ffmpeg.org/) (through [yt-dlp's builds](https://github.com/yt-dlp/FFmpeg-Builds)) and [Deno](https://deno.com/).

## License

[MIT](LICENSE)

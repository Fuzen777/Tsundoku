# Changelog

All notable changes to Tsundoku. Versions follow [Semantic Versioning](https://semver.org/): every change bumps the patch number, and a release bumps the minor or major number, folding in the patch entries since the previous release.

## 0.1.1 · 2026-11-05

### Fixed

- **Dropdowns in dark mode**: the lists of Settings', the history's, the preset editor's and the preview card's dropdowns were white with faint light text in dark mode. They now use the theme's background and text colors.

## 0.1.0 · 2026-11-05

The first release.

### New

- **Install and update**: a per-user installer, so no administrator rights are needed. Tsundoku checks GitHub for a newer version at startup and offers to install it, and Settings → About checks on demand.
- **Tools**: the first start finds yt-dlp, ffmpeg and Deno on this PC and offers to use them, or downloads the official builds, checked against their published SHA-256. Settings → Tools shows each one's version and source, uses any copy you pick, and updates Tsundoku's own: yt-dlp at most once a day at startup or with Update Now, on the Stable or Nightly channel; ffmpeg and Deno when you check.
- **Preview**: paste a link, or drop one on the window, to see the video's thumbnail, title, uploader, site, length, upload date and best quality. Playlists, channels, live streams and premieres are refused with a plain message, as are unsupported sites, unavailable videos and videos needing a sign-in, with yt-dlp's error underneath.
- **Duplicate warning**: a video downloaded before shows when, with which preset and where the file is, with Open File and Show in Folder, whatever the link (`youtu.be`, Shorts, a timestamp, a playlist it's in). Download Again with the same preset asks before replacing the file.
- **Downloads**: pick a preset and Download. The queue runs two at a time, with each one's stage, size, speed and time left, and Stop, Resume, Cancel, Retry, Open File, Show in Folder and Open Log. It comes back after a restart, and closing while downloading asks first.
- **Presets**: Best Video, 1080p MP4, Best Audio and Audio MP3 to start with, and an editor for the quality, container or audio format, file name, folder, embedding, SponsorBlock and extra yt-dlp arguments, showing the arguments it gives.
- **History**: every download in a searchable list, with site and preset filters, sorting, each video's downloads when it has several, Open Source Page, Download Again and Forget. yt-dlp's `archive.txt` is kept in sync.
- **Settings**: downloads at once, a speed limit, the default preset, and a browser to read cookies from for videos that need a signed-in account.
- **Fixes on failures**: Update yt-dlp or Try Nightly when a site's page couldn't be read, Set Up Cookies when a video needs a signed-in account.
- **Window**: Windows 11's Mica backdrop, light and dark themes following Windows, its last size and position remembered, and a second start bringing the open window to the front.

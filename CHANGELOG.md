# Changelog

All notable changes to Tsundoku. Versions follow [Semantic Versioning](https://semver.org/): every change bumps the patch number, and a release bumps the minor or major number, folding in the patch entries since the previous release.

## 0.0.2 · 2026-11-05

### New

- First-run setup: Tsundoku finds yt-dlp, ffmpeg and Deno on this PC and offers to use them, or downloads the official builds into its own folder, checked against their published SHA-256, with each download's size and progress.
- Settings → Tools: each tool's version, path and source, Choose File… to use any copy, and switching to Tsundoku's own copy.
- yt-dlp updates: at startup at most once a day and with Update Now, on the Stable or Nightly channel. ffmpeg and Deno: Check for Updates, and Update when a newer build exists.
- Settings are saved in `settings.json` in the data folder.

## 0.0.1 · 2026-11-05

### New

- The app's window, with Windows 11's Mica backdrop and light and dark themes following Windows. It reopens at its last size and position, and opening Tsundoku again brings it to the front.
- Navigation between Queue, History, Presets and Settings, with the link field across the top (it can't fetch links yet). Settings shows the app's version.

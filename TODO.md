---
type: TODO
rules:
  top-level-categories:
    - Quick Wins
    - Features
    - Concerns
    - Fixes
    - Testing
    - Features in Consideration
  sub-categories: Group tasks by the feature or part of the app they touch. When a task needs another one, the prerequisite comes first.
  quick-wins: Simple, uncontentious tasks that fit in one pass. This category takes precedence over the others, except Concerns and Features in Consideration, which need their category for context. Its only sub-categories are Features and Fixes, and entries about neighbouring components sit next to each other.
---
# Tsundoku TODO

> [!info]
> v1 itself is specified in [PROJECT](./PROJECT.md). This list holds what comes after it.

## Features

### Playlists and Channels

- [ ] **Playlist entry picker.** Pasting a playlist or channel link lists its videos (`--flat-playlist`), with the ones already downloaded greyed out and unchecked, and queues the checked ones. This lifts v1's single-video rule. From here yt-dlp also reads `archive.txt` (`--download-archive`), so it skips known videos itself.
- [ ] **Channel subscriptions.** Watch a channel or playlist, check it on a schedule, and queue only new uploads (`--break-on-existing` with the archive). Builds on the picker's playlist handling.

### Getting Links In

- [ ] **Tray icon and clipboard watcher.** Closing the window keeps Tsundoku running in the tray, and copying a video link offers to queue it.
- [ ] **Browser send to app.** A browser button or right-click entry that queues the current page, through a `tsundoku://` link (Tauri's deep-link plugin) or a local endpoint.

---

## Testing

### Tools

- [ ] **Choose File… in Settings → Tools.** Pick a copy of each tool through Windows' file dialog, including a wrong program and an ffmpeg without ffprobe beside it. The checks behind it are tested; the dialog itself wasn't driven.
- [ ] **An antivirus taking a tool.** A quarantined or blocked yt-dlp should give the "Windows Security or your antivirus blocked or removed it" message, at setup and when it's already set up. Hard to cause on purpose.

---

## Features in Consideration

### History

- **Missing-file tracking.** Notice when a downloaded file was deleted or moved, mark it in the history, and offer to download it again.
- **Cross-site fuzzy matching.** Flag a video with the same title and length (±2 s) from another site or under another ID, such as a repost. Can give false positives.
- **Import an existing library.** Fill the history from an existing `archive.txt`, and from files on disk whose name holds an `[id]` or whose tags hold the source URL written by `--embed-metadata`.

### Downloads

- **Live streams and premieres.** Record live streams (`--live-from-start`) and wait for upcoming premieres (`--wait-for-video`) instead of refusing them.
- **Clips and chapters in the UI.** Download a time range (`--download-sections`) or split by chapter (`--split-chapters`) from the preview card rather than through extra arguments.

### Distribution

- **macOS and Linux builds.** Per-OS tool downloads and installers, and testing on both.
- **Windows code signing,** so SmartScreen stops warning on install.

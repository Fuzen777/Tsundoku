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

### Look

- [ ] **A designed app icon.** The current one is a placeholder: three books piled up, drawn in `tsundoku/src-tauri/icons/source.svg`, from which `npm run tauri icon` makes every size.

### Playlists and Channels

- [ ] **Playlist entry picker.** Pasting a playlist or channel link lists its videos (`--flat-playlist`), with the ones already downloaded greyed out and unchecked, and queues the checked ones. This lifts v1's single-video rule. From here yt-dlp also reads `archive.txt` (`--download-archive`), so it skips known videos itself.
- [ ] **Channel subscriptions.** Watch a channel or playlist, check it on a schedule, and queue only new uploads (`--break-on-existing` with the archive). Builds on the picker's playlist handling.

### File Hosts

- [ ] **Google Drive and Dropbox.** The other two of the big four file hosts, after MEGA and MediaFire, as hosts of their own in `core::hosts`.
- [ ] **Password-protected links.** MEGA's `#P!` links and MediaFire's protected files are refused with a message for now.

### Getting Links In

- [ ] **Clipboard watcher.** Off by default. Every copied link goes to the tools to ask whether they can read it, with yt-dlp's generic extractor off, so an unsupported link (a Google Doc, a shop) is turned down in under a second without being visited; direct image links (…/photo.jpg) are ignored. Supported links go to Pending, or straight to the queue with Download Right Away on; ones the history already has are dropped silently (Fuzen's choice: downloading again is the exception, done by hand). A switch in the Pending view pauses it. Builds on the Pending tab.
- [ ] **Download Right Away.** Off by default: links the watcher catches start with the default preset once read, as Start does for a link not read yet (`PendingLinks::start`). Not "auto-start", which reads like starting with Windows.
- [ ] **Tray icon.** Closing the window keeps Tsundoku running in the tray, with Watch Clipboard, Start Pending, Open and Quit in its menu, and Start with Windows. Wanted by Fuzen; without it the watcher only works while the window is open.
- [ ] **Clipboard shortcut.** A global shortcut (such as Ctrl+Alt+D) that adds the link on the clipboard, for when the watcher is off.
- [ ] **Browser send to app.** A browser button or right-click entry that queues the current page, through a `tsundoku://` link (Tauri's deep-link plugin) or a local endpoint.

---

## Testing

### Tools

- [ ] **Choose File… in Settings → Tools.** Pick a copy of each tool through Windows' file dialog, including a wrong program and an ffmpeg without ffprobe beside it. The checks behind it are tested; the dialog itself wasn't driven.
- [ ] **An antivirus taking a tool.** A quarantined or blocked yt-dlp should give the "Windows Security or your antivirus blocked or removed it" message, at setup and when it's already set up. Hard to cause on purpose.

### Preview

- [ ] **Dropping a link from a browser.** Drag a link, or the address bar's padlock, from Firefox, Chrome and Edge onto the window. The drop handler was tested with a synthetic drop, not a real drag between apps.
- [ ] **Open File and Show in Folder** on the duplicate warning and the queue, and **Open Log**: that they open the right file, folder and log. Their errors (file gone, unknown download) are tested.

### Install and Updates

- [ ] **Updating from one release to the next**: the banner's Install and Restart, end to end. It needs two published releases and the repository public, since the updater can't read a private repository's releases.
- [ ] **Windows 10**: the installer, the plain background without Mica, and WebView2 being downloaded where it's missing.

### Downloads

- [ ] **The confirmation dialogs**: closing while downloading, Download Again with the same preset, deleting a preset, leaving a preset with unsaved changes. They're Windows' own dialogs, which the automated checks can't click.
- [ ] **Cancelling during a merge**: that ffmpeg ends with yt-dlp and leaves no locked file. Ending the whole process tree is tested with cmd and ping, and Cancel while downloading was tried for real.
- [ ] **Update yt-dlp from a failure**: the button under a "couldn't read this site's page" error. It's the same update as Settings → Tools, but no site failed that way during testing.
- [ ] **Cookies from Firefox**: a video that needs a signed-in account, previewed and downloaded with Settings → Cookies on Firefox. The arguments reach yt-dlp (seen in a log), but no such video was tried.
- [ ] **A site other than YouTube** downloading end to end, such as archive.org or SoundCloud, and a video with subtitles and SponsorBlock segments.

---

## Features in Consideration

### History

- **Missing-file tracking.** Notice when a downloaded file was deleted or moved, mark it in the history, and offer to download it again.
- **Cross-site fuzzy matching.** Flag a video with the same title and length (±2 s) from another site or under another ID, such as a repost. Can give false positives.
- **Import an existing library.** Fill the history from an existing `archive.txt`, and from files on disk whose name holds an `[id]` or whose tags hold the source URL written by `--embed-metadata`.

### Downloads

- **Live streams and premieres.** Record live streams (`--live-from-start`) and wait for upcoming premieres (`--wait-for-video`) instead of refusing them.
- **Clips and chapters in the UI.** Download a time range (`--download-sections`) or split by chapter (`--split-chapters`) from the preview card rather than through extra arguments.

### Images

- **Image posts through gallery-dl.** Add gallery-dl as a fourth tool and send image-first sites (Instagram, X, Pixiv…) to it by domain, the rest to yt-dlp, so Instagram photo posts and carousels can be downloaded. One post per link first, profiles later; a download becomes a folder and its file list. See the [feasibility review](https://claude.ai/artifact/CaFPJUSLnVixQCc3WccNm5).
- **Duplicate images by hash.** Store each downloaded image's SHA-256 to catch identical files, and a perceptual hash (`image_hasher`) to catch reposts, resizes and re-compressed copies, as Hitomi Downloader's duplicate finder does. A later addition: a hash exists only once the file is downloaded, so it can flag or discard a duplicate but not prevent the download, which stays the post ID check's job. Needs image posts above.

### Distribution

- **macOS and Linux builds.** Per-OS tool downloads and installers, and testing on both.
- **Windows code signing,** so SmartScreen stops warning on install.

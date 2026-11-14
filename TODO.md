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

## Priority

- [x] Tools ⇒ Check all for update
- [x] Fix default RMB ⇒ It still uses some kind of default that make no sense in our app.

- [x] Check and update all tools instead of one by one.

- [x] Per item destination override

- [x] Move already downloaded files

- [x] In app log + viewer so we avoid depending on external programs ? Need to make sure the app remains lightweight.

- [x] Pending : when "Video Download as" match batch, show "default" on individual entries.

- [x] Group by : change dropdown for buttons. Group by Status is its own dimension. Site/Folder/Rule (Rule is new) is a segmented picker. Rule shows Rule number and Destination. (So for instance Downloads\\{site} shows Downloads.) Groups should be collapsible.

- [x] History : only missing ⇒ add only existing

- [x] Add, remove, duplicate and move presets, default can’t be deleted and is always first, it can be edited.

- [x] Pending and Queue ⇒ Group + Sort so we can group by destination and sort by recency for instance.
- [x] Manual select of Dark/Light/Auto

### Aesthetics

- [x] Use Icons instead of Text where it makes sense (accept, reject…) with hover text.
- [x] RJ-Library inspired dark mode
- [ ] Light mode revamp that takes its roots from the dark mode above
- [x] Accent color picker

## Features

### Look

#### Not Now

- [ ] **A designed app icon.** The current one is a placeholder: three books piled up, drawn in `tsundoku/src-tauri/icons/source.svg`, from which `npm run tauri icon` makes every size. Do once the app is feature complete.

### Playlists and Channels

- [x] **Playlist entry picker.** Pasting a YouTube channel or playlist, or an Instagram account, lists its items in the preview card, with the ones already downloaded greyed out and unticked, and queues the ticked ones. Phase 1 of Watched Links. Downloads don't read `archive.txt` (`--download-archive`), which would block Download Again.
- [ ] **Watched Links** (was Channel subscriptions). Phase 2, the Watched tab with Check Now, is in 0.1.79. Phases 3 and 4: checked on a schedule, queuing only what's new (`listing::before_known_ground`, which stops past the pinned posts), with a folder of their own and notifications. Shorts and past live streams come with them; X accounts are listed and watched since 0.1.134.

### File Hosts

#### Not Now

- [ ] **Google Docs, Sheets and Slides.** Left out of Drive folders for now. They could download as .docx, .xlsx and .pptx through `docs.google.com/KIND/d/ID/export?format=...`.
- [ ] **Password-protected links on file hosts.** Videos with a password (yt-dlp's `--video-password`) are done, and every part a host needs is in place (`HostFacts::unlock`). Each host waits for a real link that needs a password to test it with: MEGA's `#P!` links are decoded already (in `D:\Dev\tsundoku\scripts\passwords\mega-unlock.patch`, checked against public links that have since expired), and how MediaFire, 1fichier, pCloud, Box and TransferNow take a password is known. OneDrive, WeTransfer, SwissTransfer and Filemail need their pages read first.

### Getting Links In

- [ ] **Pick any clipboard shortcut.** Settings offers four fixed key combinations; recording one the user presses would fit any keyboard.
- [ ] **Shortcut taken at startup.** A clipboard shortcut another app took since it was picked fails quietly at startup; Settings could say so.

#### Not Now

- [ ] **Browser extension.** A toolbar button and a right-click Send Link to Tsundoku entry for a link on the page, through the same `tsundoku://add?url=…` links as the bookmark. Firefox-family browsers (Zen) need it signed by Mozilla.

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

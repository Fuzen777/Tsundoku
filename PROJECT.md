---
type: General Documentation
---
# Tsundoku

> [!abstract]
> A Windows desktop app (Tauri 2) that downloads videos with yt-dlp and remembers everything it has downloaded. Paste a link and it shows what the video is, warns you if you already have it (when, with which preset, and where the file went), then downloads it with the preset you pick. It's meant to be shared, so it fetches and updates yt-dlp, ffmpeg and Deno itself. The name, 積ん読, is the habit of buying things and letting them pile up unread.

## Goal

Existing yt-dlp GUIs put a window on top of the command line but don't keep track of what was already downloaded, so the same video ends up downloaded twice. yt-dlp's own download archive (`--download-archive`) skips known videos, but it stores only a site and an ID per line: it can't tell you what the video was, when you got it or where the file went, and GUIs rarely expose it. Tsundoku keeps a searchable history of every download, checks it before anything joins the queue, and keeps yt-dlp's archive in sync with it.

## Resolved Decisions

| Decision | Choice | Rationale |
| --- | --- | --- |
| Stack | Tauri 2: React + TypeScript UI, Rust core | User's choice. WinUI 3 felt awkward to build with; Tauri gives a small installer, signed self-updates and full CSS freedom, and React is already familiar |
| Running yt-dlp | `yt-dlp.exe` as a separate process, never as a library | yt-dlp updates without an app release, cancelling ends the process, and a crash can't take the app down |
| Platforms | Windows first, with code kept portable | User's choice |
| Audience | Shareable | User's choice |
| Duplicate detection | Exact match on site and video ID | User's choice |
| History storage | SQLite is the source of truth; the app keeps yt-dlp's `archive.txt` in sync from it | A rich history, plus yt-dlp's own skipping once playlists arrive |
| Playlist and channel links in v1 | Refused until the playlist picker exists | User's choice |
| Later features | Playlist picker, channel subscriptions, tray icon with clipboard watcher, browser send to app | User's choice: wanted, not in v1. Listed in [TODO](./TODO.md) |
| Name | Tsundoku | User's choice |
| Repository | Its own git repository from the first commit, private on GitHub until it's ready to share | User's choice: version tags and the updater's GitHub releases work from day one, with no history move later |
| Code layout | All code in a `tsundoku/` subfolder, docs and scripts at the project root | User's choice, as in RJ Library and Poids Vesta Venus |

## Assumptions

> [!info]
> Defaults taken without asking. Any of them can be overridden.

### Stack and Structure

1. Tauri 2 with React, TypeScript and Vite. The Rust core owns downloads, the history, the tools and settings; the UI only presents and calls the core's commands.
2. Core logic lives in modules with no Tauri dependency, with unit tests, as Icon Picker splits Core from App.
3. SQLite through `rusqlite` in the core, not Tauri's SQL plugin from the UI, so the history has a single owner.
4. English UI, with every string in one file from day one so translations can come later.
5. Windows 11 first; Windows 10 untested.
6. Rust's build output (several GB) and the test download folder stay out of Google Drive, under `D:\Dev\tsundoku`, as Icon Picker's build output does.
7. The Coding monorepo ignores the project's folder, as it does Icon Picker's.
8. Per-user NSIS installer, so no admin rights are needed. App updates go through Tauri's updater, signed with Tauri's own key. No Windows code signing in v1, so SmartScreen warns on first install.

### Tools

1. Three tools: yt-dlp (official release), ffmpeg and ffprobe (yt-dlp's own FFmpeg builds), and Deno (official release). Since 2025.11.12, yt-dlp needs a JavaScript runtime for full YouTube support, and Deno is the one it recommends.
2. They live in the app's data folder under `tools\`. The first run finds copies already on PATH (such as winget's) and offers to use them; otherwise it downloads the official ones. Settings can point each tool at any file.
3. Downloaded at runtime, never bundled: the installer stays small, and the app never redistributes ffmpeg, which is GPL.
4. yt-dlp is checked for updates at startup (at most once a day) and on demand. The Stable channel is the default; Nightly is offered in Settings for faster site fixes.
5. ffmpeg and Deno updates are offered, never automatic, since they rarely break anything.
6. Tools the user picked (such as winget's) are never updated by the app; Settings shows their version.

### Downloads

1. One yt-dlp process per download, two at a time by default.
2. Processes start without a console window, inside a Windows job object so that ending one also ends the ffmpeg it started.
3. Progress comes from `--newline --progress-template` as one JSON line per update, and the final path from `--print after_move:filepath`.
4. Every download gets `--no-playlist`, so a `watch?v=…&list=…` link downloads just that video.
5. Stop keeps the partial files so Resume continues where it left off (`--continue`); Cancel deletes them.
6. Each download keeps yt-dlp's full output as a log, kept for 30 days.
7. Cookies are off by default. Settings can pick a browser for `--cookies-from-browser`: Firefox works, while Chrome and Edge are blocked on Windows by their app-bound encryption.

### History

1. A video counts as downloaded once yt-dlp has finished and moved the file into place. Failed, stopped and cancelled downloads aren't recorded.
2. Videos are keyed by site and ID exactly as yt-dlp writes them in its archive (`youtube dQw4w9WgXcQ`), so `archive.txt` stays in yt-dlp's format and works with yt-dlp on the command line.
3. The check runs on the site and ID returned by the preview's metadata, never on the URL, so every form of a link to the same video is caught.
4. Forget removes a video from the history and from `archive.txt` so it can be downloaded again; the file stays where it is.

### Presets

1. Four built-in presets:
   1. **Best Video**, the default: best video and audio in any codec, merged into MKV with no re-encoding.
   2. **1080p MP4**: up to 1080p, preferring MP4 and M4A, merged into MP4 for compatibility.
   3. **Best Audio**: the best audio stream in its own format (usually Opus or M4A).
   4. **Audio MP3**: the best audio converted to MP3 at the best quality.
2. Each preset holds a format choice, a container, a file name template, a destination folder, what to embed (metadata, thumbnail, chapters, subtitles with their languages), SponsorBlock (off, mark or remove) and extra arguments.
3. Built-ins can be edited, duplicated and reset. Any preset can be the default.
4. The file name template is `%(title).150B [%(id)s].%(ext)s` by default: the title is trimmed to stay under Windows' path limit, and the ID in the name lets a later import recognise the file.
5. Video presets save to `Videos` and audio presets to `Music` by default.
6. Extra arguments go to yt-dlp after the app's own. The few that would break progress, history or the single-video rule (`--yes-playlist`, `--download-archive`, `--print`, `--progress-template`…) are refused.

## Scope

### In Scope

- The add bar: paste a link or drop one onto the window.
- The preview card: video details, the duplicate warning, the preset picker and Download.
- The queue: progress, Stop and Resume, Cancel, Retry, logs, and the limit on downloads at once.
- The history: search, filters, sorting, opening files and folders, Download Again, Forget.
- The preset editor.
- Settings.
- First-run tool setup and tool updates.
- App updates through Tauri's updater.

### Out of Scope

- Playlist and channel links, channel subscriptions, the tray icon and clipboard watcher, and browser send to app: all planned in [TODO](./TODO.md).
- Missing-file tracking, cross-site fuzzy matching, and importing an existing archive or folder: not chosen for now.
- Live streams and upcoming premieres, which are refused with a message.
- Clipping time ranges and splitting by chapter through the UI. Extra arguments can still pass `--download-sections` or `--split-chapters`.
- Signing in with a username and password; only browser cookies.
- macOS and Linux builds, languages other than English, Windows code signing, and a portable mode.

---

## Architecture

```text
Tsundoku/                   its own git repository
├─ PROJECT.md, TODO.md…     docs, and Scripts/, at the project root
└─ tsundoku/                code only
   ├─ src/                  React + TypeScript UI
   ├─ core/                 the Rust core, with no Tauri dependency
   └─ src-tauri/            the Tauri app: window, plugins, commands
```

[AGENTS](./AGENTS.md) has the full layout.

### Core Components

| Component | Responsibility |
| --- | --- |
| Tool manager | Finds, downloads, verifies and updates yt-dlp, ffmpeg and Deno, and reports their versions |
| Metadata fetcher | Runs `yt-dlp -J --no-playlist` on a link and returns the preview; refuses playlists, channels and live streams |
| Download queue | Order, the limit on downloads at once, Stop, Resume, Cancel and Retry; restored after a restart |
| yt-dlp runner | Turns a preset into arguments, starts yt-dlp in a job object without a console, reads its progress lines and the final path |
| History | SQLite videos and downloads: duplicate check, search, Forget; keeps `archive.txt` in sync |
| Presets | Built-in and user presets, and their translation into yt-dlp arguments |
| Settings | Stored as JSON in the data folder |

### UI Components

| Component | Responsibility |
| --- | --- |
| Add bar and preview card | Link input, metadata preview, duplicate warning, preset picker |
| Queue view | Running, waiting, stopped and failed downloads |
| History view | Searchable table of every download |
| Presets view | Preset list and editor |
| Settings view | Tools, downloads, cookies |
| First-run setup | Finding or downloading the tools |

## Data and State

1. **Data folder** (`%LocalAppData%\Tsundoku`): `tsundoku.db`, `archive.txt`, `settings.json`, `tools\` and `logs\`.
2. **`videos` table:** site (yt-dlp's extractor key) and video ID, unique together, plus source URL, title, uploader, length, upload date, thumbnail URL, and when it was first downloaded.
3. **`downloads` table:** one row per finished download of a video, with the preset, file path, size, format (resolution and codecs) and date. A video downloaded twice (as video, then as audio) has two rows.
4. **`archive.txt`:** one line per video in yt-dlp's format, rewritten by the app after each change. In v1 the app is its only writer and does the duplicate check itself; yt-dlp starts reading it (`--download-archive`) when playlists arrive.
5. **Queue state:** waiting and stopped downloads are saved so they come back after a restart. Running ones come back stopped.
6. **`settings.json`:** tool paths and update channel, the limit on downloads at once, rate limit, cookies browser, the presets and which one is the default, and window size and position.

## Behavior and UX

### Layout

1. The add bar runs across the top.
2. Navigation on the left: Queue, History, Presets, Settings.
3. The preview card opens under the add bar when a link is fetched.

### Adding a Video

1. Paste a link into the add bar or drop one onto the window, then press Enter or Add.
2. The preview card fetches the metadata with a spinner and Cancel, then shows the thumbnail, title, uploader, length, upload date and the best available quality.
3. Refused with a plain message:
   1. Playlist and channel links: "Playlists aren't supported yet. Paste a single video."
   2. Live streams and upcoming premieres.
   3. Unsupported sites and unavailable videos, with yt-dlp's error turned into a readable sentence.
4. **Already downloaded:** a warning on the card shows the last download's date, preset and file path, with Open File and Show in Folder. Download becomes Download Again.
5. **Already in the queue:** the card says so, with no Download button.
6. Pick a preset (the default is preselected) and click Download. The video joins the queue and the add bar clears.

### Queue View

1. One row per download: thumbnail, title, preset, stage, progress bar, size, speed and time left.
2. Stages: Waiting, Downloading Video, Downloading Audio, Merging, Embedding, Done, Stopped, Failed.
3. Row actions: Stop or Resume, Cancel, Retry (failed only), Open Log, and for finished rows Open File and Show in Folder.
4. Finished rows stay until Clear Finished.
5. Closing the app while downloads are running asks first.

### History View

1. A table with thumbnail, title, uploader, site, length, preset, download date and size, newest first.
2. Search across titles and uploaders, filters by site and preset, and sorting on any column.
3. Row actions: Open File, Show in Folder, Open Source Page, Download Again, and Forget (asks first).
4. A video downloaded more than once expands to show each download.

### Presets View

1. The preset list, with Set as Default, Duplicate, Delete, and Reset for built-ins.
2. The editor holds every field from the Presets assumptions, with the raw yt-dlp arguments it produces shown underneath.

### Settings View

1. **Tools:** each tool's path and version, Update Now, and the yt-dlp update channel.
2. **Downloads:** downloads at once, rate limit, default preset.
3. **Cookies:** none, or a browser to read them from.

### First Run

1. A setup screen lists the three tools. For each one found on PATH it offers Use This Copy or Download Fresh; missing ones are downloaded, with their size and progress shown.
2. Downloads can't start until yt-dlp and ffmpeg are ready. Without Deno, YouTube previews warn that some formats may be missing.

### Look

1. Mica backdrop, with light and dark themes following Windows.
2. A DESIGN.md is written along with the first screens.

## Edge Cases and Risks

1. **Sites break yt-dlp:** a failure with an extractor error suggests updating yt-dlp, or switching to Nightly.
2. **Same video, another link:** `youtu.be`, Shorts, timestamps and tracking parameters are all caught, since the check uses the metadata's site and ID.
3. **Same video, another ID or site** (a re-upload, a repost): not caught, since matching is exact only.
4. **Same video in two presets:** the warning shows. Download Again records a second download of the same video.
5. **Download Again with the same preset** would produce the same file name, so it asks before overwriting (`--force-overwrites`).
6. **The file already exists on disk but isn't in the history** (downloaded outside the app): yt-dlp skips the download, and the app records the existing file in the history. Check how yt-dlp reports the path in this case while building.
7. **Moved or deleted files:** the history keeps the old path, and Open File says the file is missing.
8. **Cancelling during a merge:** the job object also ends ffmpeg, so no stray process keeps the file locked.
9. **Age-restricted or members-only videos:** the error points to the cookies setting.
10. **Antivirus:** yt-dlp's exe is sometimes flagged and quarantined. The first run and updates check that the file is really there and say so clearly when it isn't.
11. **Unsigned installer:** SmartScreen warns until the app is code-signed.

## Open Questions

None.

---
type: Agent Instructions
title: AGENTS
---

## Introduction

- Tsundoku is a Windows desktop app (Tauri 2: a React + TypeScript UI over a Rust core) that downloads videos with yt-dlp and keeps a history of everything it has downloaded, so it can warn before a duplicate.
- [PROJECT.md](./PROJECT.md) holds the original spec and its decisions, [TODO.md](./TODO.md) the features planned after v1.

## Rules

- All UI text lives in `tsundoku/src/strings.ts`, in English. Never write UI text in a component.
- UI labels use Title Case: buttons, toggles, navigation, headings, dialog titles and buttons, placeholders, column headers and short names. Minor words (a, an, the, and, or, of, to, in, on, for, with, as, at, by, from) stay lowercase unless first or last, and hyphenated words take both capitals. Sentences (descriptions, messages, empty-state explanations, status texts) stay in sentence case. See [DESIGN](./DESIGN.md).
- `tsundoku-core` (`tsundoku/core`) has no Tauri dependency. Downloads, the history, the tools, presets and settings go there, with unit tests; the app crate (`tsundoku/src-tauri`) only exposes them as commands and events, and the UI only presents.
- Types the UI reads are Rust types in the core deriving `ts_rs::TS` with `#[ts(export)]`; `cargo test` writes them to `tsundoku/src/bindings`. Never hand-write a TypeScript type that mirrors a Rust one, and commit the regenerated files with the change.
- `tsundoku/src/api.ts` is the only place that calls `invoke` or another Tauri API.
- Before a visual change, read [DESIGN](./DESIGN.md). If something differs notably from it, tell the user.
- Never suppress, ignore or work around an ESLint, Clippy or TypeScript error or warning; fix the cause.
- Meta files (docs, scripts) go at the project root; `tsundoku/` holds code only.
- After implementing a feature, update [FEATURES](./FEATURES.md) and remove the item from [TODO](./TODO.md) if it is listed.
- Every change bumps the patch version and gets an entry in [CHANGELOG](./CHANGELOG.md); a commit that bumps the version is tagged `vX.Y.Z`. A release bumps the minor or major version and folds in the patch entries since the previous release.

## Layout

| Path | Contents |
| --- | --- |
| `tsundoku/src` | The React UI: `views/` (with `views/settings/`), `components/`, `strings.ts`, `api.ts`, `queries.ts` (React Query hooks), `standIns.ts` (what `api.ts` returns in a browser), `toolProgress.ts` (live install and update stages), `usePreview.ts` (the preview card's state, and link drops), `errors.ts` (the core's errors → sentences), `format.ts` (sizes, lengths, dates), `bindings/` (generated) |
| `tsundoku/core` | `tsundoku-core`: the UI-free Rust logic and its tests. `process` runs external programs, `tools` finds, installs and updates them, `metadata` asks yt-dlp what a link is, `history` is the SQLite history and `archive.txt`, `preview` combines the last two for the preview card, `settings` and `data_folder` hold what's stored. `tests/fixtures/metadata` holds real yt-dlp answers |
| `tsundoku/src-tauri` | The Tauri app: window, plugins, `tools.rs` (the tool commands), `history.rs` (the preview and history commands), `tauri.conf.json`, `capabilities/` |
| `Scripts` | Dev, check and build scripts |

## Building and Testing

| Script | What it does |
| --- | --- |
| `Scripts\Dev.cmd` | Starts the app with `tauri dev`: the UI reloads live, and Rust changes rebuild and restart the app |
| `Scripts\Check.cmd` | `cargo fmt --check`, Clippy with warnings as errors, `cargo test` (which also regenerates `src/bindings`), then `tsc`, ESLint with no warnings allowed, and Vitest. Run it before every commit. |
| `Scripts\Build.cmd` | Builds the release app and its NSIS installer |
| `Scripts\Capture-Fixtures.ps1` | Runs yt-dlp on the test links and saves its answers, trimmed, as the metadata tests' fixtures. Run it when yt-dlp's output changes; it needs the network |

- First setup: `npm install` in `tsundoku/`. Needs Rust (MSVC toolchain), Node and Visual Studio's C++ build tools.
- **This PC:** the git-ignored `.cargo/config.toml` at the project root sends Rust's build output to `D:\Dev\tsundoku\target`, since Google Drive would sync it and lock files mid-build, and sets the dev data folder (below). Cargo reads it from any folder below the project root. Without it, Cargo builds into `tsundoku/target`. The installer lands in `<build folder>\release\bundle\nsis`.
- `tsundoku/.cargo/config.toml` (committed) tells ts-rs to write to `src/bindings`.
- The version lives in `tsundoku/package.json`, which `tauri.conf.json` reads, and in `[workspace.package]` of `tsundoku/Cargo.toml`. Bump both together.
- The UI alone runs in a browser with `npm run dev` in `tsundoku/` (port 1420), which helps check layouts: outside Tauri, `api.ts` returns the stand-ins in `standIns.ts`, and actions fail. Adding `?setup` to the address shows the first-run setup. In this workspace, the Coding root's `.claude/launch.json` has it as `tsundoku-ui`.
- To drive the real app's UI from a script, start it with `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9223` set: its WebView then takes DevTools Protocol connections on that port (`http://127.0.0.1:9223/json` lists the page), for evaluating JavaScript and taking screenshots.
- The project is its own git repository (`github.com/Fuzen777/Tsundoku`, private until it's ready to share); the Coding monorepo ignores this folder. Commits use the GitHub noreply address set in this repository's git config. Commit subjects follow `Title | description`. `README.md`, `CHANGELOG.md` and the other files GitHub shows stay plain GFM, without Obsidian admonitions.

## Data and Testing

- The data folder holds `settings.json`, `tools\` (one folder each for `yt-dlp`, `ffmpeg` and `deno`: Tsundoku's own copies), the history's `tsundoku.db` (with its `-wal` and `-shm` files while the app runs) and `archive.txt`. Dev builds use their own: `TSUNDOKU_DATA_DIR` when set (this PC's `.cargo/config.toml` sets `D:\Dev\tsundoku\data`), otherwise `%LocalAppData%\Tsundoku Dev`. Release builds use `%LocalAppData%\Tsundoku` and ignore the variable.
- `D:\Dev\tsundoku\` is the disposable sandbox: `data\` (the dev data folder), `downloads\` (test downloads) and `target\`. Anything under it can be changed or deleted freely for a test. Never test against an installed copy's `%LocalAppData%\Tsundoku`.
- The first-run setup shows while `settings.json` has no yt-dlp or ffmpeg: delete it, with the app closed, to see it again. With `tools\` left in place, the setup offers those copies; delete `tools\` too for a first run from nothing. Setting `ytDlpLastUpdateCheck` to an old value makes the startup update of yt-dlp run.
- Until downloads exist, the duplicate warning can be tried by adding rows to `tsundoku.db` (a `videos` row and a `downloads` row pointing at it). `archive.txt` is rewritten from the database when the app starts.

## Technical Notes

- The main window starts hidden and transparent (`tauri.conf.json`). `src-tauri/src/lib.rs` applies Mica, which needs Windows 11, and records whether it worked. `main.tsx` reads that through the `app_info` command before the first render and sets `data-backdrop` on `<html>`: `mica` makes the page transparent, `solid` paints `--color-bg`. `App` shows the window after its first frame, and Rust shows it anyway after 3 seconds if the UI never does.
- `single-instance` is the first plugin registered, as it requires: a second launch focuses the running window. `window-state` restores size, position and maximized state, never visibility.
- Every Tauri permission the UI uses is listed in `src-tauri/capabilities/default.json`; `core:window:allow-show` is there for the hidden start.
- The CSP in `tauri.conf.json` allows `https:` images, for video thumbnails. Network requests (GitHub) come from Rust, never from the UI.

### Running Programs

- `core::process` is the only place that starts yt-dlp, ffmpeg or Deno. Each starts without a console window (`CREATE_NO_WINDOW`), suspended, is put in a new job object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, then resumed, so it can't start anything outside the job. Ending the job ends yt-dlp and the ffmpeg it started; closing the job's handle (when the app exits or crashes) does the same.
- process-wrap was tried and dropped: its job object for `std` never sets kill-on-close, so the tools would outlive a crash.
- Output is read line by line on two threads into a channel; a carriage return also ends a line. `run` polls every 100 ms for cancellation and its time limit.
- Tauri commands that run programs or download are `async` and do the work through `spawn_blocking`, since a sync command runs on the main thread and would freeze the window.

### Tools

- `core::tools::catalog` names each tool's GitHub repository, release file and checksum file. ffmpeg comes from yt-dlp/FFmpeg-Builds' **shared** build (91 MB, 193 MB installed), not the static one (202 MB, 340 MB installed): ffmpeg and ffprobe share their DLLs instead of each carrying a copy. Everything in its `bin` folder is kept except ffplay.
- Checksum files come in two forms: `<hash>  <name>` lines (yt-dlp, FFmpeg-Builds) and PowerShell's `Hash : <HASH>` (Deno's `.sha256sum`). `checksum.rs` reads both.
- An install downloads to `tools\.download-<tool>\`, checks the SHA-256, unpacks to `tools\.new-<tool>\`, then swaps that folder in for `tools\<tool>\` (the old one moved aside, deleted after, put back on failure). Each tool has its own working folders, since the setup installs all three at once. Afterwards the tool is run (`--version`) to catch an antivirus quarantine.
- GitHub allows 60 API requests an hour per network without signing in. The latest release of each tool is cached for 10 minutes, so the setup's sizes and downloads cost one request per tool.
- yt-dlp updates itself with `--update-to stable@latest` (or `nightly@latest`). Naming the release, not just the channel, lets it go back from a nightly build to an older stable one, which plain `--update-to stable` refuses as a downgrade. It renames itself to `yt-dlp.exe.old` while updating and deletes that at its next update.
- ffmpeg and Deno are compared by the SHA-256 of the release file they came from, stored in `settings.json`. FFmpeg-Builds rebuilds daily under the rolling tag `latest`, so its releases are labelled by date. A copy adopted from `tools\` at setup has no stored checksum, so its first check always offers an update.
- Progress reaches the UI as `tool-progress` events (`ToolProgress`); every install or update ends with one whose stage is `null`. `tools-changed` tells the UI a tool changed on its own (the startup update of yt-dlp).
- A tool's errors reach the UI as `ToolError`, a tagged union; `errors.ts` turns each kind into a sentence from `strings.ts`, with the technical `detail` underneath. `PreviewError` and `HistoryError` work the same way.

### Metadata and Preview

- `metadata::normalize_link` accepts only http and https links with a host (adding `https://` when there's no scheme), so pasted text, a search (`ytsearch:`) or a file path never reaches yt-dlp. The link is also passed after `--`, so nothing in it can be read as an option.
- yt-dlp runs as `--ignore-config --no-playlist --flat-playlist --dump-single-json --encoding utf-8`, with `--js-runtimes deno:<path>` when Deno is set up (`--no-js-runtimes` otherwise, so a Deno on PATH can't hide that it's missing) and `--ffmpeg-location`. `--flat-playlist` makes a playlist or channel answer at once instead of reading each video; `--encoding utf-8` keeps its messages from being mangled by the Windows code page. The Deno path must be a Windows path: yt-dlp silently ignores a `/d/…` one.
- The site is yt-dlp's `extractor_key` in lowercase, as yt-dlp's archive writes it (`youtube`, `soundcloud`, `archiveorg`), and with the video ID it's what the duplicate check matches on. A format counts as video when it has a height and isn't `vcodec: none`, since some sites (archive.org) give no codec.
- yt-dlp reports a failure as its last `ERROR:` line; `classify_failure` sorts it into a `PreviewError` by phrases in it. Upcoming premieres are errors too ("This live event will begin in…"), not a field of the answer. When a site's message changes, add the new phrase and a test.
- `PreviewService` runs one fetch at a time: starting one cancels the one before, and the UI ignores an answer that arrives after a newer request (`usePreview`).
- `dragDropEnabled` is `false` in `tauri.conf.json`: with Tauri's own drop handling on, WebView2 never gives the page a dropped link.

### History

- `tsundoku.db` is SQLite (`rusqlite`, bundled), in WAL mode with foreign keys on. Migrations are SQL strings in `history.rs`, applied in order and counted in `PRAGMA user_version`; never change a released one, add one. A database from a newer version is refused rather than read.
- `videos` holds one row per site and ID; `downloads` one row per finished download, removed with its video (`ON DELETE CASCADE`). Times are seconds since 1970.
- `archive.txt` is rewritten whole (atomically) after every change and when the history opens, in the order videos were first downloaded.
- The UI names a download by its ID to open it or show it in its folder, never by a path, so it can only open files the history recorded.

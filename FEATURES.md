---
type: Features
---
# Features

> [!abstract]
> What the app does today. The full v1 design is in [PROJECT](./PROJECT.md).

## Install and Updates

- **Per-user installer**: installs for your account only, in `%LocalAppData%\Tsundoku`, without administrator rights, with a Start menu shortcut. WebView2 is downloaded if it's missing (it comes with Windows 11).
- **Updates**: Tsundoku checks GitHub for a newer version at startup. When there is one, a banner under the add bar offers Install and Restart, which downloads it with its progress, installs it and restarts; Later hides it until the next start. Updates are signed, and an older version is never installed in place of a newer one.
- **Check for Updates**: in Settings → About, with "up to date" or the new version as the answer.
- **Your data stays**: the history, presets, settings and Tsundoku's copies of the tools live in `%LocalAppData%\io.github.fuzen777.tsundoku`, kept through updates and uninstalls unless you tick the uninstaller's **Delete the application data**.

## Window

- **Mica**: Windows 11's backdrop shows through the window, which follows Windows' light or dark theme. Windows 10 gets a plain background.
- **Remembered placement**: the window reopens at its last size and position, maximized if it was.
- **One window**: opening Tsundoku while it is already running brings the open window to the front.

## Navigation

- **Sidebar**: Queue, History and Presets at the top, Settings at the bottom.
- **About**: Settings shows the app's version.

## Adding a Video

- **Add bar**: paste a link and press Enter or Add. A link without `https://` ("youtu.be/…") works too.
- **Drop a link**: dropping a link anywhere on the window, such as one dragged from a browser's address bar, puts it in the add bar and fetches it right away.
- **Preview card**: under the add bar, the video's thumbnail, title, uploader, site, length, upload date and best quality ("1080p", or Audio Only). A spinner shows while yt-dlp fetches it, with Cancel; a new link replaces the one being fetched.
- **Already downloaded**: when the history has the video, the card says when and with which preset, shows the file's path with Open File and Show in Folder, and Download becomes Download Again. If the file was moved or deleted, it says so. The check uses the site and the video's ID, so every link to a video is caught: `youtu.be`, Shorts, a timestamp, a playlist it's in.
- **Refused links**: playlists and channels, live streams, and premieres that haven't started each get a plain message. So do unsupported sites, unavailable videos, videos needing a sign-in or an age check, and network errors, with yt-dlp's own error underneath.
- **Without Deno**: a YouTube preview warns that some formats may be missing.
- **Download**: pick a preset (the default is preselected) and Download: the video joins the queue, and the card and the add bar clear. With a video already downloaded, it's Download Again; with the same preset as before, it asks first, since the new file replaces the old one.
- **Already in the queue**: the card says so, without Download.

## Queue

- **Two at a time**: downloads start in the order they were added, two at once.
- **Each row**: thumbnail, title, preset, what it's doing (Downloading Video, Downloading Audio, Merging, Converting, Embedding…), size, speed and time left, with a progress bar.
- **Stop and Resume**: Stop ends the download and keeps what it got; Resume continues from there. Stopping one that's waiting holds it.
- **Cancel**: removes the download and its partial files.
- **Failed downloads**: the reason, in plain words with yt-dlp's error under it, and Retry, which continues where it failed.
- **Finished downloads**: Open File, Show in Folder and Open Log, until Clear Finished. They stay in the history.
- **Logs**: each download keeps yt-dlp's full output, kept for 30 days.
- **After a restart**: the queue comes back; downloads that were running come back stopped, and waiting ones start.
- **Closing while downloading**: asks first. Downloads continue where they stopped next time.
- **Sidebar count**: the number of downloads not done yet, beside Queue.

## Presets

- **Four to start with**: Best Video (best quality, in MKV, the default), 1080p MP4 (up to 1080p, H.264 and AAC for older devices), Best Audio (the audio as it is) and Audio MP3.
- **Editor**: name; video or audio; for video, the quality limit and the container (MKV, MP4 or as merged); for audio, the format (as it is, MP3, M4A, Opus or FLAC); the file name; the folder (Videos or Music by default); what to embed (metadata, thumbnail, chapters, subtitles with their languages); SponsorBlock (off, mark as chapters, remove sponsors); and extra yt-dlp arguments.
- **Arguments shown**: the editor shows the yt-dlp arguments the preset gives, as they change. Extra arguments that would break downloads (such as `--yes-playlist` or `-o`) are refused, with the reason.
- **Set as Default, Duplicate, Delete**, and **Reset** for the four built-ins, which can be edited but not deleted.
- **Kept per download**: a download keeps the preset as it was when it was added, so editing a preset doesn't change downloads already in the queue.

## History

- **History view**: every video downloaded, newest first: thumbnail, title, uploader, site, length, and its latest download's preset, date and size.
- **Search, filters, sorting**: search titles and uploaders word by word; filter by site and by preset; sort by date downloaded, title, uploader, site, length, size or preset, either way.
- **Several downloads**: a video downloaded more than once (as video, then as audio) expands to list each download, with its format, path, Open File and Show in Folder.
- **Actions**: Open File and Show in Folder, and in its menu Open Source Page, Download Again (the link goes in the add bar and its preview opens) and Forget.
- **Forget**: after asking, removes the video from the history and from `archive.txt`, so it can be downloaded again. Its files stay where they are.
- **File Missing**: a download whose file was moved or deleted says so, and its Open File is off.
- **Kept for good**: every finished download is recorded, with the video's details, the preset, the file's path, size and format ("1080p · avc1 + mp4a", or "mp3"), and the date. A file yt-dlp found already there is recorded too.
- **yt-dlp's archive**: `archive.txt` lists every video in yt-dlp's own format (`youtube jNQXAC9IVRw`), kept up to date, so yt-dlp on the command line skips them too with `--download-archive`.

## Settings

- **Downloads at Once**: one to five, two by default. Raising it starts waiting downloads right away.
- **Speed Limit**: for each download, such as 500K or 2M, or none. A value yt-dlp wouldn't understand is refused with an explanation.
- **Default Preset**: the one the preview card picks, also settable in the presets view.
- **Cookies**: a browser to read cookies from (Firefox, Chrome, Edge, Brave, Opera, Vivaldi or Chromium), for videos that need a signed-in account or an age check. It applies to previews and downloads. Choosing a Chromium browser warns that Windows' app-bound encryption often stops yt-dlp from reading its cookies; Firefox works.

## Fixes Offered on Failures

- **A site's page couldn't be read**: Update yt-dlp, and Try Nightly when Tsundoku's copy is on Stable, right under the error, on the preview card or the queue row.
- **The video needs a signed-in account**: Set Up Cookies opens Settings.

## Tools

Tsundoku downloads with three free tools: yt-dlp, ffmpeg (with ffprobe) and Deno, which yt-dlp needs for some YouTube formats.

### First-Run Setup

- **Found copies**: the first start looks for each tool on this PC (on PATH, such as winget's or scoop's, or Tsundoku's own from an earlier setup) and shows its version.
- **Use This Copy or Download Fresh**: for each tool found, use that copy or download Tsundoku's own; tools not found are downloaded. Each download shows its size and its progress.
- **Official builds, checked**: downloads come from each tool's official GitHub releases (yt-dlp's own FFmpeg builds for ffmpeg) and are checked against the release's published SHA-256 before they're installed.
- **Antivirus check**: after installing, Tsundoku runs the tool to make sure it's there and works, and says so clearly when Windows Security or an antivirus took it.
- **Deno is optional**: the app opens once yt-dlp and ffmpeg are ready. If Deno's download fails, Continue Without Deno leaves it for later.

### Settings → Tools

- **Each tool's copy**: its version and path, and whether it's Tsundoku's copy or yours.
- **Choose File…**: point a tool at any copy on this PC. Tsundoku checks it's the right program (and that ffprobe is beside ffmpeg).
- **Use Tsundoku's Copy / Download**: switch from your copy to Tsundoku's own, or set up a tool that isn't.
- **Cancel**: stops a download or update in progress, leaving the copy already installed as it was.

### Updates

- **yt-dlp**: Tsundoku's copy updates itself in the background at startup, at most once a day, and on demand with Update Now.
- **Update channel**: Stable or Nightly, which gets site fixes sooner. Switching updates Tsundoku's copy right away, including back from a nightly build to the latest stable one.
- **ffmpeg and Deno**: Check for Updates asks GitHub for a newer build, and Update installs it. They're never updated without asking.
- **Your own copies are never updated** by Tsundoku.

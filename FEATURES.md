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
- **Portable copy**: `Tsundoku.exe` with a `.portable.txt` file beside it keeps everything in a `data` folder there instead: settings, history, tools, logs, the window's place and the WebView's data. It doesn't check for updates yet (Settings → About says so), and Start with Windows lists it as "Tsundoku (Portable)", apart from an installed copy. Only one of the two runs at a time. `Scripts\Build-Portable.cmd` makes one from the repository.

## Window

- **Mica**: Windows 11's backdrop shows through the window, which follows Windows' light or dark theme. Windows 10 gets a plain background.
- **Icons for row actions**: the actions on each row or card (Start, Remove, Stop, Resume, Retry, Cancel, Open File, Show in Folder, Edit, Delete…) are icons, and hovering one shows its name. A view's main actions (Add, Download, Start All, Clear Finished, Add Rule) keep their words.
- **Remembered placement**: the window reopens at its last size and position, maximized if it was.
- **One window**: opening Tsundoku while it is already running brings the open window to the front.
- **Keep running in the tray**: with Settings → Running in the Background → Keep Running in the Tray on, closing the window leaves Tsundoku in the notification area, still downloading and watching the clipboard. Clicking its icon opens the window; its right-click menu has Open Tsundoku, Watch Clipboard (ticked while on), Start All Pending with how many links it would start, and Quit Tsundoku, which asks first while downloads run. Off to start with: closing the window then quits, asking first while downloads run.
- **Start with Windows**: Tsundoku opens when you sign in to Windows; with Keep Running in the Tray on, it starts in the tray without showing its window.
- **Notifications**: while the window isn't in front (hidden, minimized or behind another app), a Windows notification says when a copied link was added to Pending or started downloading (with its title), what the clipboard shortcut or the browser added, and how many links Start All Pending started from the tray. Clicking one opens Tsundoku. On to start with; Settings → Adding Links → Show Notifications turns them off.

## Navigation

- **Sidebar**: Queue, Pending, History, Presets and Rules at the top, Settings at the bottom.
- **About**: Settings shows the app's version, and "Portable" for a portable copy.
- **Data folder**: Settings → About shows the data folder's path, with Open Folder to open it in File Explorer.

## Adding a Video or Post

- **Add bar**: paste a link and press Enter or Add. A link without `https://` ("youtu.be/…") works too.
- **Several links at once**: pasting text with several links in it (one per line, or a paragraph holding them) sends them all to Pending instead of the preview card. Words that aren't links are left out, and each link is taken once.
- **Drop a link**: dropping a link anywhere on the window, such as one dragged from a browser's address bar, puts it in the add bar and fetches it right away. Dropping several links sends them to Pending.
- **Preview card**: under the add bar, the video's thumbnail, title, uploader, site, length, upload date and best quality ("1080p", or Audio Only). A spinner shows while yt-dlp fetches it, with Cancel; a new link replaces the one being fetched.
- **Already downloaded**: when the history has the video, the card says when and with which preset, shows the file's path with Open File and Show in Folder, and Download becomes Download Again. If the file was moved or deleted, it says so. The check uses the site and the video's ID, so every link to a video is caught: `youtu.be`, Shorts, a timestamp, a playlist it's in.
- **Refused links**: playlists and channels, live streams, and premieres that haven't started each get a plain message. So do unsupported sites, unavailable videos, videos needing a sign-in or an age check, and network errors, with yt-dlp's own error underneath.
- **Image posts**: an Instagram, X, Bluesky, Pixiv or other image post shows its caption, author, date and what it holds ("2 images, 4 videos"). Each link is read by whichever tool suits it, yt-dlp or gallery-dl, and by the other when the first can't, so the user never has to choose.
- **Download a post**: a post has Download without a preset, since its files are kept as the site gives them. They all go into the site's folder (Instagram, X, or the site's name) in the download folder, by the last rule, which can be changed. Files already there are skipped, so Download Again only fetches what's missing.
- **MEGA, MediaFire, Google Drive and Dropbox**: a link to a file or a shared folder shows its name, date and what it holds (the file's size, or "14 files, 1.2 GB") and has Download without a preset. Tsundoku downloads these itself, nothing to set up. A shared folder keeps its own folder and subfolders inside the download folder; files already there are skipped. MEGA and MediaFire files are checked once they arrive, and a damaged one is deleted so Retry downloads it again. Google Docs, Sheets and Slides in a Drive folder are left out. A link missing its key, a password-protected link, a used-up download limit and a link on one of these hosts that isn't to a file or folder Tsundoku recognizes each get a plain message.
- **Without Deno**: a YouTube preview warns that some formats may be missing.
- **Download**: pick a preset (the default is preselected) and Download: the video joins the queue, and the card and the add bar clear. With a video already downloaded, it's Download Again; with the same preset as before, it asks first, since the new file replaces the old one.
- **Where it goes**: beside Download, the folder the download will be saved in and what picked it: a rule ("Rule 2"), the preset's folder or the download folder. Clicking it opens the rules, the presets or Settings.
- **Already in the queue**: the card says so, without Download.

## Pending

- **Links waiting to start**: the Pending tab holds links pasted or dropped several at once, or copied while Watch Clipboard is on, oldest first, until they're started. They're kept through a restart.
- **Watch Clipboard**: off to start with, turned on in Settings → Adding Links or with the box on the Pending tab. While Tsundoku is open, even minimized, links copied in any app go to Pending: one link, or several on their own, separated by spaces or lines. A copy holding anything else (a sentence with a link in it) is left alone, and so are direct links to a file (an image, a PDF), what's copied inside Tsundoku itself, what a password manager marks private, and what was copied before the watcher was turned on.
- **Checking copied links**: a copied link is read before it shows, with "Checking 2 copied links…" meanwhile. One that no tool can download (a Google Doc, a shop, a news page without a video, a channel) leaves without a word, and so does one already downloaded or in the queue. One that needs a fix, such as signing in, shows with its message and fixes like any other. So does a MEGA, MediaFire, Google Drive or Dropbox link Tsundoku doesn't recognize, so a file you meant to download doesn't go missing.
- **Clipboard Shortcut**: a key combination picked in Settings → Adding Links (Ctrl+Alt+D, Ctrl+Shift+D, Ctrl+Alt+V or Alt+Shift+D; none to start with) adds the links on the clipboard to Pending from any app, as if pasted there, so each shows even when nothing can download it. It works with Watch Clipboard off. A shortcut another app already uses is refused, keeping the one picked before.
- **Send from your browser**: Settings → Adding Links → Send from Your Browser explains how to add a bookmark to the browser's toolbar, with Copy Bookmark Address. Clicking the bookmark on a page sends that page to Pending, as if pasted, and the browser stays in front: Tsundoku starts if it isn't running (in the tray with Keep Running in the Tray on), and a notification says what was added. The browser asks once whether to open Tsundoku. Works in Firefox, Zen, Chrome, Edge and the rest.
- **Download Right Away**: copied links, links the clipboard shortcut adds and pages sent from the browser start downloading once they're read, with the default preset, instead of waiting to be started. One downloaded before still waits, marked. Links pasted or dropped by hand still wait.
- **Read in the background**: each link is read one at a time, so its row fills in with the thumbnail, title, uploader, site and what it holds, as the preview card shows them. Instagram links wait the same random 5 to 10 seconds between reads as between files, and links from other sites are read meanwhile.
- **Already downloaded**: a link the history has says when and with which preset, and its Start becomes Download Again. Two links to the same video are kept once. A link already in the queue says so.
- **Start All**: sends every link to the queue except those downloaded before, already in the queue or that couldn't be read. Videos start with the preset picked in Videos Start As (the default one to start with); a link's own preset picker overrides it. Links still being read start once they're read, unless they turn out to be downloaded already.
- **Select**: with links ticked (or all of them, with the box at the top), Start Selected and Remove Selected act on those instead. Starting a video again with the preset it was downloaded with asks first, since the new file replaces the old one.
- **Each row**: Start (or Download Again), Remove, and the preset picker for a video. A link that couldn't be read shows why, the same fixes as the preview card, and Try Again.
- **Where it goes**: under a read link's details, the folder it will be saved in and what picked it (a rule, the preset's folder or the download folder), as the preview card shows it. It follows the preset the link would start with, and clicking it opens the rules, the presets or Settings.
- **Sidebar count**: the number of links waiting, beside Pending, leaving out copied links still being checked.

## Queue

- **Two at a time**: downloads start in the order they were added, two at once.
- **Posts**: a post's row shows what it holds ("3 images, 1 video") instead of a preset, and its progress counts files. On Instagram, each file waits a random 5 to 10 seconds first (Settings → Instagram), so the account isn't blocked.
- **MEGA, MediaFire, Google Drive and Dropbox**: a file host's row shows what it holds instead of a preset. Its progress covers every file together, with the speed limit applied; Stop and Retry continue the file they stopped in.
- **One Instagram post at a time**: while an Instagram post downloads, the next one waits, since two at once would read twice as fast. Downloads from other sites go ahead of it meanwhile.
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
- **Editor**: name; video or audio; for video, the quality limit and the container (MKV, MP4 or as merged); for audio, the format (as it is, MP3, M4A, Opus or FLAC); the file name; the folder (the download folder by default); what to embed (metadata, thumbnail, chapters, subtitles with their languages); SponsorBlock (off, mark as chapters, remove sponsors); and extra yt-dlp arguments.
- **Arguments shown**: the editor shows the yt-dlp arguments the preset gives, as they change. Extra arguments that would break downloads (such as `--yes-playlist` or `-o`) are refused, with the reason.
- **Set as Default, Duplicate, Delete**, and **Reset** for the four built-ins, which can be edited but not deleted.
- **Kept per download**: a download keeps the preset as it was when it was added, so editing a preset doesn't change downloads already in the queue.

## Rules

- **Folder rules**: "when all of these match, save in this folder". A condition checks the site (by any of its names, such as X, twitter or x.com, with every supported site suggested), the uploader or account, the title, the link, the kind (video, audio, post of images or shared file) or the preset, with Is, Is Not, Contains or Doesn't Contain, ignoring case. A rule without conditions matches every download.
- **In order**: rules are tried from the top, and the first one that's on and matches decides. Without one, a preset's own folder is used, then the last rule's folder, then (with the last rule off) the download folder.
- **Folders that follow the download**: `{site}` and `{uploader}` in a folder become the download's site and uploader (Unknown without one), made safe as folder names. Clicking {site} or {uploader} under the folder field puts it where the cursor is. A folder that isn't a full path goes inside the download folder.
- **Each as a sentence**: "When the site is Instagram and the uploader is “natgeo”", with its number and its folder as it will be used. Move Up, Move Down, Edit, Turn Off or On, and Delete (after asking). An edit is a draft until Save.
- **Everything else**: every install starts with "Everything else → {site}", so each site's downloads get a folder of their own. It's always the last rule ("Always Last"), so every other rule comes first and new rules go above it, and a preset's own folder comes before it too. It has no conditions: its folder can be changed and it can be turned off, which sends what it took to the download folder, but it can't be moved or deleted. Until 0.1.30 it took only posts of images.
- **Kept per download**: a download keeps the folder it got when it was added, so changing the rules doesn't move downloads already in the queue.

## History

- **History view**: every video downloaded, newest first: thumbnail, title, uploader, site, length, and its latest download's preset, date and size.
- **Search, filters, sorting**: search titles and uploaders word by word; filter by site and by preset; sort by date downloaded, title, uploader, site, length, size or preset, either way.
- **Several downloads**: a video downloaded more than once (as video, then as audio) expands to list each download, with its format, path, Open File and Show in Folder.
- **Posts**: a downloaded post shows what it held ("2 images, 1 video") and expands to list every file it saved, each with its size, Open File and Show in Folder, or File Missing.
- **Actions**: Open File, Show in Folder and Open Source Page, and in its menu Download Again (the link goes in the add bar and its preview opens) and Forget.
- **Forget**: after asking, removes the video from the history and from `archive.txt`, so it can be downloaded again. Its files stay where they are.
- **File Missing**: a download whose file was moved or deleted says so, and its Open File is off.
- **Kept for good**: every finished download is recorded, with the video's details, the preset, the file's path, size and format ("1080p · avc1 + mp4a", or "mp3"), and the date. A file yt-dlp found already there is recorded too.
- **yt-dlp's archive**: `archive.txt` lists every video in yt-dlp's own format (`youtube jNQXAC9IVRw`), kept up to date, so yt-dlp on the command line skips them too with `--download-archive`.

## Settings

- **Download Folder**: the folder downloads go in, each site's in a folder of its own by the last rule, unless another rule or the preset picks another folder: picked at the first-run setup, a Tsundoku folder in Downloads by default, with Choose Folder… and Use Default. Rules' folders that aren't full paths go inside it. A portable copy's Downloads folder beside it is saved without the copy's own folder, so it follows the copy to another drive.
- **Downloads at Once**: one to five, two by default. Raising it starts waiting downloads right away.
- **Speed Limit**: for each download, such as 500K or 2M, or none. A value yt-dlp wouldn't understand is refused with an explanation.
- **Default Preset**: the one the preview card picks, also settable in the presets view.
- **Adding Links**: Watch Clipboard, the Clipboard Shortcut, Send from Your Browser, Download Right Away for the links they add (see Pending), and Show Notifications (see Window).
- **Running in the Background**: Keep Running in the Tray and Start with Windows (see Window).
- **Instagram**: the pause before each Instagram file, and between Instagram links in Pending: a random time in a range, 5 to 10 seconds to start with, up to 600.
- **Cookies**: where to read cookies from, for links that need a signed-in account or an age check. It applies to previews and downloads. Automatic (the default) finds every browser profile on the PC and picks, for each link, the one signed in to its site (by which cookies exist, never their values), or for an unknown site the one that visited it last. A single browser (Firefox, Zen, LibreWolf, Floorp, Waterfox, Chrome, Edge, Brave, Opera, Vivaldi or Chromium) uses its profile signed in to the site, else its default one. Choosing a Chromium browser warns that they lock and encrypt their cookies on Windows; Firefox and its forks work.
- **Supported Sites**: every site yt-dlp and gallery-dl can read, merged into one searchable list without saying which tool reads which, each site once (its playlists, channels and other parts that the tools list on their own are left out). Sites that need an account (Instagram, X, YouTube, TikTok, Reddit, Facebook, Twitch, Niconico, Bilibili) come first, under Signed In with the browser profiles signed in to them, most recent first, or under Not Signed In. The list is read again when Settings opens after a minute, since sign-ins change in the browser.

## Fixes Offered on Failures

- **A site's page couldn't be read**: Update yt-dlp, and Try Nightly when Tsundoku's copy is on Stable, right under the error, on the preview card or the queue row.
- **The video needs a signed-in account**: with cookies off, Set Up Cookies opens Settings. With cookies on, the preview names the site and says whether a browser is signed in to it: "Instagram needs you signed in. Sign in to Instagram in your browser, then try again." or that the site refused the browser's sign-in, with Try Again and Cookie Settings.
- **A post of images while gallery-dl isn't set up**: Set Up Image Downloads sets it up and fetches the post again. A site gallery-dl couldn't read offers Update gallery-dl.

## Tools

Tsundoku downloads with four free tools: yt-dlp, ffmpeg (with ffprobe), Deno, which yt-dlp needs for some YouTube formats, and gallery-dl, for image posts.

### First-Run Setup

- **Where things go, asked first**: before the tools, a first screen asks where the tools go (Tsundoku's data folder, already picked, or a folder you choose; a portable copy also offers In AppData, an installed Tsundoku's tools folder, picked when its tools are already there so they're used instead of downloaded again), where they keep their caches (Tsundoku's data folder or the tools' usual folders in your user folder) and where downloads go (a portable copy's folder beside it, a Tsundoku folder in Downloads, or a folder you choose). The last two start unpicked, and Continue waits for them, so nothing lands anywhere unasked. Back on the tools screen returns to it until a tool is set up. An install set up before 0.1.20 isn't asked: it keeps the data folder's tools, the tools' usual cache folders and its download folder.
- **Found copies**: the first start looks for each tool on this PC (on PATH, such as winget's or scoop's, or Tsundoku's own from an earlier setup) and shows its version.
- **Use This Copy or Download Fresh**: for each tool found, use that copy or download Tsundoku's own; tools not found are downloaded. Each download shows its size and its progress.
- **Official builds, checked**: downloads come from each tool's official releases, on GitHub (yt-dlp's own FFmpeg builds for ffmpeg) or Codeberg for gallery-dl, and are checked against the release's published SHA-256 before they're installed.
- **Antivirus check**: after installing, Tsundoku runs the tool to make sure it's there and works, and says so clearly when Windows Security or an antivirus took it.
- **gallery-dl too**: image posts are as much what Tsundoku is for as videos, so the setup sets up gallery-dl with the others (a copy found, such as an installed Tsundoku's in AppData, is used as it is).
- **Deno and gallery-dl are optional**: the app opens once yt-dlp and ffmpeg are ready. If only Deno's or gallery-dl's download fails, Continue Without Deno (or gallery-dl, or Them) leaves them for Settings → Tools. An install set up before 0.1.25 isn't asked again; Settings → Tools sets gallery-dl up.

### Settings → Tools

- **Each tool's copy**: its version and path, and whether it's Tsundoku's copy or yours.
- **Choose File…**: point a tool at any copy on this PC. Tsundoku checks it's the right program (and that ffprobe is beside ffmpeg).
- **Use Tsundoku's Copy / Download**: switch from your copy to Tsundoku's own, or set up a tool that isn't.
- **Cancel**: stops a download or update in progress, leaving the copy already installed as it was.
- **Caches**: Data Folder keeps yt-dlp's, gallery-dl's and Deno's caches (sign-ins, sites' player code, downloaded scripts) in the data folder's `cache`; Usual Folders leaves them where each tool keeps them in your user folder. It applies from the tools' next run.

### Updates

- **yt-dlp**: Tsundoku's copy updates itself in the background at startup, at most once a day, and on demand with Update Now.
- **Update channel**: Stable or Nightly, which gets site fixes sooner. Switching updates Tsundoku's copy right away, including back from a nightly build to the latest stable one.
- **ffmpeg, Deno and gallery-dl**: Check for Updates asks for a newer build, and Update installs it. They're never updated without asking.
- **Your own copies are never updated** by Tsundoku.

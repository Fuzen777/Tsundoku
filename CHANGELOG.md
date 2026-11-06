# Changelog

All notable changes to Tsundoku. Versions follow [Semantic Versioning](https://semver.org/): every change bumps the patch number, and a release bumps the minor or major number, folding in the patch entries since the previous release.

## 0.1.22 · 2026-11-06

### Fixed

- **Back from the tools kept the first screen's picks**: going back to "Where Things Go" showed it blank again. It now shows what was picked, also after closing Tsundoku before the tools were set up.

## 0.1.21 · 2026-11-06

### New

- **Google Drive and Dropbox**: pasting a Google Drive or Dropbox link, to one file or a shared folder, shows its name and what it holds on the preview card, with Download, like MEGA and MediaFire links: no account or setup needed, a shared folder keeps its own folders, files already there are skipped, and Stop and Retry continue where they stopped. Google Docs, Sheets and Slides in a Drive folder are left out, since they aren't files. Both are in Settings → Supported Sites, and {site} names them in rules ("Google Drive", "Dropbox").

### Changed

- A Google Drive or Dropbox link to a video now downloads the file exactly as it was uploaded, like any other file there, instead of being treated as a video with a preset.
- The messages for a used-up download limit and for a link missing its key now fit every file host.

## 0.1.20 · 2026-11-06

### New

- **Portable copy**: put a `.portable.txt` file beside `Tsundoku.exe`, and that copy keeps its settings, history, tools, logs and window position in a `data` folder beside it instead of your user folder. Settings → About shows "Portable"; a portable copy doesn't check for updates yet. Its Start with Windows entry is "Tsundoku (Portable)", so it never changes an installed copy's. `Scripts\Build-Portable.cmd` builds one.
- **Where things go, asked at the first start**: a new first screen asks where the tools go, where they keep their caches and where downloads go, before setting up the tools. The cache and download choices start unpicked, so nothing appears anywhere unasked. A portable copy can keep its downloads beside it, following it to another drive. Installs already set up aren't asked and keep what they had.
- **Tool caches** in Settings → Tools: keep yt-dlp's, gallery-dl's and Deno's caches in Tsundoku's data folder, or in their usual folders in your user folder.

## 0.1.19 · 2026-11-06

### Changed

- **The posts rule is always last**, so every rule you add comes before it. New rules go above it, it can't be moved or deleted, and it shows "Always Last". It can still be edited or turned off. If it was higher up, it moves to the bottom; if it was deleted, it comes back turned off.

### New

- **Clickable placeholders**: under a rule's folder, {site} and {uploader} buttons put the placeholder where the cursor is, over any selected text, so they don't have to be typed.

## 0.1.18 · 2026-11-06

### New

- **Send from Your Browser**: Settings → Adding Links offers a bookmark for your browser's toolbar (Copy Bookmark Address, then paste it as a new bookmark's URL). Clicking it on a video's or a post's page sends that page to Pending while you stay in the browser: Tsundoku starts if it isn't running (in the tray, with Keep Running in the Tray on), and a notification says what was added. The first time, the browser asks whether to open Tsundoku.

### Changed

- Download Right Away also applies to pages sent from the browser, and no longer needs Watch Clipboard on.

## 0.1.17 · 2026-11-06

### New

- **Keep Running in the Tray**: a new Settings → Running in the Background section. With it on, closing the window leaves Tsundoku in the notification area, still downloading and watching the clipboard. Clicking its icon opens the window; right-clicking gives Open Tsundoku, Watch Clipboard, Start All Pending (with how many) and Quit Tsundoku, which asks first while downloads run.
- **Start with Windows**: Tsundoku opens when you sign in to Windows, in the tray without its window when Keep Running in the Tray is on.
- **Clipboard Shortcut**: in Settings → Adding Links, pick Ctrl+Alt+D, Ctrl+Shift+D, Ctrl+Alt+V or Alt+Shift+D. Pressing it in any app adds the links on the clipboard to Pending, as if pasted there, even with Watch Clipboard off. A shortcut another app already uses is refused with a message.
- **Notifications**: while the window isn't in front, a Windows notification says when a copied link was added to Pending or started downloading, what the shortcut added, and what Start All Pending in the tray started. Clicking one opens Tsundoku. On to start with; Show Notifications in Settings → Adding Links turns them off.

### Changed

- Download Right Away now also applies to links the clipboard shortcut adds.

## 0.1.16 · 2026-11-06

### Fixed

- **MEGA folders**: a shared folder no longer fails with "missing its key" when some of its files also keep their key for another folder, which about a third of public MEGA folders do. A file the link can't open is left out instead of failing the whole folder.
- **MediaFire**: a file page without a Download button is no longer always taken for a browser check.

### Changed

- MEGA shared folders and MediaFire folders show their date on the preview card.
- A folder's download shows Finishing only after its last file, and the speed shown after resuming no longer starts out far too low.

## 0.1.15 · 2026-11-06

### New

- **Watch Clipboard**: a new Settings → Adding Links section, and a box on the Pending tab, turn on a clipboard watcher. While Tsundoku is open, links you copy in any app go to Pending, one or several at a time. Each is checked first ("Checking 2 copied links…"): links no tool can download, and links already downloaded or in the queue, leave without a word. Copies holding anything besides links, direct links to an image or a file, what you copy inside Tsundoku and what a password manager marks private are left alone.
- **Download Right Away**: with the watcher on, copied links start downloading once they're read, with the default preset, instead of waiting in Pending. A link that needs a fix, such as signing in, still waits there.

## 0.1.14 · 2026-11-06

### New

- **Pending**: a new Pending tab in the sidebar holds links waiting to be started. Pasting or dropping several links at once sends them there instead of the preview card. Each one is read in the background, one at a time, so its row shows what it is, whether it was downloaded before and whether it's in the queue, and they're kept through a restart.
- **Start All**: starts every pending link not downloaded before, with the preset picked in Videos Start As (each video can have its own instead). Links still being read start once they're read. Ticking links gives Start Selected and Remove Selected.

### Changed

- **One Instagram post at a time**: an Instagram post now waits while another one downloads, even with two downloads at once, since each pauses between its files on its own and two together read twice as fast. Downloads from other sites go ahead of it. Reading Instagram links in Pending waits the same pause between links.
- The add bar says "Paste One Link or Several".

## 0.1.13 · 2026-11-06

### New

- **MEGA and MediaFire**: pasting a MEGA or MediaFire link, to one file or a shared folder, shows its name, date and what it holds ("14 files, 1.2 GB") on the preview card, with Download. Tsundoku downloads the files itself, with no tool to set up: a shared folder keeps its own folder and subfolders inside the download folder, files already there are skipped, Stop and Retry continue where they stopped, the speed limit applies, and each file is checked once it arrives (MEGA's against its key, MediaFire's against its checksum). A damaged file is deleted and downloaded again on Retry.
- **Plain messages for file hosts**: a MEGA link without its key ("the part after #"), a password-protected link, and MEGA's free download limit each get a message of their own.
- **Rules**: a kind for shared files, and {site} names the host ("MEGA", "MediaFire"). MEGA and MediaFire are in Settings → Supported Sites.

### Changed

- "There's no video to download here" now reads "There's nothing to download here", since it's also what an empty shared folder says.

## 0.1.12 · 2026-11-06

### New

- **Open the data folder**: Settings → About shows where Tsundoku keeps its settings, history, tools and download logs, with Open Folder to open it in File Explorer.

## 0.1.11 · 2026-11-06

### New

- **Rules**: a new Rules tab in the sidebar decides where downloads go. A rule says "when all of these match, save in this folder", checking the site, uploader, title, link, kind (video, audio or post of images) or preset, with is, is not, contains or doesn't contain, ignoring case. Rules are tried from the top, and the first one that's on and matches wins over a preset's own folder and the download folder. A rule's folder can hold {site} and {uploader}, such as `D:\Hydrus\{site}\{uploader}`, and a folder that isn't a full path goes inside the download folder. Rules can be moved up and down, turned off and deleted.
- **Where a download goes**: the preview card shows the folder a download will be saved in and what picked it (a rule, the preset or the download folder). Clicking that opens where it's set.

### Changed

- Posts of images go in their site's folder through a rule every install starts with, "When it's a post of images → {site}", which can be edited like any other. Settings → Posts of Images no longer has a folder; a posts folder chosen before becomes that rule's folder.

## 0.1.10 · 2026-11-06

### Changed

- **One download folder**: everything Tsundoku downloads now goes to a Tsundoku folder in Downloads by default, videos, audio and posts of images alike, with each site's posts in a folder of their own there (Instagram, X…). Settings → Downloads has Download Folder to choose another one. A preset with a folder of its own still uses it, and Settings → Posts of Images can still send posts somewhere else. Until now, presets went to Videos or Music and posts to Pictures.

## 0.1.9 · 2026-11-05

### New

- **A post's files in the history**: a downloaded post keeps every one of its files. Its row in the history shows what it held ("2 images, 1 video") and a Files list with each file's name, size, Open File and Show in Folder, and File Missing for one that was moved or deleted. Forgetting the post forgets its files' records too; the files stay.

### Changed

- The history's preset filter leaves out posts, which have no preset.

## 0.1.8 · 2026-11-05

### New

- **Download posts of images**: an Instagram, X or other image post's preview card has Download. Every file of the post, as the site gives it, goes into the site's own folder (Instagram, X…) in the posts folder, ready for a Hydrus import folder. The queue shows what the post holds and counts its files as they're saved, and the history keeps the post with its first file and the size of all of them. Downloading a post again only fetches the files that aren't in the folder any more.
- **Settings → Posts of Images**: the posts folder (the Pictures folder by default), and the pause before each Instagram file, 10 to 20 seconds to start with, so the account isn't blocked for reading too fast.

## 0.1.7 · 2026-11-05

### Changed

- **Supported Sites** lists each site once: the parts of a site that are listed on their own (its playlists, channels, embeds, seasons, live streams and the like, such as "ArteTVPlaylist" beside "ArteTV") are left out when the site itself is listed, and so are descriptions that aren't site names.

## 0.1.6 · 2026-11-05

### New

- **Settings → Supported Sites**: every site Tsundoku can download from, in one searchable list, whichever tool reads it. Sites that need an account come first: under Signed In with the browser profiles signed in to them (Tsundoku uses the most recent one), or under Not Signed In with a hint to sign in. Image sites join the list once image downloads are set up.

## 0.1.5 · 2026-11-05

### New

- **Sign-in hint**: when a site needs you signed in, the card says what to do about it. If cookies are off, it says to turn them on. If no browser is signed in to the site, it says to sign in there and offers Try Again. If a browser is signed in but the site still refused, it suggests signing in again. Cookie Settings opens Settings.

### Changed

- Browser profile folders passed to the tools use Windows' separators throughout, and the queue's tests no longer look at the PC's browsers.

## 0.1.4 · 2026-11-05

### New

- **Cookies set to Automatic**: Tsundoku finds the browsers on your PC and their profiles (Firefox, Zen, LibreWolf, Floorp, Waterfox, Chrome, Edge, Brave, Opera, Vivaldi, Chromium), and for each link reads cookies from the profile signed in to that site, so nobody has to type a profile path. It only looks at which cookies exist, never at what they hold. A site Tsundoku doesn't know uses the profile that visited it last. Automatic is the default for new settings; None and a single browser are still there.
- **Zen, LibreWolf, Floorp and Waterfox** in Settings → Cookies. Picking a browser reads its profile signed in to the site, or its default profile.

## 0.1.3 · 2026-11-05

### New

- **Previews of image posts**: an Instagram, X, Bluesky, Pixiv, Tumblr or other image post shows its caption, author, date and how many images and videos it holds ("2 images, 4 videos"). Links to image sites are read with gallery-dl first and everything else with yt-dlp, and when one can't handle a link the other is tried, so any link simply works or doesn't. Downloading these posts comes next; the card says so for now.
- **Set Up Image Downloads**: a photo post while gallery-dl isn't set up says so, with a button that sets it up and tries again. A site gallery-dl couldn't read offers Update gallery-dl.

### Changed

- **Messages no longer name yt-dlp** when a link can't be read ("This site isn't supported."), since gallery-dl may be the one reading it. Profiles are refused like playlists and channels.

## 0.1.2 · 2026-11-05

### New

- **gallery-dl in Settings → Tools**, the first step towards downloading image posts such as Instagram photos and carousels, which yt-dlp can't. It works like the other tools: use your own copy or Tsundoku's, downloaded from gallery-dl's releases on Codeberg (where it moved from GitHub), checked against their SHA-256 and updated when you check. It's optional and not part of the first-run setup. Nothing uses it yet.

### Changed

- **Tool messages** no longer name GitHub, since gallery-dl comes from Codeberg: "Looking for the latest release…" and "Couldn't reach the download site."

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

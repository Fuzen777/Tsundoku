# Changelog

All notable changes to Tsundoku. Versions follow [Semantic Versioning](https://semver.org/): every change bumps the patch number, and a release bumps the minor or major number, folding in the patch entries since the previous release.

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

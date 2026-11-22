# Changelog

All notable changes to Tsundoku. Versions follow [Semantic Versioning](https://semver.org/): every change bumps the patch number, and a release bumps the minor or major number, folding in the patch entries since the previous release.

## 0.2.15 · 2026-11-22

### Changed

- **A channel's or account's list shows only what will download**: unticking an item fades it slowly for two seconds, then moves it under Not Downloading, a heading under the list folded to start with; ticking it again while it fades keeps it. What was downloaded before or is in Downloads waits there too, and unfolding it and ticking one moves it back up. Items listed by Load More, Load New or Load All now always arrive ticked.
- **Download page by page**: Download Selected is now Download, with Download and Load More beside it while there's more to list: it queues what's ticked, keeps the card open, and lists the next ones to go on picking.

## 0.2.14 · 2026-11-22

### Changed

- **Watch can download what you ticked**: Watch on a channel's, playlist's or account's list now asks what Tsundoku downloads of it now: New Content Only, Latest 50 Posts (or another number), Selected Posts with how many are ticked in the list above, or Everything Listed with how many it holds. What you pick is queued with the source's Save In and Videos Start As as it starts being watched, so downloading some of it and watching it no longer takes pasting the link twice. Latest and Everything Listed leave out what was downloaded before or is in Downloads; a ticked item downloaded before is downloaded again, asking first when that replaces its file. Everything listed then counts as known, so Check Now lists only what's posted later; before, Watch's choice put items on the source's card as new, to pick from there.

## 0.2.13 · 2026-11-22

### Added

- **History is grouped and shown as pictures too**: History's toolbar groups the downloads by Site, Folder, Uploader or Month, under headings that fold away, with the groups listed in the sidebar to jump to, as in Downloads. Its header has the same List, Masonry and Gallery buttons and − / + sizes: a picture shows its row under it when clicked, and a right-click offers its actions. It still reads the history a page at a time and draws only what's on screen, so it stays quick with thousands of downloads. Its grouping and sort are now remembered.
- **Pending as pictures, with chapters**: Pending's header has the List, Masonry and Gallery buttons too, and while it's grouped, the sidebar lists its groups to jump to.
- **Back to the top**: clicking the open view in the sidebar again scrolls it back to its top.

### Changed

- **The view buttons stay in sight**: in Downloads, the List, Masonry and Gallery buttons and − / + moved into the header beside Clear Finished, so they're there however far down the list you've scrolled.

## 0.2.12 · 2026-11-22

### Changed

- **Paste Link is at the top of the sidebar**: Paste Link and Type a Link moved from above every view to the top of the sidebar, under the app's name, so the views now take the window's whole height. Type a Link, or Ctrl+L, opens its field in a panel under them, wide enough for a long link; Esc, its × or a click elsewhere closes it and keeps what you typed. "There's no link on the clipboard." shows under Paste Link for a few seconds. Ctrl+V outside a text field adds what you copied from any view, as before.

## 0.2.11 · 2026-11-22

### Added

- **A warning when a manual rule doesn't match**: a manual rule applied to a link its conditions don't match still applies, as before, but an orange warning under the folder line now says so, and that {1}, {2}… have no value when its pattern finds nothing in the link. It shows on the link's card, in Pending and in Test a Link, without asking anything.

## 0.2.10 · 2026-11-21

### Added

- **Rules' chapters in the sidebar**: while Rules is open, the sidebar lists Automatic Rules and Manual Rules under it, each with how many rules it holds, to jump straight to either list. The one at the top is marked as you scroll.

## 0.2.9 · 2026-11-21

### Changed

- **The Queue is now called Downloads**: it holds the downloads running, waiting and done until you clear them, while History keeps every download ever made. Its button in the sidebar, its title, Remove from Downloads, the In Downloads marks and the notifications say so.
- **One List, Masonry or Gallery for the whole view**: the view buttons moved from each channel's heading to the toolbar's right, and now show every download that way, posts and videos added on their own included. They start as the headings were set.
- **Pictures alone**: in Masonry and Gallery, a download is its picture, without its title and progress under it (they show when you point at it); one without a picture shows its title in its place.
- **Grouped by status, cards don't say it again**: under its status's heading, a row leaves out "Waiting", "Done" or "Downloading" (a stage such as Merging still shows), a picture its corner icon, and a channel's heading says only how many downloads it holds.

### Added

- **Chapters in the sidebar**: grouped, the Downloads view lists its groups under its button in the sidebar, the statuses with their sites, folders or rules under them, each with how many downloads it holds. A click scrolls there, unfolding a folded status, and the one at the top is marked as you scroll, as Settings' sections are.

### Fixed

- **Grouping by rule follows the rule a download was added with**: a rule added later that matches everything no longer pulls every download already there under it. Each download keeps the rule that gave its folder (a rule deleted since shows as Deleted Rule). Downloads added before this version go by the rules as they are now when those give them the same folder, else under Unknown Rule with their folder.

## 0.2.8 · 2026-11-21

### Changed

- **MEGA's own reset time**: when MEGA's transfer quota is used up, a download now waits exactly as long as MEGA says, until the quota frees up, then goes on by itself from where it stopped. Before, it guessed: 30 minutes, then an hour, two and four, so it could sit idle for hours after the quota was back, or try again too soon. The row says "MEGA's download limit is used up. Continuing at …" with MEGA's time.
- **A dropped connection that got somewhere doesn't count against the download**: a download whose connection drops is still tried again by itself after 30 seconds, 2 minutes and 8 minutes, but once more of it has arrived, the three tries start over. A big file on a shaky connection keeps going instead of failing after the third drop. The same goes for a host's download limit: once more has downloaded, the next wait starts at 30 minutes again.

## 0.2.7 · 2026-11-21

### Added

- **Manual rules**: Rules has a second list, Manual Rules, for rules that never pick downloads by themselves. Apply one to a link with Apply a Rule…, beside where it will be saved: it then decides the folder, file name, preset and unpacking it sets for that link, whatever its conditions. Its conditions are there for their patterns: what a Matches pattern captures fills {1}, {2} and so on in the folder and the file name, so a manual rule "title matches `(RJ\d+)`, save in `D:\Voice\{1}`" sorts only the links you apply it to by their code. Add Manual Rule beside the list's heading adds one, and a rule's More Actions menu moves it from one list to the other. Manual rules are always on, have no Download Right Away, and Apply a Rule… lists them first.
- **Apply a Rule… on the link's card**: the card that opens for a pasted link has Apply a Rule… beside its folder too, as Pending's links have, so one link can take a rule, a manual one included, without going through Pending.
- **Try a rule in Test a Link**: Rules' Test a Link has Apply a Rule… beside its folder, to see what a rule applied by hand gives a link, and it stays picked for the next link tested.

### Fixed

- **Test a Link's Archives follows the rules as they're edited**, as its other answers do.

## 0.2.6 · 2026-11-21

### Added

- **Passwords are tried before you're asked**: a link that needs a password is first tried with the passwords Tsundoku knows, and asks only when none of them opens it, saying how many it tried. That goes for the preview card, a link in Pending and a download in the queue, which runs again with each one before waiting for you.
- **Settings → Passwords**: Your Passwords, the list that was in Settings → Archives, is now tried on links too, in its order, before anything else. The passwords you had there are still there.
- **Save Past Passwords**: off unless you turn it on. While it's on, a password you enter on a link, a download or an archive is kept once it has opened it, apart from your list, and tried after your list the next time: on links from the same site, and on any archive. Settings says how many are kept, and Forget Past Passwords forgets them all.
- **An archive is also tried with the password that opened its link**, which is often the same one.

### Changed

- **An archive's password is no longer added to your list**: the Password field on a failed download now unpacks that download with what you type, and keeps it as a past password only while Save Past Passwords is on. It says which under the field. To have a password tried every time without that, add it to Your Passwords in Settings.
- **Links are tried with care**: with 20 passwords at most, and never with a past password from another site, since trying one sends it to the link's site. Archives are opened on your PC, so every past password is tried on them.
- **Backups** hold the past passwords, and restoring one brings its own back in place of this PC's.

## 0.2.5 · 2026-11-21

### Changed

- **Paste Link instead of the add bar's field**: the field across the top, where a link was pasted and then added, is now one button. Paste Link adds what you copied in one click: one link opens its preview, several go to Pending. With no link on the clipboard, it says so beside the button and adds nothing. Ctrl+V anywhere in the window still does the same, and so does Paste on the empty Queue and Pending.
- **Type a Link**: the small button beside it, or Ctrl+L, brings a field back to type a link, or to edit the last one shown, until it's added or closed with Esc.
- **A copied sentence isn't taken for links**: Paste Link, Ctrl+V and the Clipboard Shortcut add what the clipboard holds without anyone reading it first, so among several words they now take only links written with `http://` or `https://`. Before, a copied sentence could send words such as "notes.txt" to Pending as links. A single copied link still works without `https://` ("youtu.be/…"), and a link inside a copied sentence is now found, where it used to fail as "That isn't a link".

## 0.2.4 · 2026-11-21

### Added

- **Load All on big X accounts**: X lists an account's posts only so far at a time, about 1,000 posts, then makes the next ones wait up to 15 minutes, so Load All on an account of thousands used to hang and give up. It now waits and goes on by itself: the card says "Paused by X, resuming at 4:46 AM" with the time left, taken from what X itself says, the posts listed so far stay there to tick and download meanwhile, and the listing continues where it stopped once X answers again, as often as it takes. Load More and Load 50 New wait the same way, and Stop works during a wait.
- **How long it takes**: on an X account too big to list in one go, a line under Load All says how X paces it and how long the whole listing takes at most, by X's count of the account's photos and videos ("up to 7 hours"). It often ends sooner: X's lists don't always reach an old account's first posts.
- **A listing remembers what it loaded**: what Load More, Load 50 New and Load All listed is kept for 30 days, for channels and accounts on every site. Opening the same channel or account again shows its newest items together with everything listed before, and the buttons go on from where they stopped, even after Tsundoku was closed. One listed whole opens whole.

### Changed

- **X accounts continue instead of starting over**: each read of an X account now starts on the page where the last one stopped, rather than reading the account from its newest post again, so Load More takes the same few seconds however far down the list is. Its first list and Load More show 50 posts or more, the next 50 of its photos or of its videos with the other kind's posts among them, rather than exactly 50.
- **Another X account while X makes one wait**: reading an X account when X's allowance is used up now says so at once ("X lists only so many posts every 15 minutes…") instead of waiting 5 minutes and failing.

## 0.2.3 · 2026-11-21

### Added

- **What's New whenever you want**: Settings → About has a What's New button beside the version. It opens the What's New card again, on this version's changes, with a list of every earlier version to pick from, so what an update brought can be read again after its card was closed.

## 0.2.2 · 2026-11-20

### Fixed

- **X videos' pictures**: a video or GIF posted on X now shows its cover wherever its picture goes: an account's list in List, Masonry and Gallery, the preview card and its strip of a post's files, Pending, Watched and the Queue. Before, they showed an empty tile. Watched items listed earlier without a picture get theirs the next time a check reads them, which covers an account's newest posts; downloads already in the queue keep the empty tile.

## 0.2.1 · 2026-11-20

### Added

- **X's count on Load All**: an X account's Load All now says how many photos and videos the site counts on it, as its Media tab does ("Load All (372 Photos & Videos)"). X counts files there, not posts, so the button's hint says how many of them the posts listed so far hold, and the line shown while listing counts up against it ("Listing… 150 posts · 212 of 372 photos & videos · 12 new"). The site's count runs a little ahead of what can be listed: it includes GIFs and videos posted in replies, which its lists leave out.

## 0.2.0 · 2026-11-18

Everything since 0.1.0. Tsundoku now downloads posts of images and shared files as well as videos, lists channels, playlists and accounts to pick from, watches them for what's new, and takes links in many more ways.

### Added

- **Posts of images**: Instagram, X, Bluesky, Pixiv, Tumblr and other image posts preview and download, through gallery-dl, which is set up at the first start with the other tools and updates itself once a day, as yt-dlp does. Pick which of a post's files to download; a post downloaded in part marks what's left, so Download Again gets just the rest. Files are named after the account, the post's date, its ID and their place in it, which Settings → Posts of Images changes.
- **Shared files and folders**: MEGA, MediaFire, Google Drive, Dropbox, OneDrive, pCloud, Box, Icedrive, Yandex Disk, WeTransfer, SwissTransfer, TransferNow, Filemail, Files.fm, upload.ee, 1fichier and Internet Archive items download with no tool or account to set up. A folder keeps its subfolders, files already there are skipped, Stop and Retry continue where they stopped, and each file is checked once it arrives where the site allows it. A folder lists its files as a tree to tick, and downloading it again takes only the files not downloaded before. Folders on Gofile, Pixeldrain, Bunkr and Cyberdrop list their files too. OneDrive and Files.fm folders with folders inside them can't be read yet.
- **Mirror pages and short links**: MultiUp and Mirrored.to pages open their first working mirror, and short links (Bitly, TinyURL, is.gd, t.co and others) read as the link they lead to.
- **Checks only a person can pass**: a download behind a "check you're a person" page (Cloudflare's, on MediaFire and other sites) waits in the queue with Open Check, which shows the site's page in a small Tsundoku window. Once you click Download there, Tsundoku downloads the file, or the window does when the site gives it only to the browser that passed. Workupload and Buzzheavier download this way.
- **Passwords**: a video that needs a password (Vimeo's, and the other sites yt-dlp opens with one) asks for it on the preview card, in Pending and in the queue, and keeps it with the link, so Retry and Download Again never ask twice. It's never written in a log. File hosts' password-protected links can't be opened yet, and say so.
- **Unpacking archives**: zip, 7z and rar archives, in one file or split into parts, are unpacked once downloaded, trying the passwords in Settings → Archives. Where they're unpacked (Smart, In Place or Folder) and what happens to the archive afterwards are set there, or for the downloads a rule matches. An archive that can't be unpacked says why, and Retry only unpacks it.
- **Channels, playlists and accounts**: a YouTube channel or playlist, an Instagram or X account, or another site's playlist lists what it holds in the preview card, to tick and Download Selected. The list shows as a List, Masonry or Gallery, with Load More, Load 50 New and Load All, Hide Downloaded, and a choice of one type (Images, Videos, Mixed or Files). Instagram and X need a browser signed in to them.
- **Watched**: a new view for the channels, playlists and accounts you want to follow, each with its own folder and preset. Check Now, Check All or a site's own check lists what's new since; All New Items shows every source's new items in one list, and opens by itself when a check finds something. A source folds to one line with its latest downloads, and Show Downloads in History opens the history at what came from it. Checks are by hand for now.
- **Pending**: several links pasted or dropped at once wait in a new Pending view, each read in the background, with Start All, and each link's own preset, folder, rule and picked files. Removing has Undo.
- **More ways to add links**: Watch Clipboard, with Download Right Away; a Clipboard Shortcut that works in any app; a Send from Your Browser bookmark; Send to Tsundoku buttons on Instagram, X and YouTube, through a userscript; dropping a text file, a bookmarks export or an Internet shortcut; Click'n'Load buttons (FileCrypt's, off until you turn it on); and Ctrl+V anywhere in the window. The empty Queue and Pending list them all.
- **Rules**: a new Rules view decides where downloads go, and if you want their preset, their file names, how their archives are unpacked and whether copied links download right away. Conditions check the site, uploader, title, link, kind or preset, joined by AND or OR, with Match Case and regular expressions. Folders and file names take placeholders such as `{site}`, `{uploader}`, `{account}`, `{playlist}`, `{title}` and `{date}`, and what a pattern captured. Rules have names, can be dragged into place, duplicated and tested against a link, and one can be applied to a single link in Pending. The last rule that matches wins.
- **The queue**: Pause All, Resume All, Retry Failed and Download Next; grouping by status, site, folder or rule, and sorting, as in Pending; the downloads picked from one channel or account together under its heading, as rows or pictures; a folder picked for one download; each download's log shown under its row; Copy Details for a bug report; Remove from Queue with Undo; and a double-click to open a finished download.
- **Downloads that wait instead of failing**: a dropped connection is tried again by itself, a site that asks to slow down or a used-up download limit waits and then continues (with Try Now), and a download that won't fit on its drive waits for room.
- **Pacing**: Settings → Pacing sets the pause between files and how many downloads run at once for Instagram, X, Pixiv and Tumblr, and an hourly limit for Instagram.
- **History**: filters by kind, date, whether the file is still there and watched source; ticked rows for Forget Selected, Show Selected in Folder and Move Selected…; Move Files…, which the history follows; each file of a post or a shared folder; thumbnails kept so rows don't go blank; and how much what it lists takes on disk, beside Tsundoku's own data.
- **Cookies set to Automatic**: Tsundoku finds your browsers' profiles (Firefox, Zen, LibreWolf, Floorp, Waterfox, Chrome, Edge, Brave, Opera, Vivaldi, Chromium) and reads cookies from the one signed in to each link's site. When a site needs you signed in, the card says what to do about it, and Settings → Supported Sites lists every site, with where you're signed in.
- **Running in the background**: Keep Running in the Tray, Start with Windows, notifications for links added and for failed or finished downloads, and the queue's progress on the taskbar button and in the tray's tooltip.
- **Appearance**: Light, Dark or Auto, and an accent color: one of six, or any color you pick.
- **Portable copy**: with a `.portable.txt` file beside `Tsundoku.exe`, that copy keeps everything in a `data` folder beside it. It doesn't check for updates yet.
- **Where things go, asked at the first start**: the tools, their caches and the downloads, before the tools are set up. Installs already set up aren't asked.
- **Back up and restore**: Settings → About saves your settings, presets, rules and history in one file, and restores it on another PC or after a reinstall.
- **What's New**: shown after an update, and from the update banner before you install one.
- **Presets and tools**: Add Preset, Move Up and Move Down, and Update All in Settings → Tools.
- **Keyboard shortcuts**: Ctrl+L for the add bar, Ctrl+1 to Ctrl+7 for the views, and Delete in Pending. Settings → Keyboard Shortcuts lists them.

### Changed

- **One download folder**: everything goes to a Tsundoku folder in Downloads by default, each site's downloads in a folder of their own there (YouTube, Instagram…), instead of Videos and Music. Settings → Downloads picks another folder, and Everything Else, the first rule in Rules, can be changed or turned off. A preset with a folder of its own still saves there.
- **A new look**: a new dark mode and a clearer light mode, icons with tooltips for the actions on each row, and Settings' sections listed in the sidebar while it's open.
- **Presets save as you edit**. The default preset sits first, and any preset but the default can be deleted; the built-in ones can be put back.
- **A history that stays quick** at thousands of downloads: it reads them a page at a time and draws only the rows on screen. It counts items rather than videos.
- **Tool updates**: yt-dlp and gallery-dl are updated once a day even while Tsundoku stays open, and an update waits until no download is using the tool.
- **Folders open in your folder app**: Show in Folder and every Open Folder button use the app Windows opens folders with, and File Explorer when that app is gone.
- **Words that fit everything you download**: messages no longer name the tools or call every download a video.

### Fixed

- **Dropdowns in dark mode** were white with faint light text.
- **Right-clicking the window** showed the browser's own menu (Back, Reload, Print). It now opens nothing, apart from Cut, Copy and Paste in a text field and Copy over selected text.
- **Show in Folder froze Tsundoku** on a PC where Windows' setting for opening folders points to an app that's gone.
- **The setup's options** show all their text: a long path wraps instead of being cut off.

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

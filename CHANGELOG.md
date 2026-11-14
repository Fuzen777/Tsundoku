# Changelog

All notable changes to Tsundoku. Versions follow [Semantic Versioning](https://semver.org/): every change bumps the patch number, and a release bumps the minor or major number, folding in the patch entries since the previous release.

## 0.1.143 · 2026-11-14

### Fixed

- **Show in Folder froze Tsundoku**: on a PC where Windows' setting for opening folders still points to an app that's gone (a file manager uninstalled since), Show in Folder showed "Application not found" and Tsundoku stopped responding until it was closed. Show in Folder and every Open Folder button now open File Explorer itself, with the file selected, and Tsundoku keeps responding while Windows opens anything, files included.

### Changed

- **Show Selected in Folder** in History selects the first of the ticked files in each folder it opens, rather than all of them.

## 0.1.142 · 2026-11-14

### Added

- **Downloads from one channel together in the queue**: the items you pick from a channel, playlist or account (Download Selected, or a watched source's new items) now sit together in the queue under the source's own heading, so it's clear they came from the same place: its picture and name, where its downloads stand ("3 of 12 done · 2 downloading · 7 waiting") and their progress together. Fold it away with the chevron, or click the picture or the name. The heading also pauses, resumes, retries or cancels all of them at once, and clears them once they're all done. Picking more from the same source later adds them to the same heading. Downloads added before this version show as before.
- **List, Masonry or Gallery in the queue**: a source's heading has the picker's view toggle and picture size buttons, so its downloads can show as pictures. Right-click a picture for its actions, or click it to see its full row.

## 0.1.141 · 2026-11-14

### Changed

- **Watched, grouped by site**: the Watched tab now puts its sources under a heading per site (instagram.com, x.com, youtube.com), each saying how many new items wait in it, with a button that checks every source under it. A heading folds its group away, and Group By None lists them all together. A source whose last check failed moves to the top under Needs Attention until a check works.
- **Cards that fold**: each source folds to its picture, name and one line on where it stands: when it was checked, how many items are new, downloaded and skipped, and when it last posted. A card opens by itself while it has new items; the chevron, picture or name opens or folds it, Expand All opens every card and folded site, and Collapse All folds every card.
- **Sorted your way**: by Most New (to start with), Name, Last Checked, Latest Post or Date Watched, either way round. The grouping and the sort are remembered.
- **Skipped Before** opens the same picker as the new items (List, Masonry or Gallery), so several skipped items can be ticked and downloaded at once.

### Added

- **Check All**: at the top of Watched, checks every source one after the other.
- **All New Items**: beside Sources, shows what's new on every source in one list, the latest posted first, each with its source's name under it (with its site when another source has the same name: "NASA on x.com"). Download Selected queues the ticked ones, each with its own source's folder and preset, and skips the rest; Skip All skips them all.
- **From Watched to History**: a source's Show Downloads in History button opens the history at what was downloaded from it, and the history has a filter of watched sources for the same, which names them the same way.

## 0.1.140 · 2026-11-14

### Added

- **Load More and Load All**: a channel's or an account's list in the preview card no longer stops at its newest 50. Under the items, Load More lists the next ones (50 more videos or X posts, 48 more Instagram posts), and Load All lists everything, with how many there are when the site says ("Load All (8,634)"). While Load All runs, the card counts what's listed so far ("Listing… 150 of 8,634 posts") and adds the items as they come; Stop ends it and keeps what's listed, and Load More or Load All carries on from there. What you ticked stays ticked, and new items that arrive are ticked as the first ones were (unless you unticked everything first). Instagram is read a page of 12 at a time with pauses between, so a big account takes a while; if Instagram asks to slow down anyway, the card says when to try again. Closing the card or reading another link stops it. Once everything is listed, the card says how many there are ("8,634 posts") instead of "Newest".
- **Hide Downloaded**: a button beside the picker's views hides what's downloaded or already in the queue, so only the new items show, with how many are hidden after the count ("12 of 40 selected · 10 hidden"). Select All and Download Selected then take only what's shown. It's remembered.

## 0.1.139 · 2026-11-14

### Added

- **Channels, playlists and accounts wait in Pending**: a YouTube channel or playlist, an Instagram account or an X account copied while Watch Clipboard is on now waits in Pending instead of leaving without a word, and so does one pasted with other links, sent from your browser or found in a dropped file. They sit under Needs Attention, at the top of Pending whatever its grouping, and nothing starts them: not Start All, Start Selected, Download Right Away or the tray's Start All Pending, so copying one never starts a huge download. Each row says what it is, and Pick What to Download opens its list in the preview card; once Download Selected queues what you ticked, or you Watch it, it leaves Pending. A copied one shows a notification while the window isn't in front, and a second link to the same channel or account is kept once.

## 0.1.138 · 2026-11-14

### Added

- **Gallery and masonry in the picker**: a channel's, playlist's or account's list, the new items of a watched source and a post's files now have three buttons beside their select-all box: List, Masonry (the pictures in their own shapes, in columns) and Gallery (a grid of pictures cut to one shape). In Masonry and Gallery, − and + make the pictures smaller or larger. Ticking works the same in every view: click a picture to tick or untick it, and an unticked one is dimmed. Each picker remembers its view and size; both start in Gallery.

## 0.1.137 · 2026-11-14

### Changed

- **Unpacking choices as toggles**: a rule's Unpack Archives is now a row of buttons, Default, Skip, Smart, In Place and Folder, and Settings → Archives' Where to Unpack (Smart, In Place, Folder) and After Unpacking (Recycle Bin, Delete, Keep) are too. What each choice does is in How Rules Work and in the new (?) beside Archives, How Unpacking Works.
- **Archive passwords as tags**: Settings → Archives' Passwords shows each password as a tag with × to remove it. Type one and press Enter to add it; pasting several lines adds a tag for each. Each change is saved at once.

## 0.1.136 · 2026-11-14

### Added

- **Where archives are unpacked**: Settings → Archives → Where to Unpack offers Smart (as before: one file or folder comes out as it is, several go in a folder named after the archive), In a Folder Named After the Archive (always, even for a single file) and In Place (everything beside the archive as it is, never a new folder). A name already taken gets "(2)", so nothing is replaced.
- **Unpacking in rules**: a rule's Unpack Archives sets it for the downloads it matches, whatever Settings → Archives says: Don't Unpack, Smart, In a Folder Named After the Archive or In Place. Leave as Is follows Settings. The rule's line says what it does, and Test a Link shows how a link's archives would be unpacked and which rule decides.

## 0.1.135 · 2026-11-14

### Changed

- **Words that fit everything you download**: more messages and hints stop calling everything a video or a post, since playlists hold songs and shared folders hold files. A channel or account link among several now says it lists several items, and a page Tsundoku can't list says it holds several items. Watch and Watched say Check Now lists what's new. In Rules, a rule that names files says "Names files", and the hints of {title}, {id} and a file name's {post} cover shared files too. Settings → Tools says yt-dlp downloads videos and audio, and gallery-dl posts of images and files on sites such as Gofile and Pixeldrain.

## 0.1.134 · 2026-11-14

### Added

- **X accounts**: an X profile link (`x.com/name` or `twitter.com/name`, or its Replies, Highlights or Media tab) now lists the account's newest 50 posts with pictures or videos in the preview card, as an Instagram account does, with the account's picture. Tick the ones you want and Download Selected: each downloads as if its own link had been pasted, named with the account's handle. Downloaded and queued posts are marked, and Watch keeps the account in Watched, where Check Now lists what it posts next. Like X's posts, accounts need a browser signed in to X.

### Changed

- **An X post's link** in the history is now the post's own address (`x.com/name/status/…`), whichever link to it was pasted.

## 0.1.133 · 2026-11-12

### Added

- **Picking the files of a Gofile or Pixeldrain folder**: a folder or list on Gofile, Pixeldrain, Bunkr or Cyberdrop that holds files other than pictures and videos (archives, documents) now shows them as a list of their names and sizes, numbered, as a shared folder's are, instead of blank tiles, so you can see what you're ticking. Folders of only pictures and videos keep the strip.

### Fixed

- **Gofile links** failed with "Couldn't read this site's page" since a recent update of one of Tsundoku's tools, which dropped the site; they're read again.
- **Ticking a post's files quickly in Pending**: several ticks in a row could lose one; each now shows at once and is kept, as in a shared folder's list.
- **The sentence over files downloaded before** no longer says the files not picked or not downloaded are the ones ticked when they aren't (a link in Pending read before its first download, or ticks changed since): it then says only how many there are.

## 0.1.132 · 2026-11-12

### Added

- **Pick files inside a shared folder**: a link that holds several files (a MEGA, MediaFire, Google Drive, Dropbox, OneDrive, pCloud, Box, Icedrive, Yandex Disk or Files.fm folder, an Internet Archive item, a WeTransfer, SwissTransfer, TransferNow or Filemail transfer) now lists its files on the preview card as a folder tree, everything ticked to start with, so Download still takes the whole folder. Untick files or whole folders to leave them out; the box above the list ticks them all, with how many are ticked and their size. A folder of more than ten files has a filter, which also lets the box tick just the files shown, and a folder of more than 200 starts folded. The list stays quick with thousands of files.
- **In Pending**: Pick Files opens the same list under a shared folder's row, and the ticks are kept, through a restart too. Rows in Pending and the Queue say "12 of 340 files, 1.2 GB" when some are left out.
- **Downloading a folder again**: the list marks each file Downloaded or Not Downloaded, and ticks only those not downloaded before, files the owner added since included, so Download Again gets just them. Files already in the folder are still skipped as before.
- **Files no longer there**: a picked file the folder no longer has is skipped, and when none of the files picked is left, the download fails saying so rather than taking the rest of the folder.

### Changed

- **Pick Files' icon** is now a list with ticks, for a post's files as for a folder's.

## 0.1.131 · 2026-11-12

### Added

- **Videos with a password**: a video that needs a password (Vimeo's, and the other sites yt-dlp opens with one) now asks for it instead of failing. The preview card shows a password field and Unlock (Enter works too), says so when a password doesn't open the link, and becomes the usual card once it does. In Pending, such a link waits as Needs a Password, with the same field on its row: it never leaves by itself, even copied, and Start All and Download Right Away leave it until it's unlocked. A download whose password is missing or no longer works waits in the queue with Enter Password instead of failing. The password is kept with its link in Pending, the queue and the history, so Retry and Download Again never ask twice, and it's never written in a log, Copy Details or a notification.

### Changed

- **Links on file hosts with a password** still can't be opened, and now say Tsundoku can't enter passwords for that site yet. An expired MEGA link says it's unavailable instead of naming MEGA's error.

## 0.1.130 · 2026-11-12

### Added

- **Unpacking archives**: a zip, 7z or rar archive a download brings is unpacked beside it once it's downloaded, in one file or split into parts (`.part1.rar`, `.r00`, `.7z.001`, `.zip.001`), shown as Unpacking with its progress. One file or folder inside comes out as it is; several go in a folder named after the archive, "(2)" added when that name is taken. An archive split over several links is unpacked by the download that brings its last part. Names that would leave its folder and links inside it are left out. Afterwards the archive moves to the Recycle Bin, or is deleted or kept, as Settings → Archives says. Unpacking is on to start with.
- **Settings → Archives**: Unpack Archives After Downloading, After Unpacking, and Passwords: one per line, tried in order on an archive that has a password until one opens it.
- **An archive that wasn't unpacked**: the download fails saying why (a password, a missing part, damage, not enough room) and keeps every file; Retry only unpacks it, without downloading again. For a password, the row has a field to add it to the list, with Add and Retry.

## 0.1.129 · 2026-11-12

### Added

- **Click'n'Load buttons**: Settings → Adding Links → Take Links from Click'n'Load Buttons (off until you turn it on) lets a protected link folder on FileCrypt send its links to Tsundoku with its Click'n'Load button, once you've passed its check in your browser, as it would to JDownloader. The links wait in Pending: each is read, those nothing can download leave without a word, and none starts by itself, even with Download Right Away on. A notification says how many came ("FileCrypt Sent 12 Links"). Tsundoku listens on this PC only and takes links only from the sites it lists; FileCrypt is listed from its documented Click'n'Load form, and its real button still has to be tried. JDownloader uses the same port, so only one of them can listen at a time: when another app has it, Settings says so, with Try Again. Your browser may ask once whether the page may reach devices on your network: allow it for the button to work.

## 0.1.128 · 2026-11-12

### Added

- **Short links**: a short link from Bitly (bit.ly), TinyURL, is.gd, X (t.co), Cuttly, RB.GY, Tiny.cc, T.LY, Google (goo.gl), Ow.ly or Buffer (buff.ly) now opens to where it leads, and reads as that link would: the preview, the download, the history and the folder rules all go by the address it leads to, and a short link to a MultiUp or Mirrored.to page works too. Tsundoku asks the short-link site once where the link leads and never opens a page on it: a short link that shows a page before leading anywhere (a preview or a warning) says so, so you can open it in your browser and add the link it ends on. A short link to a site Tsundoku can't download from names that site, and a copied one leaves quietly, as that site's own link would.

## 0.1.127 · 2026-11-12

### Changed

- **One check window for every site**: the check window no longer knows any site's pages, so a site changing its layout changes nothing. On any site Tsundoku reads itself, a check (Cloudflare's "Just a moment...") holds the download for Open Check, even when it's in front of the link itself, which then shows the end of its address and "Size unknown". In the window, the first download you start from the site is offered to Tsundoku: if Tsundoku can download it on its own, the window stops and the row downloads it with its progress; if not, the window downloads it and the row shows its progress. Downloads from other sites, in an advert's frame or pop-up, never count. The window's title now always says to click Download again if nothing happens, since a site's first click can go to an advert.
- **What the check window can't do**: it can't know a file's name or size before you click Download, takes one file per click, can't list a folder whose page asks for a check, and leaves a site's timers and captchas to you. Its sign-ins stay in it. Links from sites Tsundoku doesn't support still say so: the window isn't offered for them.

## 0.1.126 · 2026-11-12

### Changed

- **Checks on every site**: a check only a person can pass (Cloudflare's "Just a moment...") is now recognized the same way on every site Tsundoku reads itself, whatever the site answers with, instead of some sites reporting it as a refused or missing file. A file whose download meets one waits for Open Check as MediaFire's do.

## 0.1.125 · 2026-11-12

### Changed

- **Downloads in the check window**: a file the check window downloads itself (Workupload's) now shows on its row like any download: the file's name and size as soon as it starts, then a moving bar with the speed and time left. Stop stops it at once and leaves nothing behind, and the file keeps the name the site gives it, without a number added.

## 0.1.124 · 2026-11-12

### Added

- **Buzzheavier**: pasting a Buzzheavier link (buzzheavier.com/…) shows its ID and "Size unknown" until its file is known, since Buzzheavier answers Tsundoku with a check. Its download waits for you to open its check: click Download on its page, and the window closes and Tsundoku downloads the file with its progress. Buzzheavier's first click on Download can open an advert, so the window's title says to click again if nothing happens.

## 0.1.123 · 2026-11-12

### Changed

- **MediaFire**: Tsundoku now asks MediaFire as itself rather than as Firefox. Posing as a browser was what made MediaFire's Cloudflare check appear: asked honestly, its pages come without it, so most MediaFire downloads no longer wait for Open Check. The check stays for when MediaFire still asks.

## 0.1.122 · 2026-11-12

### Added

- **Workupload**: pasting a Workupload link (workupload.com/file/…) shows its ID and "Size unknown", since Workupload checks for a browser before it shows anything. The download waits for you to open its check: click Download on its page, and the check window saves the file itself, then hides; the download moves the file into place and takes its name.
- **Downloads in the check window**: for a site whose file only downloads in the browser that passed its check, the check window saves the file, and the row says so meanwhile. Stopping or removing the download, or closing the window, stops it and deletes what had arrived, and the row says so.

## 0.1.121 · 2026-11-12

### Added

- **Mirrored.to mirror pages**: pasting a Mirrored.to link (mirrored.to/files/…, or a short mir.cr link) opens its first working mirror on a site Tsundoku downloads from, as MultiUp's pages do. When none of Tsundoku's own hosts is among the mirrors, a GoFile or Pixeldrain mirror is taken, for both kinds of pages. A page whose mirrors are all on other sites says which sites they are.

## 0.1.120 · 2026-11-12

### Added

- **Filemail**: pasting a Filemail link (filemail.com/d/…) shows the transfer's files, with Download; each file is checked once it arrives. An expired transfer, one with a password and one found to hold a virus say so.

## 0.1.119 · 2026-11-11

### Added

- **TransferNow**: pasting a TransferNow link (transfernow.net/dl/…) shows the transfer's files, with Download; a transfer of several files goes in a folder named after it. An expired or deleted transfer and one with a password say so.

## 0.1.118 · 2026-11-11

### Added

- **MultiUp mirror pages**: pasting a MultiUp link (multiup.io/download/…) opens its first working mirror on a site Tsundoku downloads from, preferring those that check the file and make no one wait, and shows that file with Download. A page whose mirrors are all on other sites says which sites they are.

## 0.1.117 · 2026-11-11

### Added

- **upload.ee**: pasting an upload.ee file link (upload.ee/files/…) shows the file's name, date and size, with Download. A deleted file says so.

## 0.1.116 · 2026-11-11

### Added

- **Files.fm**: pasting a Files.fm link to a folder (files.fm/u/…) or a file (files.fm/f/…) shows its name, date and what it holds, with Download; a folder's files go in a folder named after it. A deleted link says so, as does a folder with folders inside it, which Tsundoku can't read yet.

## 0.1.115 · 2026-11-11

### Added

- **SwissTransfer**: pasting a SwissTransfer link (swisstransfer.com/dl/…, or the older /d/…) shows the transfer's title and files, with Download; a transfer of several files goes in a folder named after it, keeping any folders inside. An expired or deleted transfer, one with a password and one whose downloads are used up say so.

## 0.1.114 · 2026-11-11

### Added

- **1fichier**: pasting a 1fichier file link (1fichier.com/?…) shows its name and size, with Download, as a free download without an account. Tsundoku waits out 1fichier's countdown before the file (the row says Counting Down, with the time left), and when 1fichier makes free downloads wait between each other, which can be a few hours, the download waits in the queue and says when it goes on. A file with a password, one its owner keeps for 1fichier's subscribers and a deleted one say so.

## 0.1.113 · 2026-11-11

### Added

- **Icedrive**: pasting a public Icedrive link (icedrive.net/s/…) to a file or a folder shows its name, date and what it holds, with Download; a folder keeps its subfolders. A link that's no longer shared says so.

## 0.1.112 · 2026-11-11

### Added

- **Box**: pasting a Box shared link (app.box.com/s/…, or a company's Box site) to a file or a folder shows its name and what it holds, with Download; a folder keeps its subfolders. Box Notes in a folder are left out. A link with a password and one that was removed say so.

## 0.1.111 · 2026-11-11

### Added

- **Yandex Disk**: pasting a public Yandex Disk link (disk.yandex.ru, yadi.sk and its other domains) to a file or a folder shows its name, date and what it holds, with Download; a folder keeps its subfolders, and each file is checked once it arrives. A link to a single video still goes through yt-dlp with a preset, as before. When Yandex asks for its captcha before a file, the download waits for you to open the check, as MediaFire's does. A deleted link and one whose downloads are used up for now say so.

## 0.1.110 · 2026-11-11

### Added

- **Internet Archive items**: pasting an archive.org link to a book, software, images or any other item that isn't video or audio shows its title and files, with Download: the files its uploader gave it, in their own folders, without the copies archive.org makes of them (other formats, OCR text) or its records of the item. A link to one of an item's files (archive.org/download/…) downloads just that file, whichever it is. Each file is checked once it arrives. Video and audio items still go through yt-dlp with a preset, as before. A book only lent to read online says so.

### Changed

- A file host's server that can't be reached is asked again at once, twice, before the download waits to try again.

## 0.1.109 · 2026-11-11

### Changed

- MEGA, Google Drive, Dropbox and OneDrive are asked as Tsundoku rather than as Firefox, since they answer it the same. MediaFire still says it's Firefox.

## 0.1.108 · 2026-11-11

### Added

- **WeTransfer**: pasting a WeTransfer link (we.tl or wetransfer.com/downloads) shows the transfer's name, date and files, with Download. A transfer of several files keeps them in a folder named after it. An expired or deleted transfer, one with a password and one that asks for an email check each say so.

### Changed

- pCloud and WeTransfer are asked as Tsundoku rather than as Firefox, since they answer it the same.

## 0.1.107 · 2026-11-11

### Added

- **pCloud**: pasting a pCloud public link (u.pcloud.link or e.pcloud.link) to a file or a folder shows its name, date and what it holds, with Download, like MEGA and Dropbox. A folder keeps its own folder and subfolders. A deleted link, one with a password and one whose traffic is used up each say so.

## 0.1.106 · 2026-11-11

### Fixed

- **Files on Pixeldrain, Gofile, Bunkr and Cyberdrop**: a file that's neither an image nor a video, such as a .zip, shows on the card and in the queue as "1 file", titled with its name, instead of "1 image" titled with its ID. The card no longer fetches the whole file as its picture, which could mean gigabytes.

## 0.1.105 · 2026-11-11

### Fixed

- **OneDrive folders**: their files download now; every file of a shared folder failed as "deleted" before. A folder of more than 200 files is read whole instead of failing after its first part.
- A OneDrive link that needs a password says so, rather than that it's private or turned off.
- A OneDrive folder with folders inside it says Tsundoku can't read those yet, rather than that it was deleted. OneDrive doesn't give Tsundoku their contents; empty ones are passed over.

## 0.1.104 · 2026-11-11

### Added

- **Apply a rule to one link**: in Pending, the rules button after a read link's folder (Apply a Rule…) lists every rule, and the one you pick decides that link's folder, preset and file name, whatever its conditions, even a rule that's turned off. What it doesn't set still comes from the rules that match. The line says which ("Picked: Nat Geo"), Use the Matching Rules goes back, and the pick is kept through a restart and goes with the link to the queue.

## 0.1.103 · 2026-11-11

### Added

- **MediaFire's check, in a window of Tsundoku's**: a MediaFire file behind MediaFire's "check you're a person" page no longer fails. Its download waits in the queue, saying so, while the others carry on, and nothing opens until you choose. Open Check shows MediaFire's page in a small Tsundoku window: do whatever the check asks and click Download, and the window closes while Tsundoku downloads the file itself, checked as usual. The window never downloads anything itself, can't be taken away from MediaFire by an advert, and remembers a check you passed. Leave the download for later, or close the window, and it keeps waiting. In a shared folder, the files that don't need the check download first, and the window shows each remaining file's page in turn.
- A notification says when a download waits for you to open a check, under Notify When a Download Fails.

### Changed

- A MediaFire download link copied from your browser, pasted while a download waits for that file's check, gets the download past it instead of saying it's already in the queue.

## 0.1.102 · 2026-11-11

### Added

- **Placeholders without a value**: a rule can leave out a placeholder the download has no value for instead of writing Unknown. With its new When a Placeholder Has No Value set to Leave Out, `{site}\{account}` saves a MediaFire file in MediaFire rather than MediaFire\Unknown: a folder whose placeholders all have no value goes, and one with some keeps them, without the separator of what's missing. In the rule's File Name, `{account}` on a download without one is left out the same way instead of becoming the site. Every rule starts with Write Unknown, as before. The rule's row says "Leaves out placeholders without a value", and How Rules Work explains both.

### Fixed

- A rule's file name no longer keeps the separator of a {1} its pattern didn't capture, or of a shared file's unknown date: "{account} - {date} - {post}" on a MEGA file is "MEGA - AbCdEf12", not "MEGA - - AbCdEf12".

## 0.1.101 · 2026-11-11

### Changed

- **How Rules Work**: the explanations under each field of a rule's editor (Name, When, Save In, File Name, Start As and Download Right Away) moved into a (?) beside Add Rule, so the editor shows the fields alone. Start As still says why when a rule can't pick the preset.

## 0.1.100 · 2026-11-11

### Added

- **OneDrive**: pasting a OneDrive link to a file or a shared folder shows its name, date and what it holds on the preview card, with Download, like MEGA and Dropbox. Short links (1drv.ms, in both the old and the newer form) and the onedrive.live.com links they lead to work without an account. A shared folder keeps its own folder and subfolders, and each file is checked once it arrives. A deleted file, or a link that's private, expired or turned off, says so.
- Not tried against OneDrive itself yet: reading a shared folder's subfolders, links in the onedrive.live.com `?cid=…&id=…&authkey=…`, `redir` and `embed` forms, and a download in the app. A single file and a folder's top level were read while building it.

## 0.1.99 · 2026-11-10

### Added

- **Captured text in rule file names**: a rule's File Name can use {1}, {2}… for what its Matches pattern captured, as its folder can: with title matches `(RJ\d+)` and the name `{1}`, "SomefileRJ123456.rar" is saved as "RJ123456.rar". Rule file names now also apply to a shared file (MEGA, MediaFire, Google Drive, Dropbox) when it's a single file, which keeps its extension, and a post's name gets {post} and {number} added when it lacks them.
- **{numberIfMany}**: a file name placeholder for the file's number only when several files of the post are downloaded, so a single photo gets no number. Settings → Posts of Images and rules both have it.
- The empty File Name field of a rule says "Using Default".

## 0.1.98 · 2026-11-10

### Changed

- **Settings slides up the sidebar**: opening Settings slides it up under Rules, with its sections fading in, and leaving it slides it back down, instead of jumping. It stays still when Windows is set to show fewer animations.

## 0.1.97 · 2026-11-10

### Changed

- **Rule file names are posts' names**: a rule's File Name now works like Settings → Posts of Images, with the same placeholders ({account}, {date}, {post}, {number}, {site}), the same checks and an example of the name. Left empty, Settings' name is used. Videos keep their preset's name.

## 0.1.96 · 2026-11-10

### Added

- **File names per rule**: a rule can name its downloads' files, such as `{uploader} - {title}`, with the same placeholder buttons as its folder, plus {number} for a post's files. Without {id}, the ID is added at the end, so two files never share a name. Left empty, videos are named by their preset and posts by Settings → Posts of Images, as before. Shared files keep their own names.

## 0.1.95 · 2026-11-10

### Changed

- **Settings back at the bottom of the sidebar**: Settings sits at the bottom again, and only moves up under Rules while it's open, to list its sections. The sidebar's rows also keep their height when it has to scroll.

## 0.1.94 · 2026-11-10

### Changed

- **AND/OR on the condition's row**: the AND/OR button now starts each condition's row instead of sitting on a row of its own between conditions.

## 0.1.93 · 2026-11-10

### Fixed

- **MediaFire's browser check**: MediaFire now sends some files' pages to a browser check, so their downloads failed with "MediaFire answered 403 Forbidden". A link copied from the page's Download button (`download1234.mediafire.com/…`) is now downloaded from as it is, since your browser already passed the check, and the file's page is only asked once that link has run out. When the page asks for the check, the message says so and what to do: open the link in your browser, click Download, and add the link of the download it starts.

## 0.1.92 · 2026-11-10

### Changed

- **AND and OR per condition**: each gap between two conditions has its own AND/OR button, in capitals, so one rule can mix them. Conditions joined by OR are alternatives and AND joins those: "site is X OR site is Bluesky AND title contains cat" takes a cat from either site. A rule set to Or in 0.1.89 keeps OR between all its conditions.

## 0.1.91 · 2026-11-10

### Added

- **Rule names**: a rule can have a name, set in its editor. It shows above the rule's sentence and, instead of "Rule 3", wherever the rule is mentioned: beside a download's folder on the preview card and in Pending, beside a preset a rule picked, in Test a Link, and in the Queue's and Pending's groups by rule. Rules without a name keep their number.

## 0.1.90 · 2026-11-10

### Added

- **Match case**: each condition of a rule that compares text has a Match Case button (Aa) beside it. On, "Zoo" no longer matches "zoo", for Is, Contains and patterns alike, and the rule's sentence ends with "(matching case)". Off, as conditions you had stay, case is ignored as before.

## 0.1.89 · 2026-11-10

### Added

- **And or Or between conditions**: a rule with several conditions shows an And button between them; clicking it switches the rule to Or, so one matching condition is enough ("When the site is X or the site is Bluesky"), and clicking again goes back. And is the default, and rules you had stay And.

## 0.1.88 · 2026-11-10

### Changed

- **Settings' sections in the sidebar**: Settings now comes right after Rules in the sidebar, and while it's open its sections are listed under it, in place of the row of pills at the top of Settings. The section you're looking at is marked as you scroll. The sidebar scrolls when the window is too short for it.

### Fixed

- **Jumping to a section moved the whole window**: going to a section of Settings near the end, such as About, pushed the whole window up, leaving an empty band at the bottom. Only Settings scrolls now, and the same goes for Test a Link's rule number in Rules.

## 0.1.87 · 2026-11-10

### Added

- **Drag rules into place**: each rule in Rules has a grip on its left; drag it up or down and a line shows where it will go. Everything Else stays first.

## 0.1.86 · 2026-11-10

### Changed

- **The last matching rule wins**: in Rules, the bottom-most rule that matches a download now decides its folder, preset and Download Right Away, so a rule overrides the ones above it. Everything Else moves to the top ("Always First"), and new rules go at the bottom. Rules you had are turned upside down once, so each still decides what it did.

## 0.1.85 · 2026-11-10

### Added

- **Duplicate a rule**: each rule in Rules has a Duplicate button beside Edit, which opens a copy of the rule just below it, ready to change and save. The last rule (Everything Else) has none.

## 0.1.84 · 2026-11-10

### Changed

- **Presets save as you edit**: a change in the preset editor is saved a moment after you stop typing or clicking, with "Saving…" and "Saved." under the fields, so the Save and Discard buttons are gone. Opening another preset, adding one or leaving the Presets view saves first; only edits that can't be saved (an empty name, a refused argument) say why, and ask before they're dropped. Set as Default, Duplicate, Reset and Delete no longer wait for a save.

## 0.1.83 · 2026-11-10

### Changed

- **Tooltips on icon buttons**: resting the pointer on an icon-only button (or reaching it with Tab) for half a second shows its name in a small box under it, styled for the light and dark themes, instead of Windows' plain hover text. Moving along a row of icons shows each name at once, and a click, Escape or scrolling closes it. The accent swatches in Settings → Appearance have them too.

## 0.1.82 · 2026-11-10

### Changed

- **Words for everything you download**: History counts items rather than videos ("15 items"), and its Forget and Move Files dialogs, its empty page and the "already in the queue" messages no longer call every download a video, since posts of images and shared files are listed too.

## 0.1.81 · 2026-11-10

### Fixed

- **Pick a Color**: the last accent swatch now opens Windows' color picker in the app. It did nothing before.

## 0.1.80 · 2026-11-10

### Changed

- **A clearer light mode**: light mode now follows the dark mode's layers. The current view sits in a light gray area, cards are white on it, and fields, secondary buttons and hovered rows are a soft gray step, so cards and controls stand apart. Muted text is a little darker, for easier reading.

## 0.1.79 · 2026-11-10

### Added

- **Watched**: a new view between Pending and History for channels, playlists and accounts you want to follow. Watch, on the list a channel, playlist or account opens, asks what counts as new now (nothing yet, the latest few, or everything listed), where its downloads go (beating every rule) and the preset its videos start as. Check Now lists only what's new since; Download Selected queues the ticked items and skips the rest, which never come back, and Skipped Before keeps them downloadable. Checks are by hand for now.

### Changed

- **Shortcuts**: with Watched in the sidebar, Ctrl+3 opens it, and History, Presets, Rules and Settings move to Ctrl+4 to Ctrl+7.

## 0.1.78 · 2026-11-09

### Changed

- **Instagram's "rate-limit reached or login required"**: while a browser is signed in to Instagram, this message now counts as Instagram asking to slow down, so the download waits and carries on by itself instead of failing. Signed out, it still asks you to sign in.

## 0.1.77 · 2026-11-09

### Added

- **Pacing for X, Pixiv and Tumblr**: Settings → Instagram becomes Pacing. Instagram's pause and hourly limit stay as they were, and X, Pixiv and Tumblr can each get a pause too: their posts then wait between files, their links in Pending wait between reads, and one post of the site downloads at a time. They have none until you set one.

## 0.1.76 · 2026-11-09

### Added

- **Test a link against the rules**: a field at the top of Rules reads a link and shows the folder its download would go to, the preset it would start as, and whether copied links of it download right away, each with the rule that decides it. Nothing is downloaded.

## 0.1.75 · 2026-11-09

### Added

- **Open the download folder**: Settings → Downloads and the end of Rules have Open Folder beside the download folder.
- **Jump to a section of Settings**: a row of links at the top of Settings goes straight to each section.
- **Set Up Deno from the preview card**: the warning that Deno is missing has Set Up Deno, which sets it up and reads the link again.
- **Remove one finished download**: a finished download in the Queue has Remove from Queue (×), with Undo. It stays in the history.
- **Tick a whole group in Pending**: each group's heading has a box that ticks or unticks every link in it.

### Changed

- **Folded groups stay folded**: Pending and the Queue remember the groups you folded while Tsundoku runs, instead of unfolding them when you leave the view.
- **No repeated folder when grouped by folder**: in Pending grouped by Folder, a row shows only what picks its folder, since the heading shows the folder.

## 0.1.74 · 2026-11-09

### Added

- **Any accent color**: Settings → Appearance → Accent Color has a seventh swatch, Pick a Color, which opens Windows' color picker. Tsundoku keeps the color's hue and works out shades for light and dark mode that read as well as the six others.

## 0.1.73 · 2026-11-09

### Added

- **Logs inside the app**: Show Log on a download in the Queue opens its log under the row instead of in another program: the log's end, following along every second while it downloads, with Copy Log and Open in Editor beside it.

## 0.1.72 · 2026-11-09

### Added

- **Move downloaded files**: in History, a video's ⋯ menu has Move Files…, and ticked rows Move Selected…: their files go to a folder you pick and the history follows them, so opening them and the duplicate check still work. A post's or a shared folder's files keep their subfolders, and a name already taken gets " (2)".

## 0.1.71 · 2026-11-09

### Added

- **A folder for one download**: the preview card's destination line and each read link in Pending have Change Folder…, which saves that one download in a folder you pick, whatever the rules say, and Use the Usual Folder to go back. In the Queue, the ⋯ menu of a download that's waiting, stopped or failed has them too; a stopped one starts over in its new folder, after asking.

## 0.1.70 · 2026-11-09

### Added

- **Accent color**: Settings → Appearance → Accent Color recolors main buttons, selections, progress bars, links and the sidebar's counts: Indigo, as before, or Teal, Green, Amber, Rose or Purple, each with its own shade for light and dark mode.

## 0.1.69 · 2026-11-09

### Changed

- **Group By as buttons, with Status apart**: in Pending and the Queue, grouping by status is now a switch of its own, beside None, Site, Folder and Rule as buttons. With Status on and one of the others picked, each status holds the other groups. Grouping by status from before is kept.

### Added

- **Group by rule**: Rule groups the links or downloads by what picks their folder: the rule, shown with its number and the part of its folder that doesn't change (a rule saving in `Downloads\{site}` shows Downloads), then a preset's own folder, then the download folder.
- **Folding groups**: clicking a group's heading folds it away, and clicking again unfolds it.

## 0.1.68 · 2026-11-09

### Added

- **Add and move presets**: Presets has Add Preset under the list, which adds one set up like Best Video, and Move Up and Move Down on each preset. The default preset now always sits at the top: setting another as the default moves it there.

### Changed

- **Delete any preset but the default**: the built-in presets can now be deleted too, and put back as they ship from the ↺ menu beside Add Preset. The default can't be deleted, so another one has to become the default first.

## 0.1.67 · 2026-11-09

### Changed

- **Default in Pending's preset pickers**: a link that starts with the preset picked in Videos Start As now shows Default in its own picker, so the links you set apart stand out. Hovering it names the preset, and picking Default sends a link back to following Videos Start As. A link whose preset a rule picks still shows that preset and the rule.

## 0.1.66 · 2026-11-09

### Added

- **Only Existing Files in the history**: the Only Missing Files checkbox is now a Files list with All Files, Only Existing Files and Only Missing Files, so you can also list only the downloads whose file is still there.

## 0.1.65 · 2026-11-09

### Added

- **Update All in Settings → Tools**: one button brings every tool that's Tsundoku's own copy up to its latest version, one after the other, instead of checking each one by hand. It then says which tools it updated, or that all are up to date, and why any couldn't be updated. Copies you chose yourself are left alone.

## 0.1.64 · 2026-11-09

### Fixed

- **No browser menu on right-click**: right-clicking the window showed the WebView's own menu (Back, Reload, Save As, Print), which made no sense in the app. It now opens nothing, except in a text field, where Cut, Copy and Paste stay, and over selected text, where Copy stays.

## 0.1.63 · 2026-11-06

### Added

- **{account} and {playlist} in rules' folders**: {account} becomes the uploader's handle, such as natgeo, while {uploader} stays the name the site shows, such as National Geographic; on a site without handles, {account} is the name too. {playlist} becomes the playlist, channel or account a download was picked from in a list, such as `D:\Channels\{playlist}`, or No Playlist for a link added on its own.

### Changed

- **Rules know an uploader by either name**: a condition on the uploader now checks the handle as well as the name, so "uploader is natgeo" catches National Geographic's Instagram posts, which it missed since a post's uploader is the name the account shows.

## 0.1.62 · 2026-11-06

### Changed

- **Tidier code behind the Not Picked marks**: the line that refreshes which files of a post were left out, once a download finishes, is indented like the code around it. Nothing changes in the app.

## 0.1.61 · 2026-11-06

### Fixed

- **Instagram signed out asks for a sign-in**: an Instagram link Instagram won't show without a signed-in account said "Couldn't reach the site", and its download was tried again and again as if the connection had dropped. It now says Instagram needs you signed in, with the fixes for that (Set Up Cookies, or signing in in your browser), and isn't tried again by itself. A connection that really drops is still tried again.

## 0.1.60 · 2026-11-06

### Added

- **Thumbnails that don't go blank**: History keeps a small copy of each download's picture in Tsundoku's data folder (a post's own first image, a video's picture on the site, or a frame of the video itself), so rows keep their pictures after a site's link stops working, as Instagram's do after a while, or the post is deleted. Downloads from before get theirs in the background, as far as their files or the sites' pictures still allow. Forget deletes it with the rest.

## 0.1.59 · 2026-11-06

### Added

- **Ways to add links on the empty screens**: an empty queue now says it takes videos, posts of images and shared files, and lists every way to add links, each with where it stands and one button: Paste adds what you copied, as Ctrl+V does; Choose File… adds files of links, as dropping them does; Watch Clipboard has Turn On, or a check once it's on; the Clipboard Shortcut shows its keys, with Change or Set Up; and Send from Your Browser and the send buttons have Set Up, which opens Settings → Adding Links right at them. An empty Pending lists the same ways, the ones that work on their own first.

## 0.1.58 · 2026-11-06

### Added

- **Room checked before downloading**: before a download starts, Tsundoku checks there's room for it on its drive. One that won't fit waits instead of failing partway, with "D: has 3.1 GB free, and this needs 14.0 GB." on its row, while the downloads after it that fit go ahead. It starts by itself once there's room: the drive is looked at again every 30 seconds, whenever a download ends or is removed, and on Retry or Resume. What the downloads already running on that drive still need is set aside, and a small margin, since sizes are estimates. A shared file's size is exact; a video's is the estimate for the formats its preset picks, read with its link. A post of images, a video added before Tsundoku restarted, or one whose site gives no sizes starts as before.

## 0.1.57 · 2026-11-06

### Added

- **Pick from a channel, playlist or account**: pasting a YouTube channel (any of its tabs), a YouTube playlist or an Instagram account lists what it holds in the preview card instead of being refused: a channel's or account's newest 50 videos or posts, newest first, or a whole playlist in its own order. Each item has a box, with its thumbnail, title, date and length or what the post holds. New ones start ticked; one downloaded before is greyed and marked Downloaded and can still be ticked, one in the queue is marked In Queue. A box at the top ticks them all or none. Download Selected adds each ticked item to the queue as its own download, exactly as if its link had been pasted, with the preset a rule picks or the one chosen in Videos Start As, and asks first before replacing files downloaded with the same preset. Another site's playlist (a SoundCloud set, a Bandcamp album) is listed too when it gives each item a link of its own.

### Changed

- **Pages of several posts**: an X account's media or another page of many posts now says plainly that Tsundoku can't list that site's pages yet, instead of reading as one huge post. A channel, playlist or account link in Pending now says to paste it in the add bar to pick from it.

## 0.1.56 · 2026-11-06

### Added

- **Drop a file of links**: dropping a text file, a browser's bookmarks export (.html) or an Internet shortcut (.url) anywhere on the window sends the links in it to Pending. Several files at once work too. Links from a file are checked as copied links are: those with nothing to download, such as an ordinary page or a site no tool supports, leave on their own, counted meanwhile as "Checking 1,234 links from files…", and the rest stay as links added by hand do, marked when downloaded before. A file with more than 300 links asks first, saying about how long checking them takes. A file with no links, or of another kind, says so.

### Changed

- **Long lists in Pending**: thousands of links are added at once, the list updates a few times a second at most while they're read, and Pending shows 200 links at a time, with Show More under them.

## 0.1.55 · 2026-11-06

### Added

- **Pick which files of a post to download**: the preview card shows a post's images and videos as a strip of small pictures, with a play mark on videos and each file's number in the post, all ticked. Untick the ones you don't want and only the others are downloaded, still named after their place in the post; the queue says how many it takes ("2 of 6 files"). In Pending, Pick Files on a post's row opens the same strip, and Start downloads what's ticked there, even after a restart. The history now remembers which files each download took, so when you preview a post you downloaded only part of, the files you left out are marked Not Picked and come ticked, the others are marked Downloaded, and Download Again gets just the rest. Posts downloaded before this version count as taken whole.

## 0.1.54 · 2026-11-06

### Added

- **More History filters**: show only videos, audio, posts or shared files; only what you downloaded today, in the past 7 or 30 days, in the past year or between two dates; or only the downloads whose file is missing.
- **Act on several downloads at once**: tick rows in History, or all of them with the box above the list (everything the search and filters show, even what isn't on screen yet), then Forget Selected, which asks once, or Show Selected in Folder, which opens each folder once with its files selected and asks first when that's more than 5 folders. Changing the search or a filter unticks them.

### Changed

- **Forget says what it does**: the confirmation explains that Tsundoku won't warn before the video is downloaded again, instead of naming a file only the tools use.

## 0.1.53 · 2026-11-06

### Added

- **Send buttons on Instagram, X and YouTube**: a userscript puts a Send to Tsundoku button on each Instagram post and reel, each X post, YouTube's watch pages and Shorts, and in the corner of YouTube's thumbnails in feeds, search results, channels and the list beside a video. One click sends that post or video to Pending, as the bookmark does, without opening it or copying anything, and a sent one keeps a check while the tab is open. Settings → Adding Links → Copy Userscript copies it for the Tampermonkey or Violentmonkey add-on, with how to add it.

## 0.1.52 · 2026-11-06

### Added

- **Downloads recover by themselves**: a download whose connection drops is tried again after 30 seconds, then 2 minutes, then 8 minutes, continuing from what it got, before it shows as failed. Its row says so, such as "The connection dropped. Trying again at 2:32 PM (2 of 3).", with the time left. When MEGA's, Google Drive's or Dropbox's download limit is used up, the download waits too, 30 minutes, then an hour, two, and up to four hours each time it's still used up, and starts again by itself instead of failing. A waiting row has Try Now, which starts it without waiting any longer (for a site that asked to slow down, all of that site's downloads go on), and can still be stopped or cancelled.

## 0.1.51 · 2026-11-06

### Added

- **What's New after an update**: the first start on a newer version shows a card under the add bar with what changed since the version you last ran, as this changelog says it, until you close it. A new install shows nothing.
- **What's New before updating**: when a newer Tsundoku is out, its banner has a What's New link that shows the new version's changes before you install it, or Release Notes, which opens its page, when the release came without them.

## 0.1.50 · 2026-11-06

### Changed

- **A history that stays quick at thousands of downloads**: History now reads your downloads a page at a time as you scroll and draws only the rows on screen, so it opens, searches and scrolls at once however long it gets: with 5,000 downloads it opens in about a fiftieth of a second instead of a second and a half. A search runs as soon as you pause typing, and a row you expanded is still expanded when you scroll back to it.

## 0.1.49 · 2026-11-06

### Added

- **Waiting when a site asks to slow down**: when Instagram, or any other site, answers that it's getting too many requests, the download no longer fails, and neither do the site's next ones. It goes back to waiting, keeping what it got, and the site's other downloads, the links Pending reads and the preview card wait with it, while other sites carry on. Each row says so, such as "Instagram asked to slow down. Continuing at 2:32 PM.", with the time left counting down, then it continues by itself. The first wait is 5 minutes, then 10, 20, 40 and at most an hour each time the site asks again before anything from it works.
- **An hourly limit for Instagram**: Settings → Instagram → Hourly Limit, off to start with, keeps you further from Instagram's limits. Once the past hour holds that many Instagram files, the next Instagram download waits until older ones are an hour old, says so on its row, then starts by itself.

## 0.1.48 · 2026-11-06

### Added

- **Undo after removing**: removing links in Pending (Remove Selected, Delete or a row's Remove) and Clear Finished in the queue show "Removed 12 links" or "Cleared 3 finished downloads" at the bottom of the window for 8 seconds, with Undo. It puts back exactly what went: the same links or downloads, in the same places, with their presets and what was read, and links that were ticked come back ticked. The message waits while the pointer is on it or it has the keyboard's focus, and Escape or its × closes it. Only the latest removal can be undone, and a link or download that's back by another way by then isn't added twice.

## 0.1.47 · 2026-11-06

### Added

- **Image downloads keep working by themselves**: Tsundoku's copy of gallery-dl, the tool behind posts of images, now updates itself once a day like yt-dlp's, so a change at Instagram or X no longer leaves posts failing until someone thinks to update it. It's only downloaded again when a newer build is out. In Settings → Tools, its Check for Updates is now Update Now, and both tools say they update once a day.
- **Daily updates while Tsundoku keeps running**: yt-dlp and gallery-dl are now updated whenever Tsundoku stays open past a day, in the tray with Start with Windows, say, not only when it starts.

### Fixed

- **Updates no longer pull a tool from under a download**: a new gallery-dl only replaces the old one once no preview, read or download is using it, and the daily updates of yt-dlp and gallery-dl wait until the tool isn't running. Meanwhile Update Now shows "Waiting for the downloads using it to finish…", for up to five minutes. A failed or cancelled update leaves the old copy working.

## 0.1.46 · 2026-11-06

### Fixed

- **Copy Details' labels**: the Copy Details button on a failed download now says its name when you hover it, and Copied once it has copied. Both were missing from 0.1.41, where the button came in.

## 0.1.45 · 2026-11-06

### Added

- **Notifications for failed and finished downloads**: while Tsundoku's window isn't in front, a notification says when a download failed ("Couldn't Download" and its title, or "Couldn't Download 3 Links" when several fail together), and when every download started since the queue was last idle has ended ("All 12 downloads finished", or "11 of 12 downloads finished · 1 failed"). Cancelled downloads don't count. Settings → Adding Links has a switch for each, Notify When a Download Fails and Notify When Downloads Finish, both on to start with. The switches came with 0.1.41, but the notifications themselves only work from this version.

## 0.1.44 · 2026-11-06

### Added

- **More placeholders for rules' folders**: besides {site} and {uploader}, a folder can use {title}, {id}, {kind} (Videos, Audio, Posts or Files), {preset}, the upload date's {year}, {month} and {date}, and {added}, the day you added the download. Each has a button under the folder field that says what it becomes when you hover over it.
- **Patterns in rules**: a condition on the site, uploader, title or link can use Matches or Doesn't Match with a regular expression, ignoring case. A pattern that isn't valid is caught when you save, with what's wrong with it. What a pattern captures in parentheses can go in the folder as {1}, {2} and so on: title matches `^\[(.+?)\]` → `D:\Series\{1}`.

## 0.1.43 · 2026-11-06

### Added

- **Rules that pick the preset too**: a rule can now say how a download starts as well as where it goes, such as "When the site is SoundCloud, start as Audio MP3". Its editor has Start As (a preset, or Leave as Is) and Download Right Away (On, Off or Leave as Is), and a rule does any of the three, so it can leave the folder to a rule below it. The preview card preselects the rule's preset with "Rule 3" beside it, which opens the rules, and Pending's rows, Start All and links that download right away use it too; picking another preset on the card or a row still wins. Download Right Away On or Off in a rule beats Settings for the copied, shortcut and browser links it matches. A rule that checks the kind or the preset can't pick the preset, since both depend on it, and if a rule's preset is deleted the default one is used. Rules saved before work as they did.

## 0.1.41 · 2026-11-06

### Added

- **Copy Details for a bug report**: a failed download in the queue has a Copy Details button. It copies what a bug report needs as plain text: Tsundoku's version, each tool's version, Windows' version and build, the link, the error shown and the last 50 lines of the download's log. Your user name is blanked out everywhere in it, in paths such as C:\Users\<user> and on its own, so it can be pasted into an issue as it is. The button shows Copied for a moment once it's done.

## 0.1.40 · 2026-11-06

### Added

- **Readable names for posts' files**: the files of an Instagram, X or other image post are now named after the account, the post's date, its ID and their place in it, such as "natgeo 2026-11-04 DfPQchkG1tg 1.jpg", instead of the long number the site gives them, so they still say whose post they came from once they're moved elsewhere. Settings → Posts of Images → File Name changes the name, with Insert buttons for {account}, {date}, {post}, {number} and {site} and an example of the result under the field. A name without {post} and {number}, which could give two files the same name, or with a character Windows refuses, isn't saved. Files downloaded before keep their names; downloading a post again saves it under the new one.

## 0.1.39 · 2026-11-06

### Added

- **Progress on the taskbar and in the tray**: Tsundoku's taskbar button fills with the progress of the downloads started since the queue was last idle. It turns yellow while they're all stopped, red once one failed, and moves across while no size is known yet. Hovering the tray icon sums them up, such as "3 downloading · 45% · 1 failed", instead of always reading Tsundoku.

## 0.1.38 · 2026-11-06

### Added

- **Keyboard shortcuts**: Ctrl+V anywhere in the window but a text field adds the links you copied, as if pasted in the add bar and added: one link shows its preview, several go to Pending. Ctrl+L goes to the add bar, Ctrl+1 to Ctrl+6 open Queue, Pending, History, Presets, Rules and Settings in the sidebar's order, and Delete removes the links ticked in Pending. Hovering the sidebar, the add bar or Remove Selected shows their keys, and Settings → Keyboard Shortcuts lists them all.

## 0.1.37 · 2026-11-06

### Added

- **Back up and restore**: Settings → About's Back Up… saves your settings, presets, folder rules and history in one file, such as Tsundoku Backup 2026-11-06.zip, with what you downloaded before, so the warnings about downloading something again keep working. Restore… loads one on another PC or after a reinstall: it replaces those with the backup's, then restarts Tsundoku. What it replaces is saved first as a backup in the data folder's backups folder, so a wrong restore can be undone. The tools, logs, the queue and Pending aren't in a backup, and a restore keeps which tools this PC uses and whether Tsundoku starts with Windows. A backup from a newer Tsundoku is refused.

## 0.1.35 · 2026-11-06

### Added

- **Buttons for the whole queue**: on the queue's title's right, Pause All stops every running download and holds every waiting one, Resume All resumes every stopped one, and Retry Failed retries every failed one. Each does what the row's own button does and shows only while a download needs it. The tray menu has Pause All Downloads too, which reads Resume All Downloads while every download is stopped.
- **Download Next**: a waiting download's ⋯ menu moves it to the front of the line, so it starts before the others waiting, even after a restart.

## 0.1.34 · 2026-11-06

### Changed

- **A new dark mode**, after RJ Library's: the current view now sits darker than the window around it, with cards and fields lighter again, all in plain grays with no tint. Lines are easier to see, supporting text is a softer gray, and the main buttons, the selected choice of a switch and the counts in the sidebar are a deep indigo with white text instead of light blue with dark text. Buttons that confirm a deletion are dark red with white text. Light mode doesn't change.

## 0.1.33 · 2026-11-06

### Added

- **Group and sort Pending and the queue**: a toolbar above each list groups it by the folder each download will be saved in, its site or where it stands (ready to start, downloaded before, not read yet… in Pending; downloading, waiting, stopped, failed or done in the queue), under a heading per group with how many it holds, and sorts it by date added, title, uploader or site, either way. Pending and the queue each keep their own choice through a restart. To start with, nothing changes: no groups, oldest first.

## 0.1.32 · 2026-11-06

### Added

- **Light, Dark or Auto**: Settings → Appearance picks Tsundoku's theme. Auto, the default, follows Windows' light or dark mode as before; Light and Dark keep it so whatever Windows uses. The colors, title bar, Mica backdrop, scrollbars and menus all change at once, and the next start opens in the chosen theme from its first frame.

## 0.1.31 · 2026-11-06

### Changed

- **Icons for the actions on each row**: the buttons repeated on every row or card are now icons that show their name when you hover them, even while they're unavailable: Start, Try Again and Remove in Pending; Stop, Resume, Retry, Open File, Show in Folder, Open Log and Cancel in the queue; Edit in Rules; Duplicate, Reset and Delete in Presets; Open File and Show in Folder under the preview card's "already downloaded" warning. The preview card closes with its × at the top right while a link is read or after it fails too, instead of Cancel and Dismiss. In the history, Open Source Page leaves the ⋯ menu for an icon beside Open File and Show in Folder. A view's main actions (Add, Download, Start All, Clear Finished, Add Rule) keep their words.

## 0.1.30 · 2026-11-06

### Added

- **Pending shows where each link will be saved**: under a link's details, once it's read, the folder it will go in and what picked it (a rule, the preset's folder or the download folder), as the preview card shows it. It follows the preset the link would start with, and clicking it opens where that's set.

### Changed

- **The last rule takes everything else**: the rule every install starts with, always last, now takes every download no other rule takes, not only posts of images, so videos, audio and shared files also go in a folder for their site, such as Downloads\Tsundoku\YouTube. It reads "Everything else" and has no conditions. Its folder can still be changed, or it can be turned off to save those downloads in the download folder itself. A preset with a folder of its own still saves there. Your posts rule becomes it, keeping its folder and whether it's on.

## 0.1.29 · 2026-11-06

### Changed

- **Copied file host links Tsundoku can't read stay in Pending**: a MEGA, MediaFire, Google Drive or Dropbox link that isn't one Tsundoku recognizes, such as a kind of link it doesn't know yet, stays with "This MediaFire link isn't one Tsundoku recognizes" instead of leaving without a word, so a file you meant to download doesn't go missing. Pasting one says the same, instead of that the site isn't supported. Links on sites Tsundoku doesn't know still leave quietly.

## 0.1.28 · 2026-11-06

### Fixed

- **MediaFire's direct links**: the link behind a MediaFire page's Download button (download1234.mediafire.com/…), which download managers' browser add-ons and userscripts copy, now counts as that file's link. Watch Clipboard used to drop it without a word, and pasting it said Tsundoku couldn't download from the site.

## 0.1.27 · 2026-11-06

### Changed

- **Settings → Instagram**: the card once called Posts of Images is named for what it sets, the pause between Instagram files, and no longer points to Rules. The pause now starts at 5 to 10 seconds instead of 10 to 20; a range you already saved stays as it is.

## 0.1.26 · 2026-11-06

### Fixed

- **The setup's options show all their text**: a long path under an option, such as a tool found on the PC, wraps onto more lines instead of being cut off.

## 0.1.25 · 2026-11-06

### Changed

- **gallery-dl is set up at the first start**, with yt-dlp, ffmpeg and Deno: image posts are as much what Tsundoku is for as videos. A copy already on the PC, such as an installed Tsundoku's in AppData, is used as it is. If only Deno or gallery-dl fails to download, Continue Without leaves them for Settings → Tools. Installs already set up aren't asked again.

## 0.1.24 · 2026-11-06

### New

- **Tools in AppData for a portable copy**: the first screen's Tools card offers In AppData, where an installed Tsundoku keeps its tools, so the two can share them. When an installed copy's tools are already there, it's picked, and the next screen uses them as they are instead of downloading them again.

## 0.1.23 · 2026-11-06

### Fixed

- **Google Drive folders**: a folder holding an empty file no longer fails to load with a message about Google Docs. The empty file downloads like the rest, and a link to one empty file works too.

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

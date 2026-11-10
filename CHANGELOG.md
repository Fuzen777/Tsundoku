# Changelog

All notable changes to Tsundoku. Versions follow [Semantic Versioning](https://semver.org/): every change bumps the patch number, and a release bumps the minor or major number, folding in the patch entries since the previous release.

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

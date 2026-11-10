# Testing

What to check by hand in the real app. Every change here passed `Scripts\Check.cmd` (formatting, Clippy, the Rust and UI tests), but none has been confirmed by you in the app yet. Most were only seen in unit tests or in a browser preview with stand-in data; an item says when Claude also tried it in the real app.

Tick a box once it works. When something is off, tell Claude in the Tsundoku project with the item's number and what you saw.

Up to date with 0.1.80 (2026-11-10).

## Decisions Waiting on You

Decisions live on the [Open Decisions](https://claude.ai/artifact/A8Ewo1fPXDbs7CP6hgzwFj) page, with everything else waiting on your call. Pick a suggested answer or write your own there; threads read the answers from the page.

## Before You Start

1. Quit every running Tsundoku, including one in the tray (right-click its icon, Quit Tsundoku). Only one copy runs at a time, and the installed one in `D:\Apps\Tsundoku` is still 0.1.17, so starting another build would only bring that one to the front.
2. Run `Scripts\Build-Portable.cmd`. It builds 0.1.63 as a release build into `D:\Dev\tsundoku\portable`, which keeps its settings, history and tools in its own `data` folder, so your real history isn't touched. (`Scripts\Dev.cmd` works too, with its data in `D:\Dev\tsundoku\data`.)
3. Start `D:\Dev\tsundoku\portable\Tsundoku.exe`. Settings → About should say 0.1.63 and Portable.
4. Keep Zen signed in to Instagram and X, and these at hand:
   - A short video: `https://www.youtube.com/watch?v=jNQXAC9IVRw` (19 s)
   - A long one, for stopping and progress: `https://www.youtube.com/watch?v=aqz-KE-bpKQ` (Big Buck Bunny, 250 MB at 1080p)
   - An Instagram carousel of 6 files (2 images, 4 videos): `https://www.instagram.com/p/DfPQchkG1tg/`
   - A download that fails on purpose: add a rule "When the title contains zoo" with Save In set to a drive you don't have, such as `Z:\Nowhere`. The short video then fails with "Couldn't create the download folder." Turn the rule off when you're done.

## Look and Feel

**1. Light, Dark or Auto** · 0.1.32
- [ ] Settings → Appearance → Theme: switching between Auto, Light and Dark changes everything at once: colors, title bar, Mica, scrollbars and dropdown lists.
- [ ] Pick the opposite of Windows' mode, quit and start again → it opens in the chosen theme from the first frame, with no flash of the other one.

**2. RJ Library–inspired dark mode** · 0.1.34 · *needs your approval*
- [ ] In Dark, the current view sits darker than the sidebar and title bar, with cards and fields lighter again, in plain grays. Main buttons (Add, Download, Start All), the selected choice of a switch and the sidebar's counts are deep indigo with white text; buttons confirming a deletion are dark red with white text. [Before and after](https://claude.ai/artifact/3DE2KntFCUKpBc8ofHyHjd) (browser shots, so without Mica).
- [ ] Light mode looks exactly as it did.

**48. Accent color** · 0.1.70
- [ ] Settings → Appearance → Accent Color: picking Teal, Green, Amber, Rose or Purple recolors at once the primary buttons, the selected segments, the sidebar's counts and selection pill, checkboxes, progress bars and links. The picked swatch has a check.
- [ ] Pick a Color (the last swatch, with a pipette) → Windows' color picker → pick a bright orange → the app turns orange in both themes, readable, and the swatch gets the check. Pick another swatch and back: the orange is remembered. (0.1.74)
- [ ] Pick a Color opens Windows' color picker in the real app (it did nothing up to 0.1.80), over the window, starting on the custom color, with the full panel open. The window can't be clicked while it's open. Cancel → nothing changes. OK on a color → the app takes it at once. Add a color to Custom Colors, close, open it again → it's still there (until the app restarts). Not tried in the real app yet. (0.1.81)
- [ ] Each one reads well in Light and in Dark (white text on filled buttons, links readable on the background). Restart → the accent is there from the first frame.

**57. Tooltips on icon buttons** · 0.1.83
- [ ] Rest the pointer on any icon button (a queue row's Stop, the sort direction, the preview card's ×) → after about half a second its name shows in a small box under it, matching the theme in Light and in Dark. Seen in the browser preview, not in the real app yet.
- [ ] Slide along a row of icons → each name shows at once. Click one → its tooltip goes and doesn't come back until the pointer leaves. Escape or scrolling the list closes it.
- [ ] A greyed-out icon button still shows its name. Tab onto an icon button → its name shows; clicking one with the mouse doesn't.
- [ ] Near the window's right or bottom edge, the box stays inside the window (above the button at the bottom). Windows' own plain hover text no longer appears on icon buttons or the accent swatches.

**3. Icons for row actions** · 0.1.31
- [ ] The actions on each row are icons in Pending, the Queue, Rules, Presets and under the preview card's "already downloaded" warning. Hovering each shows its name, even while it's greyed out.
- [ ] The preview card closes with its × at the top right, also while a link is being read and after it fails.
- [ ] In History, Open Source Page is an icon beside Open File and Show in Folder.

**42. No browser menu on right-click** · 0.1.64
- [ ] Right-click a row, a button or an empty spot in any view → nothing opens (no Back, Reload, Save As, Print or Inspect).
- [ ] Right-click in the add bar or a search field → the usual Cut, Copy, Paste menu. Select a title's text and right-click it → Copy.

**4. What's New** · 0.1.51
- [ ] The first start after rebuilding the portable copy shows a What's New card under the add bar with the changes since the version that copy last ran (only 0.1.63's if it last ran before 0.1.51, which didn't record it). Its × closes it, and it doesn't come back at the next start.
- Can't be tested yet: the update banner's What's New link, which needs a published release that the app can fetch.

## Adding Links

**5. Pending tab** · 0.1.14
- [ ] Paste three or more links at once in the add bar (one per line) → they go to Pending instead of the preview card. Each row fills in one at a time (thumbnail, title, site); one downloaded before is marked, with Download Again. The sidebar count matches.
- [ ] Quit and start again → the links are still there.
- [ ] Pick a preset in Videos Start As, then Start All → every link not downloaded before goes to the queue with it; a row's own preset picker wins. Ticking links gives Start Selected and Remove Selected.
- [ ] With Downloads at Once at 2, start two Instagram posts → the second waits until the first is done, while a YouTube video goes ahead.

**6. Watch Clipboard and Download Right Away** · 0.1.15
- [ ] Turn on Watch Clipboard (the box on Pending, or Settings → Adding Links), then copy a YouTube video's link in Zen → "Checking 1 copied link…", then it's in Pending.
- [ ] Copy an ordinary page (a news article), a link you already downloaded, a direct image link and a sentence with a link in it → none of them shows up.
- [ ] Copy a password from your password manager → nothing happens.
- [ ] Turn on Download Right Away and copy a video's link → it starts downloading once it's read, with the default preset (or a rule's).

**7. Clipboard Shortcut** · 0.1.17
- [ ] Settings → Adding Links → Clipboard Shortcut: pick Ctrl+Alt+D. Copy two links in Notepad and press it with Tsundoku in the background → both are in Pending, even with Watch Clipboard off.
- [ ] If another app already uses one of the four choices, picking it is refused with a message, and the shortcut picked before stays.

**8. Send from Your Browser** · 0.1.18
- [ ] Settings → Adding Links → Send from Your Browser → Copy Bookmark Address, then add a toolbar bookmark in Zen with that as its address.
- [ ] Click it on a YouTube video's page → Zen asks once whether to open Tsundoku, the video lands in Pending, Zen stays in front, and a notification says what was added.
- [ ] With Keep Running in the Tray on, quit Tsundoku and click the bookmark again → Tsundoku starts in the tray and adds the page.
- Note: the bookmark opens whichever copy started last (portable, installed or dev).

**9. Keyboard shortcuts** · 0.1.38
- [ ] Copy a link, click an empty spot in the window (not a text field) and press Ctrl+V → its preview opens. With several links copied → they go to Pending.
- [ ] Ctrl+L → the add bar, its text selected. Ctrl+1 to Ctrl+6 → Queue, Pending, History, Presets, Rules, Settings.
- [ ] In Pending, tick links and press Delete → they're removed (with Undo). In a text field, Delete only edits the text.
- [ ] Hovering a sidebar item says its key ("Queue (Ctrl+1)"), and Settings → Keyboard Shortcuts lists them all.

**10. Send buttons on Instagram, X and YouTube** · 0.1.53
- [ ] Settings → Adding Links → Copy Userscript, and add it to Tampermonkey or Violentmonkey in Zen as the row explains.
- [ ] A Send to Tsundoku button shows on Instagram after Share (feed posts, post pages, reels), in each X post's row of actions, beside Share on a YouTube watch page, in a Short's column of actions, and in the corner of YouTube thumbnails on hover (feed, search, channels, the list beside a video).
- [ ] Clicking one sends that post or video to Pending without opening it, and the button keeps a check while the tab stays open.
- Claude tried YouTube signed out and Instagram's public post and reel pages in a scripted browser. The signed-in Instagram feed and X weren't tried.
- Known: the script updates itself from GitHub's main branch, so it can't until main is pushed and readable.

**11. Drop a file of links** · 0.1.56 · *one open decision*
- [ ] Export Zen's bookmarks (Ctrl+Shift+O → Import and Backup → Export Bookmarks to HTML) and drop the file on the window → "Checking N links from files…" in Pending. Pages with nothing to download leave on their own; videos and posts stay. Over 300 links, a Windows dialog asks first, with about how long it takes.
- [ ] Drop a .txt with a few links, and a .url shortcut (drag a link from Zen onto the desktop to make one) → their links go to Pending.
- [ ] Drop a file without links → "No links found in" and the file's name.
- [ ] With hundreds of links, Pending shows 200 at a time, with Show More under them.

**12. Ways to add links on the empty screens** · 0.1.59
- [ ] With the queue empty (Clear Finished), it lists every way to add links, each with where it stands and one button: Paste, Choose File…, Watch Clipboard (Turn On, or a check once on), Clipboard Shortcut (its keys with Change, or Set Up), and Send from Your Browser and the send buttons (Set Up opens Settings → Adding Links right at them).
- [ ] Paste adds what you copied; Choose File… adds a file's links as a drop does.
- [ ] An empty Pending lists the same ways, the ones that work on their own first.

**13. File host links that Pending keeps** · 0.1.28, 0.1.29
- [ ] With Watch Clipboard on, copy the link behind a MediaFire page's Download button (`download1234.mediafire.com/…`, the one your userscript copies) → it's read as that file and stays in Pending.
- [ ] Copy a MEGA link that isn't a file or a folder, such as `https://mega.nz/about` → it stays in Pending with "This MEGA link isn't one Tsundoku recognizes" instead of vanishing.

## Previews and Picking

**14. Pick from a channel, playlist or account** · 0.1.57
- [ ] Paste a YouTube channel (such as `https://www.youtube.com/@NASA`, any tab) → "Listing the channel's newest videos…", then its newest 50 videos, newest first, each with a box, thumbnail, title, rough date and length. New ones are ticked; downloaded ones are greyed, unticked and marked Downloaded (hovering says when); queued ones say In Queue.
- [ ] A playlist is listed whole, in its own order. A `watch?v=…&list=…` link is still that one video.
- [ ] An Instagram account (`https://www.instagram.com/natgeo/`) lists its newest 50 posts and reels; it can take a minute.
- [ ] The box at the top ticks all or none and counts them. Download Selected queues each ticked item as its own download, with a rule's preset or the one in Videos Start As.
- [ ] An X account's media page says plainly that Tsundoku can't list it yet. A channel link pasted with others waits in Pending, saying to paste it in the add bar.

**15. Pick which files of a post to download** · 0.1.55
- [ ] Paste the Instagram carousel → a strip of 6 small pictures with their numbers and a play mark on videos, all ticked. Untick 2 to 5 and Download → the queue row says "2 of 6 files", and only files 1 and 6 arrive.
- [ ] Paste it again → 2 to 5 are marked Not Picked (with a ring) and ticked, 1 and 6 Downloaded; Download Again gets just the rest.
- [ ] In Pending, a post's row has Pick Files, which opens the same strip. Untick some, restart: the ticks are kept, and Start downloads only those.
- [ ] Pick files of an X post with several images.
- Claude tried the first check in the real app: exactly files 1 and 6 arrived. Pending's strip and X weren't tried.
- Known: Start All skips a post downloaded before, even partly; use its row's Download Again.

**16. Readable names for posts' files** · 0.1.40
- [ ] Download an Instagram post → its files are named like `instagram 2026-11-02 DfPQchkG1tg 1.jpg`: the account's handle, the post's date, its ID and the file's number. An X post's files carry the account's handle too.
- [ ] Settings → Posts of Images → File Name: the Insert buttons put {account}, {date}, {post}, {number} and {site} at the cursor, the example under the field follows, and Use Default puts it back.
- [ ] A name without {post} or {number}, with an unknown {word}, or with a `:` isn't saved and says why.
- Claude checked the Instagram names in a real download.

**17. Instagram signed out asks for a sign-in** · 0.1.61
- [ ] Turn cookies off in Settings → Cookies, then paste `https://www.instagram.com/natgeo/` → it says Instagram needs you signed in, with Set Up Cookies, not "Couldn't reach the site". A download that hits this fails once and isn't retried by itself. Put cookies back to Automatic afterwards.

**18. MEGA, MediaFire, Google Drive and Dropbox** · 0.1.13, 0.1.16, 0.1.21, 0.1.23
- [ ] Paste a link of each, to a file and to a shared folder → name, date and size ("14 files, 1.2 GB") on the card, and Download without a preset. A folder keeps its own folders; files already there are skipped; Stop then Retry continues where it stopped.
- [ ] A MEGA link cut off before its key (the part after #) → a message saying so.
- Claude downloaded real links of all four on your PC, so this is a quick look in the app.

## Pending and the Queue

**19. Group and sort** · 0.1.33
- [ ] Pending's toolbar groups by Folder, Site or Status, or not at all, each group under a heading with its count (a folder group shows its path), and sorts by Date Added, Title, Uploader or Site, either way.
- [ ] The Queue has the same toolbar (status groups: Downloading, Waiting, Stopped, Failed, Done). Sorting doesn't change the order downloads start in.
- [ ] Restart → each view keeps its own choice.

**45. Default in Pending's preset pickers** · 0.1.67
- [ ] Pending, with a video link no rule picks a preset for: its picker shows Default, and hovering it says "Follows Videos Start As: <preset>". Changing Videos Start As changes what it starts with, still showing Default.
- [ ] Pick another preset on that row, then Default again → it follows Videos Start As once more. A row whose own preset is the one Videos Start As then gets also shows Default. A row a rule picks for shows the preset's name with the rule's link, and no Default.

**47. Group By's new controls** · 0.1.69
- [ ] Pending and the Queue: Group By shows Status as its own toggle, then None, Site, Folder and Rule side by side. A view grouped by status before keeps that, as the Status toggle with None.
- [ ] Rule: each group reads "Rule 2 · D:\Videos" (a rule saving in `D:\Videos\{site}` shows `D:\Videos`), then "Preset's Folder · …" and "Download Folder · …" for downloads no rule places.
- [ ] Status with Site, Folder or Rule: each status's heading holds the other groups under smaller, indented headings. Links not read yet sit under their status without a heading of their own.
- [ ] Clicking a heading folds its group, and again unfolds it, and it stays folded when you come back to the view. Restart → the grouping is kept (the folding isn't).

**49. A folder for one download** · 0.1.71
- [ ] Preview card: the folder icon after the destination line → pick a folder → the line shows it "· Picked for This One"; Download → it's saved there, whatever the rules say. The arrow icon goes back to the usual folder. Same for a post and a shared folder.
- [ ] Pending: Change Folder… on a read link, restart, the folder is still there; Start → it downloads there. Grouped by Folder or Rule, it sits under its folder or "Picked Folder · …".
- [ ] Queue: ⋯ → Change Folder… on a waiting or failed download → it downloads there (a failed one after Retry). On a stopped one it asks first, then starts over in the new folder, leaving no `.tsundoku-<n>` folder behind in the old one. Use the Usual Folder puts it back.

**20. Where each Pending link goes, and "Everything else"** · 0.1.30
- [ ] A read link's row shows the folder it will be saved in and what picked it (a rule, the preset's folder or the download folder). It follows the row's preset, and clicking it opens where that's set.
- [ ] With no other rules, a YouTube video goes to the download folder's YouTube folder by "Everything else". Turning that rule off sends it to the download folder itself.

**21. Undo after removing** · 0.1.48
- [ ] In Pending, tick two links and Remove Selected → "Removed 2 links" shows at the bottom for 8 seconds, with Undo, which puts them back in their places, still ticked, with their presets.
- [ ] The message stays while the pointer is on it; Escape or its × closes it.
- [ ] Clear Finished in the Queue → "Cleared N finished downloads" with Undo, which puts them back.

**22. Buttons for the whole queue, and Download Next** · 0.1.35
- [ ] With two long downloads running, Pause All stops them (and holds waiting ones), Resume All resumes them, and with a failed one, Retry Failed retries it. Each shows only while a download needs it.
- [ ] The tray menu has Pause All Downloads, which reads Resume All Downloads while every download is stopped.
- [ ] With Downloads at Once at 1, queue three videos. The third's ⋯ menu → Download Next → it starts right after the current one, before the second, also after a restart in between.

**51. Logs inside the app** · 0.1.73
- [ ] Queue: Show Log on a running download → its log opens under the row and grows every second, staying at the bottom; scroll up and it stays put. Hide Log closes it.
- [ ] On a done or failed download, Show Log shows the whole log; Copy Log copies it, Open in Editor opens the file as before.
- [ ] A very long log (a channel's playlist, say) says "The log is long: this is its end." and the app stays quick.

**52. Small additions from the open decisions** · 0.1.75
- [ ] Settings → Downloads: Open Folder opens the download folder in File Explorer (made first if it wasn't there). The same button sits at the end of Rules.
- [ ] Settings: the links at the top (Downloads … About) each scroll to their section.
- [ ] With Deno not set up (rename `tools\deno` with the app closed), a YouTube preview's warning has Set Up Deno → it installs, and the preview is read again without the warning.
- [ ] Queue: a finished download's × (Remove from Queue) removes it with "Removed “…” from the queue" and Undo, which puts it back in its place. History still has it.
- [ ] Pending grouped by Folder: rows show "Rule 3" (or what else picked the folder) without the path; the box on a heading ticks every link in that group, part-ticked when only some are.
- [ ] Fold a group in Pending, go to the Queue and back → it's still folded. A restart unfolds it.
- Seen in the browser preview with stand-in data.

**23. Copy Details** · 0.1.41, 0.1.46
- [ ] On a failed row (the `Z:\` rule above), hovering Copy Details says its name, and it says Copied after a click. Pasted in Notepad: Tsundoku's version, each tool's version, Windows' version and build, the link, the error and the log's last lines, with your user name blanked out.

## Waiting and Retrying

**24. Room checked before downloading** · 0.1.58
- [ ] If you have a small USB stick or a nearly full drive: make a rule save there and queue Big Buck Bunny at Best Video → it waits with "E: has X free, and this needs Y." while the downloads after it that fit go ahead. Free some space → it starts within 30 seconds.
- Known: posts, videos added before a restart and videos whose site gives no sizes aren't checked.

**25. Hourly limit for Instagram** · 0.1.49
- [ ] Settings → Pacing → Hourly Limit: set 3, then download Instagram posts → once the past hour holds 3 Instagram files, the next Instagram download waits, says so on its row, and starts by itself later. Set it back to none afterwards.

**26. Waiting when a site asks to slow down** · 0.1.49 · *watch for it*
- [ ] When Instagram (or any site) answers that it's getting too many requests, the download goes back to waiting instead of failing: "Instagram asked to slow down. Continuing at 2:32 PM." with the time left counting down. The site's other downloads, its links in Pending and the preview card wait too, other sites carry on, and it continues by itself. Try Now on a waiting row ends the site's wait.
- Hard to cause on purpose, so check it when it happens.
- [ ] Since 0.1.78, Instagram's "rate-limit reached or login required" waits the same way while Zen is signed in to Instagram; with Settings → Cookies at None it fails asking for a sign-in instead.

**27. Downloads recover by themselves** · 0.1.52
- [ ] Start Big Buck Bunny, then turn the network off until its row changes (yt-dlp retries on its own first, which can take a minute) → "The connection dropped. Trying again at … (1 of 3)." with the time left. Try Now starts it at once; with the network back, it continues from what it had.
- [ ] Pause All stops a waiting row; Resume starts its retries over.
- [ ] Watch for a MEGA, Google Drive or Dropbox download limit → it waits 30 minutes, then an hour, two and four, and never fails for it.
- Not tried against a real dropped connection or host limit.

## Presets

**46. Adding, moving and deleting presets** · 0.1.68
- [ ] Presets → Add Preset → "New Preset" appears at the end of the list, set up as Best Video, and opens. Rename it.
- [ ] Move Up and Move Down move the open preset; Move Up is greyed out right under the default, Move Down at the end. The default has neither, and no Delete.
- [ ] Set as Default on another preset → it jumps to the top with the Default badge, and the previous default can now be moved and deleted.
- [ ] Delete a built-in such as Best Audio (after making another the default if it was) → a ↺ button appears beside Add Preset; its menu's Put Back Best Audio adds it again as it ships.
- [ ] The preview card's, Pending's and the rules' preset lists follow the new order.

## Rules

**61. Drag rules into place** · 0.1.87
- [ ] Rules → hover a rule's grip (left of its number): the cursor is a hand and "Drag to Move" shows. Drag it up or down: the whole row follows the pointer and a line shows where it will go. Dropping saves the new order at once, numbers follow, and the rule that decides changes accordingly.
- [ ] Dragging above Everything Else puts the rule just under it; Everything Else has no grip. Dropping a rule where it already is changes nothing. While a rule is being edited, the grips are dimmed and don't drag.
- [ ] Dragging a link from a browser onto the Rules view still opens it in the add bar, as anywhere else.
- Seen in the browser preview with simulated drag events (the line shows, a drop saves); not dragged by hand.

**60. The last matching rule wins** · 0.1.86
- [ ] With rules from before (at least two of your own), open Rules after updating: Everything Else is first with "Always First", and your rules are in reverse order. Test a Link on a link two of them match gives the same folder and rule as before (now numbered from the other end).
- [ ] Add Rule puts the new rule at the bottom. Two rules that match the same link: the lower one decides, and moving it up makes the other decide. Everything Else can't move, and the rule below it can't move up.
- [ ] Restart: the order stays, and isn't turned upside down again.
- Not tried in the app or the browser preview.

**59. Duplicate a rule** · 0.1.85
- [ ] Rules → the copy icon (Duplicate) beside a rule's Edit opens the editor just below it, numbered one more, with the same conditions, folder, preset and Download Right Away. Change it, Save → both rules are there, the copy right after the original, and the rules below are numbered one further.
- [ ] Duplicate, then Discard → nothing is added. Everything Else has no Duplicate, and its buttons line up with the other rows'.
- Not tried in the app or the browser preview.

**55. The light mode revamp** · 0.1.80
- [ ] Settings → Appearance → Light: the current view sits in a light gray area, its cards are white, and fields, secondary buttons and hovered rows are a soft gray. With Mica, the sidebar and title bar keep Windows' tint around it.
- [ ] Go through Queue, Pending, Watched, History, Presets, Rules and Settings: nothing is hard to read or lost against its background (dropdown lists, menus, the add bar, the selected segment, the undo toast).
- Seen in the browser preview without Mica.

**54. Watched channels, playlists and accounts** · 0.1.79
- [ ] Paste `https://www.youtube.com/@jawed` → the list has Watch on the left. Watch → What Counts as New Now (Nothing yet, Its latest [30] videos, Everything listed here), Save In with Choose Folder…, Videos Start As → Watch → the card closes and Watched opens, with jawed's card and its new items (the latest ones you asked for, minus what you downloaded before). The sidebar counts them.
- [ ] Untick one, Download Selected → the ticked ones are in the queue, saved in the source's Save In folder if it has one (whatever the rules say), and the unticked one is under Skipped Before, where Download queues it.
- [ ] Check Now → "Checking…", then "Checked just now · Nothing new". On an account you're signed out of, the card shows the sign-in message with its fixes.
- [ ] Edit changes Save In and Videos Start As; ⋯ → Stop Watching asks, then removes the card. Restart → the sources, their new and skipped items are still there.
- [ ] Ctrl+3 opens Watched, Ctrl+7 Settings.
- Seen in the browser preview with stand-in data.

**28. Placeholders and patterns** · 0.1.19, 0.1.44
- [ ] Rules → Add Rule: the buttons under the folder field put {site}, {uploader}, {title}, {id}, {kind}, {preset}, {year}, {month}, {date} and {added} at the cursor, and hovering one says what it becomes.
- [ ] A folder like `{site}\{year}\{month}` shows filled in on the preview card.
- [ ] A condition with Matches and a pattern that captures, such as title matches `^(\w+)`, with `{1}` in the folder → the captured text is in the folder. A pattern that isn't valid, such as `(`, is caught on Save with what's wrong.

**53. Test a link against the rules** · 0.1.76
- [ ] Rules → paste a YouTube link in Test a Link → Test: after "Reading the link…", its title, then Saves In (the folder and "· Rule N" or "· Download Folder"), Starts As (the preset with its rule, or "· The default preset") and Copied Links (Download right away or Wait in Pending with the rule, else "As Settings → Adding Links says"). Nothing is added to the queue.
- [ ] Clicking a "Rule N" scrolls to that rule. Editing and saving a rule changes the answers without testing again.
- [ ] An Instagram post shows "No preset" under Starts As; a channel link says to test one of its videos instead; a broken link shows why, as the preview card would.
- Seen in the browser preview with stand-in data.

**29. Rules that pick the preset** · 0.1.43
- [ ] Add "When the site is SoundCloud → Start As Audio MP3" without a folder. A SoundCloud track's preview preselects Audio MP3 with "Rule N" beside it, which opens Rules; picking another preset on the card wins.
- [ ] The same link in Pending preselects Audio MP3 with "Rule N", and Start All uses it.
- [ ] Download Right Away On in a rule makes copied links it matches start downloading even with Settings' Download Right Away off (and Off the other way round).
- [ ] A rule that checks the kind or the preset can't save a Start As. Deleting the preset a rule starts as → the rule says so, and the default is used.

**30. {account} and {playlist}** · 0.1.63
- [ ] A rule "When the site is Instagram" with the folder `{account}` → natgeo's posts go into a `natgeo` folder (the handle), where `{uploader}` gives National Geographic.
- [ ] "Uploader is natgeo" matches National Geographic's Instagram posts.
- [ ] A rule with `D:\Channels\{playlist}`, then videos picked from a channel or playlist → they go into a folder named after it; a link pasted on its own goes into No Playlist.

## History

**31. A quick history** · 0.1.50
- [ ] History opens at once and loads more as you scroll; a search runs as soon as you pause typing; a row you expanded is still expanded after scrolling away and back.
- Measured with 5,000 test rows (opening went from about 1.4 s to under 40 ms). The portable copy's history is small, so the real test is your own history after the next update.

**32. Filters and acting on several** · 0.1.54
- [ ] Filter by kind (Videos, Audio, Posts, Shared Files), by date (Today, 7 days, 30 days, the year, or between two dates) and Only Missing Files (rename a downloaded file to see one).
- [ ] Tick rows, or every row the search and filters show with the box above the list. Forget Selected asks once and keeps the files; Show Selected in Folder opens each folder once with its files selected, asking first above 5 folders. Changing a filter clears the ticks.
- [ ] Both Forget dialogs read well, and neither mentions yt-dlp's archive any more.

**44. Only Existing Files** · 0.1.66
- [ ] History: the checkbox is now a Files list. Rename a downloaded file → Only Missing Files shows it, Only Existing Files hides it and shows the rest; All Files shows both. The count follows, and Select All ticks only what the list shows.

**50. Moving downloaded files** · 0.1.72
- [ ] History → a video's ⋯ → Move Files… → pick a folder → its file moves there, "Moved 1 file." shows, and Open File and Show in Folder open it in its new place.
- [ ] Tick several (a post and a shared folder among them) → Move Selected… → each post's files and each shared folder's subfolders arrive together; a name already in the folder gets " (2)". Rows whose file was missing count as "missing already".
- [ ] Move to another drive (D: to C:, say) → the files are copied, then gone from where they were.
- [ ] A file open in another app (a video playing) → "1 file couldn't be moved." with the file and Windows' reason; the others still move.

**56. Items, not videos** · 0.1.82
- [ ] History's count reads "1 item" or "15 items"; Forget on a row asks "Forget This Item?", Forget Selected "Forget 3 Items?", and Move Selected… "Move the Files of 3 Items To".
- [ ] A link already in the queue says "This is already in the queue." on the preview card. Labels that really mean videos (Videos Start As, the Videos kind) are unchanged.

**58. Presets save as you edit** · 0.1.84
- [ ] Presets → change a preset's quality or rename it → "Saving…" then "Saved." under the fields, with no Save or Discard button; the list's summary follows, and the change is still there after restarting Tsundoku. Not tried in the real app yet.
- [ ] Type in File Name, then click another preset at once → the edit is kept (open the first preset again to see it).
- [ ] Empty the name → "Give the preset a name." shows; click another preset → "Discard Changes?" asks, and Discard leaves the name as it was saved.
- [ ] Type a refused extra argument (`-o x`) → only the field's own message shows, not a second one under the fields.
- [ ] Rename a preset, then Duplicate it at once → the copy is named after the new name with " (Copy)".

**33. Thumbnails that don't go blank** · 0.1.60
- [ ] History rows have pictures, older downloads included, which fill in within a minute or so; the data folder has a `thumbnails` folder of small JPEGs.
- [ ] A day or more later, Instagram rows still have their pictures after Instagram's own links expire.
- [ ] Forgetting a row deletes its thumbnail.

## Running in the Background

**34. Tray and Start with Windows** · 0.1.17
- [ ] Settings → Running in the Background → Keep Running in the Tray: closing the window leaves Tsundoku in the tray, still downloading. Clicking the icon opens the window; right-clicking gives Open Tsundoku, Watch Clipboard, Start All Pending (with how many), Pause All Downloads and Quit Tsundoku, which asks first while downloads run.
- [ ] Start with Windows → Task Manager's Startup apps lists "Tsundoku (Portable)"; signing out and in starts it in the tray without its window. Turn it off afterwards if you don't want the portable copy starting.

**35. Notifications** · 0.1.17, 0.1.45
- [ ] With the window hidden or behind Zen: a copied link says it was added to Pending (or started, with Download Right Away); the shortcut and the bookmark say what they added; Start All Pending in the tray says how many started. Clicking one opens Tsundoku.
- [ ] A failing download (the `Z:\` rule) → "Couldn't Download" with its title; several failing together → "Couldn't Download 3 Links".
- [ ] Once every download started since the queue was idle has ended → "All 3 downloads finished", or "2 of 3 downloads finished · 1 failed". Cancelled ones don't count.
- [ ] Settings → Adding Links: Show Notifications, Notify When a Download Fails and Notify When Downloads Finish each turn theirs off.

**36. Progress on the taskbar and in the tray** · 0.1.39
- [ ] Queue Big Buck Bunny and the short video → the taskbar button fills with their progress, moving across until sizes are known; yellow after Pause All; red once one failed; gone once all have ended.
- [ ] Hovering the tray icon sums it up ("2 downloading · 45%"), and reads Tsundoku with nothing going on.

## Settings and Upkeep

**62. Settings' sections in the sidebar** · 0.1.88
- [ ] Open Settings: it sits right under Rules, and its sections are listed under it (In the Background stands for Running in the Background). The pills at the top of Settings are gone. Other views don't show the list.
- [ ] Click About, then Tools, then Downloads: Settings scrolls to each and the sidebar marks it. The window itself never moves: the add bar and the sidebar stay put, with no empty band at the bottom (it did before, worst with About).
- [ ] Scroll Settings with the wheel: the mark follows the section at the top.
- [ ] Make the window as short as it goes: the sidebar scrolls to reach About.
- [ ] Rules → Test a Link → click the deciding rule's number: only the rules list scrolls.
- [ ] Empty queue → Set Up beside the clipboard shortcut still opens Settings on that list, focused.
- Seen in the browser preview (the window never scrolled there, before or after); not tried in the real app.


**37. Settings → Pacing** · 0.1.27, 0.1.77
- [ ] The card is called Pacing: Instagram's Pause Between Files (5 to 10 seconds for a new copy; a range you saved stays) and the Hourly Limit, then X, Pixiv and Tumblr with empty fields (None).
- [ ] Give X 3 to 6 seconds, then download an X post with several images → its log shows gallery-dl sleeping between files, and a second X post waits until the first is done while a YouTube video starts. Empty both fields → X posts download at once again.
- [ ] Paste several X links with the pause set → Pending reads them a few seconds apart.

**38. Back up and restore** · 0.1.37
- [ ] Settings → About → Back Up… saves `Tsundoku Backup 2026-11-07.zip` where you choose.
- [ ] Rename a preset, then Restore… that zip → it asks first, restarts, and the preset has its old name. The data folder's `backups` folder holds a "(Before Restore)" zip; the tools and Start with Windows are as they were.
- Do this in the portable copy, so your real data isn't involved.

**39. gallery-dl keeps itself up to date** · 0.1.47
- [ ] Settings → Tools: gallery-dl has Update Now, and yt-dlp and gallery-dl both say they update once a day.
- [ ] Update Now while a post downloads → "Waiting for the downloads using it to finish…", then it updates once the download is done.

## Portable Copy and First Start

**43. Update All** · 0.1.65
- [ ] Settings → Tools → Update All: each tool's card shows its progress in turn, and the cards' own buttons wait meanwhile. Then it says "Every tool is up to date." or which tools it updated.
- [ ] With one tool set to your own copy (Choose File…), Update All leaves it alone. Offline, each tool says why it couldn't update, and the rest still go through.
- [ ] After Check for Updates on ffmpeg found an update, Update All installs it and the card's Update button goes back to Check for Updates.

**40. Portable copy and the first-run setup** · 0.1.20, 0.1.22, 0.1.24, 0.1.25, 0.1.26
- [ ] Run `Scripts\Build-Portable.cmd` while the portable copy runs → it quits that copy (never the installed one or a dev build), replaces it and starts it again.
- [ ] For a first start from nothing, quit Tsundoku, run `Scripts\Build-Portable.cmd D:\Dev\tsundoku\portable-fresh` and start the `Tsundoku.exe` it makes there. The first screen asks where the tools, their caches and downloads go, with caches and downloads unpicked and Continue waiting for them; Tools offers In AppData, picked when the installed copy's tools are there, which are then used without downloading. Back from the tools screen keeps the picks; the tools screen includes gallery-dl and wraps long paths; Continue Without shows if only Deno or gallery-dl fails. Delete the folder afterwards.

## Docs

**41. README** · 25d40db and later
- [ ] README.md describes the app as it is at 0.1.63. It's only local until main is pushed.

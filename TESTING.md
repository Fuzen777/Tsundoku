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

**82. Title bar with the tabs** · 0.2.38 · Claude tried, in a test copy without your mouse: double-click on the tabs' empty space and on the app's name maximizes and restores (on a tab it only opens the tab), Maximize maximizes and restores with its pressed look, Close quits, the buttons go grey while the window is behind another, and the window's edges and the maximize button answer Windows as resize borders and a maximize button
- [ ] Rest the pointer on Maximize → Windows 11's Snap Layouts open under it after a moment, as on other apps; picking a layout snaps the window there. Maximize turns gray under the pointer, Minimize too; Close turns red.
- [ ] Drag the window by an empty spot of the tabs' row, or by the app's name → it moves; dragged to the top of the screen it maximizes, to a side it snaps half. Dragging a maximized window by the same spots restores it under the pointer. Dragging a tab doesn't move the window.
- [ ] Resize from each edge and corner, the top one included (also over the buttons' top few pixels). Mica, the shadow and the rounded corners are still there.
- [ ] Maximized, the buttons sit in the screen's top right corner: throwing the pointer to the corner and clicking closes Tsundoku; Restore Down restores.
- [ ] Move the window to a screen with another scaling (100% and 150%) → Snap Layouts still open over Maximize, not beside it.

**48. Accent color** · 0.1.70
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
- [ ] Right-click in a search field, or the add bar's field (Type a Link) → the usual Cut, Copy, Paste menu. Select a title's text and right-click it → Copy.

**4. What's New** · 0.1.51
- [ ] The first start after rebuilding the portable copy shows a What's New card under the add bar with the changes since the version that copy last ran (only 0.1.63's if it last ran before 0.1.51, which didn't record it). Its × closes it, and it doesn't come back at the next start.
- Can't be tested yet: the update banner's What's New link, which needs a published release that the app can fetch.

## Adding Links

**77. Paste Link** · 0.2.5 · *one open decision*
- [ ] Copy a link in your browser and click Paste Link (at the top of the sidebar since 0.2.12) → its preview opens, with nothing else to click. Claude tried the button, its notes, Type a Link, Ctrl+L and Esc in a test copy, but with a stand-in for the clipboard, so that what you had copied wasn't overwritten: only a real copy is left to try.
- [ ] If you use a Clipboard Shortcut: copy a sentence without a link and press it → "Nothing to Add". Copy two links with words around them → both are in Pending, and none of the words.

**5. Pending tab** · 0.1.14
- [ ] Copy three or more links at once (one per line) and click Paste Link → they go to Pending instead of the preview card. Each row fills in one at a time (thumbnail, title, site); one downloaded before is marked, with Download Again. The sidebar count matches.
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
- [ ] Ctrl+L → the add bar's field opens, its text selected. Ctrl+1 to Ctrl+6 → Queue, Pending, History, Presets, Rules, Settings.
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

**71. Click'n'Load buttons (FileCrypt)** · 0.1.129
- [ ] Quit JDownloader first: it listens on the same port. Then turn on Settings → Adding Links → Take Links from Click'n'Load Buttons → no red line under it. (With JDownloader running, a red line says another app is listening on port 9666, with Try Again: close JDownloader, then Try Again → the line goes.)
- [ ] In Zen, open a FileCrypt folder, pass its captcha, then click its Click'n'Load button → FileCrypt opens a small window of its own. If Zen asks whether the page may reach devices on your local network, allow it.
- [ ] With Tsundoku's window behind Zen → a notification "FileCrypt Sent N Links" ("They're being read in Pending."). Pending shows "Checking N links pages sent…", then the folder's links, read and waiting to be started, even with Download Right Away on. Links Tsundoku can't download leave without a word.
- If nothing comes, tell Claude what FileCrypt's small window showed (its address and text, or a blank page), whether Zen asked anything and what you answered, and whether Settings' line is red.
- Claude tried a test page of its own in Chrome 157, Edge 157 and Firefox 159 (a form into a hidden frame, a fetch, a form into a new window), the port clash with your running JDownloader and with a stand-in, and pages that aren't FileCrypt (refused). FileCrypt's own button wasn't: its small window (plain http) makes the post, and whether Zen lets that window reach this PC is what this checks. In Chrome, a plain http page could only get through by posting into a window of its own.

## Previews and Picking

**84. Picking in Pending** · 0.2.43 · Claude tried, in a test copy with a copy of your data at 1200×720: a channel pasted on Downloads went to Needs Attention with the toast, Open in Pending opened it in full page, Esc put it in Pending at 60% of the window, Collapse and Expand, going to History and back kept the ticks, Pick in Full Page off opened it in Pending, a channel pasted on Pending opened at once, and a SoundCloud set (found a playlist only once read) went the same way
- [ ] On your screen, paste an Instagram or X account on Downloads → "Account added to Pending" with Open in Pending; open it → the list over the whole window, the site's icon before its name.
- [ ] Esc → the list sits above Pending, taking a bit more than half the window, Pending's links reachable under it. Collapse → one line with the account's picture, the site's icon, its name and how many are ticked; hovering it shows its link and details.
- [ ] Load All on a big account, then Collapse and go to another view while it loads → coming back, it's still loading or done, nothing lost.
- [ ] (0.2.45) Two Pixiv artists in Needs Attention: open the first, untick a few, Load More, then open the second → the first is back in Needs Attention with its picture, name and how many are ticked; open it again → it shows at once with the same works and ticks. Claude tried this with two YouTube channels.
- [ ] (0.2.46) Pending's list of a profile you watch: untick a few new items, Download → the Watched card shows them under Skipped Before; open the list again → they start under Discarded, marked Skipped. Claude tried the Skipped part on an unwatched YouTube channel, with nothing actually downloaded.

**14. Pick from a channel, playlist or account** · 0.1.57
- [ ] Paste a YouTube channel (such as `https://www.youtube.com/@NASA`, any tab) → "Listing the channel's newest videos…", then its newest 50 videos, newest first, each with a box, thumbnail, title, rough date and length. New ones are ticked; downloaded ones are greyed, unticked and marked Downloaded (hovering says when); queued ones say In Queue.
- [ ] A playlist is listed whole, in its own order. A `watch?v=…&list=…` link is still that one video.
- [ ] An Instagram account (`https://www.instagram.com/natgeo/`) lists its newest 50 posts and reels; it can take a minute.
- [ ] The box at the top ticks all or none and counts them. Download Selected queues each ticked item as its own download, with a rule's preset or the one in Videos Start As.
- [ ] A channel link pasted with others waits in Pending, under Needs Attention with Pick What to Download. (X accounts are listed since 0.1.134, which Claude tried in the real app.)

**15. Pick which files of a post to download** · 0.1.55
- [ ] Paste the Instagram carousel → a strip of 6 small pictures with their numbers and a play mark on videos, all ticked. Untick 2 to 5 and Download → the queue row says "2 of 6 files", and only files 1 and 6 arrive.
- [ ] Paste it again → 2 to 5 are marked Not Picked (with a ring) and ticked, 1 and 6 Downloaded; Download Again gets just the rest.
- [ ] In Pending, a post's row has Pick Files, which opens the same strip. Untick some, restart: the ticks are kept, and Start downloads only those.
- [ ] Pick files of an X post with several images.
- Claude tried the first check in the real app: exactly files 1 and 6 arrived. Pending's strip and X weren't tried.
- Known: Start All skips a post downloaded before, even partly; use its row's Download Again.

**73. Pick the files of a Gofile or Pixeldrain folder** · 0.1.133
- [ ] Paste a Gofile folder or a Pixeldrain list of yours holding several files, some neither pictures nor videos (a .zip, a .pdf, a .txt) → instead of blank tiles, a numbered list of their names with their sizes, everything ticked, and a Filter Files field over ten files. Untick some and Download → only those land, and the queue says "2 of 5 files".
- [ ] Paste it again → the files downloaded say Downloaded, the others Not Picked and are the ones ticked. Tick another → the sentence over the list only says how many weren't picked.
- Claude found no public folder of several such files to try (every one linked from GitHub READMEs or gallery-dl's own tests was gone, or held only videos). The list was checked in the browser preview with stand-ins, in light and dark; in the real app, a public Gofile folder of one .zip was read again (it failed before this version), a Pixeldrain folder of two videos kept its strip, and five quick ticks in a row on its strip in Pending all stayed, shown and saved.

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

**69. OneDrive** · 0.1.100, 0.1.105
- [ ] Paste `https://1drv.ms/f/c/2d90e71fb9eb254f/EnMm8c2mP_FPv6lwt1jy01YB6bQhoPQ25vtzAhycYisERw?e=DiI2Ab` → "3 files, 2.6 GB" on the card (no need to download it), then `https://1drv.ms/f/s!AhIXJn_J-blW00SqrairNetmeVu4` → a message that this folder has folders inside it, which Tsundoku can't read yet.
- Claude read these and two dozen other public links through Tsundoku's code (short links old and new, the onedrive.live.com address they lead to, and the redir, cid, embed and download forms), and downloaded a shared file and a folder's file with their SHA-1 checked. What's left is how the card shows them.

**67. MediaFire's check** · 0.1.93, 0.1.103, 0.1.123
- Since 0.1.123 MediaFire is asked as Tsundoku rather than as Firefox, and its pages then came without Cloudflare's check (2026-11-12), so the row below may simply download; the check steps are for when Cloudflare still asks.
- [ ] Queue: add `https://www.mediafire.com/file/orecif6losbo2uw/file` (890 MB). If its row says "MediaFire wants to check you're a person…", click Open Check → a "MediaFire Check" window shows MediaFire's page. If Cloudflare asks to verify you're human, do it, then click Download → the window closes, nothing lands in your Downloads folder, and the download continues in Tsundoku with progress, then is checked. (If the row just downloads, MediaFire didn't ask Tsundoku for the check this time; try another file of yours that does.)
- [ ] Remove that download, add the link again and close Tsundoku's window to the tray at once → when it reaches the check, a notification says "A Download Needs You".
- Claude ran this in a dev build with that link: the download waited with the message, Open Check showed MediaFire's page (Cloudflare didn't challenge the window on this PC, so its check page itself wasn't seen), and a mouse click on Download sent through DevTools was caught; the window closed and Tsundoku downloaded and checked the file. A folder with several files behind the check, a pasted download link and the notification were only unit-tested.

**76. Load All on a big X account** · 0.2.4
- [ ] Open a big X account (thousands of photos and videos), press Load All and leave it → it lists about 1,000 posts, says "Paused by X, resuming at…" with the time left, and goes on by itself after each wait, to the end ("Account · 3,412 posts", no Load buttons left). Tick and download a few posts during a wait.
- Claude tried it in a test copy with x.com/NASA: 1,210 posts in 25 seconds, the pause with X's own time, the listing going on by itself on the second at 4:46 AM up to 2,070 posts, Stop during a wait, and the account opening with everything listed before, during the wait and after a restart (a YouTube channel listed whole too). It wasn't left running to the end, to spare your X allowance: each stretch uses up X's list for 15 minutes, for X's own Media tab in your browser too.

**81. Sign in to Fanbox from a link that needs it** · 0.2.30 · Claude saw the card ask and its button open the window, signed out; the sign-in itself needs your account
- [ ] In a copy not signed in to Fanbox, paste a post of a creator you support. Its card (or its row in Pending, now under Needs an Account) says Fanbox needs Tsundoku signed in, with Sign In to Fanbox. Click it: the sign-in window opens at once, without Settings. Continue as your pixiv account: the window closes and the post is read again, showing its files. The same goes for Sign In to Pixiv on a Pixiv link.

**83. A Patreon post's links go to Pending** · 0.2.42 · Claude read 10 posts in a test copy and downloaded pictures, attached files, a video and a podcast, but none of the free posts linked anywhere
- [ ] Paste a free Patreon post whose text links elsewhere (a MEGA or Google Drive folder, a YouTube video, another Patreon post) and download it → once it's done, those links are in Pending, read but not started, each once. A link to the creator's own page, a tier's checkout or the post itself isn't there.
- [ ] Paste a free post of only text and links → its card says it has no images or files of its own and its links went to Pending, and they're there at once.

**80. A Pixiv novel's book in your reader** · 0.2.29 · Claude checked the book's files, not how a reader shows it
- [ ] Signed in to Pixiv, paste a novel's link (`https://www.pixiv.net/novel/show.php?id=…`) and download it. Open the `.epub` it saves in the reader you use: the cover first, then the novel's pages in order, its chapters listed in the reader's table of contents, furigana shown above their words, and links that open.

## Pending and the Queue

**72. Unpacking archives** · 0.1.130
- [ ] Settings → Passwords' Your Passwords empty, Save Past Passwords on (0.2.6). Download a zip, 7z or rar with a password from one of your shares → the row fails saying the archive has a password, with a Password field. Type it, Unlock → it unpacks without downloading again (Show Log has no new download line), Settings → Passwords says "1 past password is kept." and Your Passwords is still empty. Download another archive with the same password → it unpacks by itself.
- Claude tried the 0.2.6 part in a test copy with zips made here (AES, standing in for small public Internet Archive files): asked on the failed row, a wrong one asking again, the right one unpacking and kept, the next archive unpacked by itself, Forget Past Passwords, then the same archive opened from Your Passwords. A real share of yours with a password, and a rar, weren't.
- [ ] An archive split over two of your links (`.part1.rar` and `.part2.rar`, or `.7z.001` and `.7z.002`): paste the first part's link alone → it finishes with the part left as it is, and Show Log says it's "left as it is until" the next part is in its folder. Paste the other part's link → that download unpacks the whole archive, and both parts go to the Recycle Bin.
- Claude tried, in a test copy: zip and 7z archives made with 7-Zip (plain, ZipCrypto and AES with a password from Settings, 7z with hidden names, 1 MB pieces, a missing piece), identical to their sources; from archive.org, a 3 MB zip (Recycle Bin), a 7z (Keep the Archive) and a 141 MB zip stopped while Unpacking, then resumed (only unpacked again, into "… (3)"), each identical to 7-Zip's own unpacking. No rar archive was made, since WinRAR isn't on this PC; the tests' real rar archives pass.

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
- [ ] Downloads: a finished download's × (Remove from Downloads) removes it with "Removed “…” from Downloads" and Undo, which puts it back in its place. History still has it.
- [ ] Pending grouped by Folder: rows show "Rule 3" (or what else picked the folder) without the path; the box on a heading ticks every link in that group, part-ticked when only some are.
- [ ] Fold a group in Pending, go to Downloads and back → it's still folded. A restart unfolds it.
- Seen in the browser preview with stand-in data.

**23. Copy Details** · 0.1.41, 0.1.46
- [ ] On a failed row (the `Z:\` rule above), hovering Copy Details says its name, and it says Copied after a click. Pasted in Notepad: Tsundoku's version, each tool's version, Windows' version and build, the link, the error and the log's last lines, with your user name blanked out.

**79. Downloads grouped by rule, on your own downloads** · 0.2.9
- [ ] Open Downloads with your own downloads, grouped by Rule. Those added before 0.2.9 sit under the rule that still gives them the folder they have, else under "Unknown Rule · <their folder>": none of them under Games unless Games gave them their folder.
- [ ] Add a download, then add or move a rule that would now match it → it stays under the rule it was added with.
- Tried by Claude in a test copy with its own downloads and rules, not on yours.

## Waiting and Retrying

**70. 1fichier** · 0.1.114
- [ ] Paste a 1fichier file link and Download. While 1fichier makes the connection wait, the row says "1fichier's download limit is used up. Continuing at …" with its time; then it says Counting Down with the seconds left for about a minute, then downloads. Claude ran this whole path in a test copy on 2026-11-12 (the wait held it until its time, the countdown ran, the file arrived intact), but didn't see how the row looks.

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
- [ ] Watch for a Google Drive or Dropbox download limit → it waits 30 minutes, then an hour, two and four, and never fails for it. (MEGA's waits as long as MEGA says since 0.2.8: see 78.)
- Not tried against a real dropped connection or host limit.

**78. MEGA's own reset time, and retries that count progress** · 0.2.8 · *watch for it*
- [ ] When a MEGA download uses up MEGA's transfer quota (a few GB within hours, without an account), its row says "MEGA's download limit is used up. Continuing at …" with MEGA's own time (often hours away, not 30 minutes), counts down, and at that time the download goes on by itself from where it stopped, not from the start.
- [ ] A download whose connection drops again and again while more of it keeps arriving says "(1 of 3)" each time and doesn't fail; one that drops before anything more arrived counts on to "(2 of 3)".
- Claude checked with tests: MEGA's answers that give a time (its API's `tl`, and `X-MEGA-Time-Left` on its download server's 509), a part file continued with a Range request (MEGA's servers answered one live with 206), and the retries starting over in the queue. Not tried against MEGA's real quota, which can't be reached on purpose without using up the connection's.

## Presets

**46. Adding, moving and deleting presets** · 0.1.68
- [ ] Presets → Add Preset → "New Preset" appears at the end of the list, set up as Best Video, and opens. Rename it.
- [ ] Move Up and Move Down move the open preset; Move Up is greyed out right under the default, Move Down at the end. The default has neither, and no Delete.
- [ ] Set as Default on another preset → it jumps to the top with the Default badge, and the previous default can now be moved and deleted.
- [ ] Delete a built-in such as Best Audio (after making another the default if it was) → a ↺ button appears beside Add Preset; its menu's Put Back Best Audio adds it again as it ships.
- [ ] The preview card's, Pending's and the rules' preset lists follow the new order.

## Rules

**68. File names per rule** · 0.1.96, 0.1.97, 0.1.99
- [ ] Rules → Edit a rule: File Name under Save In looks like Settings → Posts of Images' (the same buttons, {numberIfMany} included, and an example), shows "Using Default" while empty, and the example then shows Settings' name. With a Matches condition such as title matches `(RJ\d+)`, a {1} button follows. A name with `:` or an unknown placeholder is refused on Save.
- [ ] A MEGA or MediaFire link to one file named like "SomefileRJ123456.rar", with that rule and the name `{1}` → it's saved as "RJ123456.rar". Not tried: the portable copy was running, so the app couldn't be started.
- [ ] A rule for Instagram named `{account} {1}` → a post's files get the post's ID and number added. Not tried: it needs a signed-in browser.
- [ ] Settings → Posts of Images with `{account} {post} {numberIfMany}`: a carousel's files are numbered, a single photo isn't. Not tried.

**66. AND and OR per condition** · 0.1.92, 0.1.94 (replaces 63)
- [ ] Rules → Edit a rule with three conditions: site is YouTube, site is Vimeo, title contains zoo. Each condition after the first starts with its own AND button, on its row, lined up with the first condition's fields (in capitals, with a tooltip). Click the first one only: it says OR, the second stays AND. Save → the sentence reads "When the site is YouTube OR the site is Vimeo AND the title contains “zoo”".
- [ ] Test a Link with "Me at the zoo" (`jNQXAC9IVRw`) → that rule decides; with a YouTube video without "zoo" in its title → it doesn't.
- [ ] Changing a condition's field keeps its AND/OR. A rule saved as Or in 0.1.89 or 0.1.90 shows OR in every gap.
- Seen in the browser preview (each button switches on its own); not tried in the app.

**65. Rule names** · 0.1.91
- [ ] Rules → Edit a rule: a Name field under its heading, with "Rule N" as its placeholder. Type "Nat Geo", Save → the row shows "Nat Geo" above its sentence.
- [ ] Paste a link the rule takes: the preview card's folder line says "· Nat Geo" instead of "· Rule N"; the same in Pending, and beside the preset when the rule picks it. Test a Link and the Queue's or Pending's Group By → Rule say "Nat Geo" too. Clicking it still opens Rules.
- [ ] Clear the name and save → "Rule N" everywhere again. Deleting a named rule asks "Delete Nat Geo?".
- Seen in the browser preview (the field and the row); not tried in the app.

**64. Match case** · 0.1.90
- [ ] Rules → Edit a rule: each condition on the site, uploader, title or link has an Aa button before ×, with "Match Case" as its tooltip; the kind and the preset have none. Click it: it lights up and its tooltip says "Ignore Case". Save → the sentence ends with "(matching case)".
- [ ] A rule "title contains Zoo" with Match Case on: Test a Link with "Me at the zoo" (`jNQXAC9IVRw`) → the rule doesn't decide; turn Match Case off → it does. Same with a Matches pattern.
- [ ] Conditions saved before this version still ignore case.
- Seen in the browser preview (the button switches); not tried in the app.

**61. Drag rules into place** · 0.1.87
- [ ] Rules → hover a rule's grip (left of its number): the cursor is a hand and "Drag to Move" shows. Drag it up or down: the whole row follows the pointer and a line shows where it will go. Dropping saves the new order at once, numbers follow, and the rule that decides changes accordingly.
- [ ] Dragging above Everything Else puts the rule just under it; Everything Else has no grip. Dropping a rule where it already is changes nothing. While a rule is being edited, the grips are dimmed and don't drag.
- [ ] Dragging a link from a browser onto the Rules view still opens its preview, as anywhere else.
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
- [ ] Tick rows, or every row the search and filters show with the box above the list. Forget Selected asks once and keeps the files; Show Selected in Folder opens each folder once with its ticked files selected (see 75), asking first above 5 folders. Changing a filter clears the ticks.
- [ ] Both Forget dialogs read well, and neither mentions yt-dlp's archive any more.

**74. Show in Folder comes to the front** · 0.1.143
- [ ] Click Show in Folder (on a Queue row, in a picture's right-click menu, in History, on a post's file) or one of Settings' Open Folder buttons → a File Explorer window opens on the folder in front of Tsundoku, with the file selected for Show in Folder, and no "Application not found" message.
- Tried in a test copy driven over DevTools: each opened File Explorer on the right folder with the file selected, with no message, and Tsundoku answered within 10 ms while Windows opened it (0.1.142 froze until the message was closed). A game had the focus the whole time, so whether the window comes in front when you click in Tsundoku yourself couldn't be seen.

**75. Folders in your folder app** · 0.1.144
- [ ] Once Windows opens folders properly again (the leftover "Open in Directory Opus" setting removed): Show in Folder and the Open Folder buttons open the folder as a double-click on a folder would, and History → tick a post of several files (or two videos in one folder) → Show Selected in Folder → every one of them is selected, not only the first.
- Tried in a test copy with the leftover setting still there: every Show in Folder and Open Folder opened File Explorer on the right folder with the file selected, with no message, Tsundoku answering within 11 ms, and its output saying each time that the folder app is gone. With Windows' own folder setting (for a test process only, the registry unchanged), the same code selected both files of a folder and opened another, with no message. Not tried in the app itself with a working folder setting, which this PC doesn't have yet.

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

**62. Settings' sections in the sidebar** · 0.1.88, 0.1.95
- [ ] Open Settings and click About, then Tools, then Downloads in the sidebar: only Settings scrolls. The window itself never moves: the add bar and the sidebar stay put, with no empty band at the bottom (it did before, worst with About).
- Checked in the browser preview: Settings sits at the bottom and moves up under Rules only while open, with its sections under it; the pills are gone; the mark follows the wheel; a short window scrolls the sidebar; Test a Link's rule number and the empty queue's Set Up still work. The window never scrolled in the browser, even before the fix, so only the real app can show it.


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
- [ ] For a first start from nothing, quit Tsundoku, run `Scripts\Build-Portable.cmd D:\Dev\tsundoku\portable-fresh` and start the `Tsundoku.exe` it makes there. The first screen asks where the tools, their caches and downloads go, with caches and downloads unpicked and Continue waiting for them; Tools offers In AppData, picked when the installed copy's tools are there, which are then used without downloading. Back from the tools screen keeps the picks; the tools screen includes gallery-dl and wraps long paths; Continue Without shows if only Deno or gallery-dl fails. Delete the folder afterwards.

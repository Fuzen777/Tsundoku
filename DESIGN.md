---
type: Agent Instructions
---
# Tsundoku Design Reference

## Guiding Principles

- **A Windows 11 app.** Mica backdrop, Segoe UI Variable, a sidebar and a content layer with a rounded top-left corner, as in Windows' own Settings app, sunk below the window with lighter cards on it in both themes, as in RJ Library. It follows Windows' light or dark theme, unless Settings → Appearance picks one.
- **Quiet chrome, clear state.** The app recedes; what is downloading, finished, failed or already owned is what stands out.
- **Consistency over cleverness.** Repeated patterns (buttons, rows, empty states) look and behave the same everywhere.

---

## Typography and Copy

- Segoe UI Variable Text, 14 px body. View titles are 20 px semibold (`text-xl`), section titles 14 px semibold.
- **Title Case for labels.** The full rule is in [Labels and Casing](#labels-and-casing).
- **Concise labels over sentences.** Use a sentence only where a label alone would be ambiguous.
- All text comes from `tsundoku/src/strings.ts`.

---

## Color

Every color is a token in the `@theme` block of `tsundoku/src/index.css`, written `light-dark(light, dark)`. Tailwind turns each one into utilities (`bg-surface`, `text-fg-muted`, `border-line`…). Never write a raw color in a component.

Which side of `light-dark()` shows follows the theme picked in Settings → Appearance: `data-theme` on `:root` (`src/theme.ts`) sets `color-scheme` to `light` or `dark`, or leaves it `light dark` for Auto, which follows Windows. Native controls, scrollbars and select lists follow it too. Never use `prefers-color-scheme` or Tailwind's `dark:` variant, which would follow Windows instead of the choice; a color that differs between themes is a token.

### Role Mapping

| Role | Token | Usage |
| --- | --- | --- |
| Window background | `--color-bg` | Painted only without Mica (Windows 10) |
| Content layer | `--color-surface` | The content area, panels: sunk below the window in both themes |
| Card | `--color-card` | Cards, one step above the content area |
| Interactive background | `--color-input`, `--color-input-hover` | Inputs, secondary buttons, hovered and selected rows, icon tiles |
| Borders | `--color-line`, `--color-line-strong` | Dividers and outlines; strong for emphasis |
| Text | `--color-fg`, `--color-fg-secondary`, `--color-fg-muted` | Primary text; supporting details; labels, empty states and placeholders |
| Accent | `--color-accent`, `--color-accent-hover` | Ai-iro (藍色), Japanese indigo, for marks and text: focus outlines, the selected navigation item's pill, progress bars, links, checkboxes |
| Accent fill | `--color-accent-fill`, `--color-accent-fill-hover`, `--color-on-accent` | A filled background with `--color-on-accent` text: primary buttons, the selected segment, count badges |

Settings → Appearance → Accent Color swaps the four accent tokens for another hue (`[data-accent]` in `index.css`): Indigo, the one above and the default, then Teal, Green, Amber, Rose, Purple, and Custom: any color picked, whose hue and some of its chroma are set at indigo's lightness for each shade (`oklch(from var(--accent-custom) …)`), so it reads as well as the others. Each keeps the same roles: a light shade for marks and text in dark mode, a deep one for fills under white text in both. Its picker is a row of 28 px round swatches, each filled with its own accent (the swatch carries `data-accent` itself), the selected one with a check and a secondary-colored outline 2 px off it. The last swatch, Pick a Color, shows the custom color with a pipette icon and opens Windows' color picker.
| Destructive | `--color-danger` | For text and icons: an error under a field, a destructive menu item, an error message's icon |
| Danger fill | `--color-danger-fill`, `--color-danger-fill-hover`, `--color-on-danger` | The danger button, confirming a destructive action |
| Attention | `--color-warning` | Warnings that don't block: the icon of the duplicate warning, and of Deno missing |
| Success | `--color-success` | Something finished well: the icon of a success message |

### Rules

> [!info]
> With Mica, the window itself is the background: surfaces are translucent so it shows through. `<html data-backdrop="mica">` makes the page transparent; `solid` (Windows 10) paints `--color-bg` instead.

> [!tip]
> When one color would serve two distinct roles, add a token rather than sharing one.

### Light Mode

Since 0.1.80, light mode follows the dark mode's layers:

1. Neutral grays with no tint: black or white at an opacity, and gray text.
2. The content area (`--color-surface`, black at 3.5%) is a light gray well below the window, and cards are nearly white on it (white at 85%).
3. Inputs, secondary buttons and hovered rows are a gray step (black at 4.5%, 7.5% hovered), so they show on a white card as on the gray content.
4. Lines are black at 10% (20% for `--color-line-strong`); secondary text is `#575757` and muted text `#767676`, which still reads on white.
5. Filled buttons, the selected segment and count badges are the deep accent with white text, as in dark mode.

### Dark Mode

Dark mode follows RJ Library's dark theme:

1. Neutral grays with no tint: white or black at an opacity, and gray text.
2. The content area (`--color-surface`, black at 45%) is darker than the window around it, and cards and inputs are lighter again (white at 4% and 7.5%).
3. Lines are white at 14% (24% for `--color-line-strong`).
4. Filled buttons, the selected segment and count badges are dark (`--color-accent-fill`, `--color-danger-fill`) with white text; the light `--color-accent` and `--color-danger` are for marks and text.
5. Every layer is translucent, so Mica still shows through; without it, they sit over `--color-bg` the same way.

---

## Layout

1. The app name (積ん読 beside it) and the add bar run across the top. The add bar is Paste Link (`primary`, a `ClipboardPaste` icon before its label), at the left of the content's column, then Type a Link (`TextCursorInput`, a ghost icon button), then, when a paste found no link, "There's no link on the clipboard." in secondary text for a few seconds. Type a Link swaps them for a field across the bar ("Type or Paste a Link" as its placeholder), Add (`primary`, off while the field is empty) and Close (`X`, a ghost icon button), until a link is added or the field is closed.
2. The sidebar is on the left: the views at the top, Settings at the bottom. While Settings is open, it moves up under the views and its sections are listed under it: 13 px rows 28 px high, indented to the views' names, in muted text (`--color-fg` on hover, with the `--color-input` background), the section at the top of Settings in medium `--color-fg`. The sidebar scrolls on its own when they don't fit.
3. The current view sits in the content layer (`--color-surface`, a line border on its top and left, rounded top-left corner).
4. A view is a `Page`: its title, then content that scrolls on its own. Within it, a `Section` groups related content under a 14 px semibold title, and a `Card` raises one item (`--color-card`, a line border, 8 px corners).
5. An empty list shows an `EmptyState`: a 40 px icon, a Title Case title and a one-sentence explanation. The empty Queue and Pending add the ways to add links under it (`WaysToAdd`): a `Card` at most 32 rem wide, one row per way, each a 16 px muted icon, its name, where it stands in 12 px secondary text, and one `sm` button (Paste, Choose File…, Turn On, Set Up or Change), or a check and On in the success color when there's nothing left to do. The queue lists the ways by hand first, Pending the ones that work on their own.
6. The first-run setup replaces the sidebar and the add bar: the app name, then the content layer across the whole width, holding a centered column (`max-w-2xl`) with the title, a sentence, its cards, and the buttons at the bottom right. Its first screen has a card per choice (where the tools go, their caches, the download folder), each an `IconHeading` and radio rows; its second, a card per tool, with Back while no tool is set up.
7. A tool is shown with `ToolHeading`: its icon on a 36 px tile (`--color-input`), its name, and what it does. `IconHeading` is the same for anything else, such as a first-run choice.
8. A view's page-wide buttons (such as Clear Finished) sit on its title's right, `sm` size.
9. The sidebar shows a count beside Queue (downloads not done yet), Pending (links waiting) and Watched (new items): an accent pill with the number, hidden at zero.
10. The preview card opens between the add bar and the current view when a link is pasted, typed or dropped, and closes with its × at the top right, whose hover text is Cancel while the link is read, Dismiss after a failure and Close otherwise. It's a `Card`: the thumbnail (176 px wide, 16:9, an empty `--color-input` tile when there's none), then the title (two lines at most), uploader and site, and a line of details (length, upload date, best quality), with × at the top right; then its notices as status messages (the duplicate warning with Open File and Show in Folder as icon buttons under it); for a post of several files, the strip of its files (see [Post Files](#post-files)), or for a file host's link of several files, the list of its files (see [Shared Folder Files](#shared-folder-files)); then a bottom row: where the download will be saved (a folder icon, the path in 12 px secondary text, truncated, and "· Rule 2", "· Preset's Folder" or "· Download Folder" in muted text, which opens where it's set), then, when a rule picked the preset, "Rule 3" in 12 px muted text, which opens the rules, then the preset picker and Download on the right.
11. The queue view's page-wide buttons are Pause All, Resume All, Retry Failed and Clear Finished, each shown while a download is in a state it acts on. It has a toolbar over its list: the number of downloads, then how the list is grouped and sorted (`ArrangeControls`: "Group By" in 12 px secondary text, Status as a one-button toggle styled like a `SegmentedControl` option, a `SegmentedControl` for None, Site, Folder and Rule, a select for the sort, and the direction as an icon button, as in the history). Grouped, each group has a heading (`GroupedList`), a button that folds the group (each view remembers which while the app runs): a chevron, a folder icon before a folder's path, the label in 12 px semibold secondary text and the number it holds in muted text; groups are 16 px apart, rows 8 px. Grouped by status and something else, a status's group holds the other groups under 12 px medium headings indented 16 px. Downloads picked together from one channel, playlist or account (two or more in the queue) sit under the source's heading (`QueueSourceGroup`) wherever the first of them falls in each group: a `Card` with the fold chevron as a ghost icon button on its left, the source's picture as the Watched view shows it (`ListingPicture`, small: a 56 px circle, or a 96 px thumbnail for a playlist), its name in semibold, "Channel · youtube.com" and where its downloads stand ("3 of 12 done · 2 downloading · 7 waiting") in 12 px secondary text, then on the right, as icon buttons, Pause All (`CircleStop`), Resume All (`Play`) and Retry Failed (`RefreshCw`), each `secondary` and shown while one of its downloads needs it, and × (Cancel All, which asks first, or Clear Finished once all are done) `ghost`; under them the downloads' progress together while any downloads, or waits after one is done. Between the name and the actions, while unfolded, the picker's view controls (see [Picker Views](#picker-views)); when they don't fit beside the name (at least 256 px for the chevron, picture and name), the controls and actions go to a line of their own, on the right. Clicking the picture or the name folds it too. Its downloads follow 8 px under it, indented 24 px behind a 2 px `--color-line` line that runs from under the chevron, 8 px from them: rows in List, pictures in Masonry and Gallery (`QueueTiles`). A picture there is the download's thumbnail (dimmed to 50% while it waits or is stopped; a `Film` or `Images` icon in muted text when there's none), with where it stands at the top left as a corner label (`Download` in the accent with the percentage while it downloads, `Clock` waiting, `CircleStop` stopped, `CircleAlert` in `--color-danger` failed, `Check` in `--color-success` done), a `ProgressBar` along its bottom while it downloads, a 2 px `--color-danger` ring when it failed, and under it its title (12 px, medium) and its stage and size (12 px secondary, in `--color-danger` when it failed). It has no buttons: a right-click opens its actions as a right-click menu (`ContextMenu`: the `Menu` panel at the pointer, each action after its button's 14 px icon, Cancel and Remove from Queue in `--color-danger`), and a click shows its row across the width under its line of pictures, with a ghost Hide Details (`ChevronUp`) at the end of its actions, the picture getting a 2 px accent ring and staying where it was on screen. A double-click on a finished download, as a row or a picture, opens its file, as Open File does. A queue row is a `Card`: the thumbnail (96 px), then the title, a line of details (preset, stage, size, speed, time left), the progress bar while it runs and its error if it failed; its actions on the right as icon buttons, the main one (Stop, Resume, Retry, Open File) `secondary` and the rest (Show in Folder, Show Log, Copy Details on a failed one, Cancel, or Remove from Queue on a finished one, both ×) `ghost`, then on a waiting row a `Menu` with Download Next. A waiting one held back (its site asked to slow down, or the hourly limit is reached) says why under its details, as a `neutral` status message with the time left as its detail ("Instagram asked to slow down. Continuing at 2:32 PM.", "11:56 left"), counting down. A row that waits to try again, or for its site, has Try Now (`ghost`) after Stop. A row that waits for a site's check says so ("MediaFire wants to check you're a person before this download…") with Open Check (`sm`, `secondary`) under the message, and no Try Now or countdown. Open Check shows the check window: a separate window of 1000×760 px titled "MediaFire Check" (", File 2 of 3" for a folder's files) holding the site's own page and nothing of Tsundoku's.
12. The pending view puts the Watch Clipboard checkbox and Start All on its title's right (`sm`, `primary`), or Remove Selected (`ghost`) and Start Selected (`primary`) while links are ticked. While copied links or links from dropped files are checked, a line with a spinner counts each above the list. Under the title, a toolbar: the select-all checkbox with the number of links, how the list is grouped and sorted (as in the queue view), and on the right "Videos Start As" with the preset picker, which goes to a second line at the window's minimum width. Grouped, the links sit under headings as in the queue view, each with a checkbox before its chevron that ticks the whole group (part-ticked when some are); grouped by Folder, a row's destination line drops the folder icon and path, keeping what picked it ("Rule 3"). Then one `Card` per link, like a queue row: its checkbox, the thumbnail (96 px), the title (or the link until it's read), "uploader · site · length · date · quality" (or "Waiting to be read" / a spinner and "Reading…", with "Starts once it's read"), then, once read, where it will be saved, as on the preview card, with Change Folder… (`FolderPen`) and Apply a Rule… (`FolderTree`, a `Menu` of every rule by its name or number, "(Off)" after one that's off) as icon buttons after it, each followed by an `Undo2` icon button while something is picked (Use the Usual Folder, Use the Matching Rules), and the rule picked named in muted text ("· Picked: Nat Geo", in place of the folder's source when it gives the folder), then its notices (already downloaded, already in the queue, why it couldn't be read with its fixes); its actions on the right: Pick Files (an icon button) for a post of several files or a shared folder, which opens the strip of its files, or the list of a folder's files, under its notices, then the preset picker for a video, with "Rule 3" before it as on the preview card, then icon buttons: Start (Download Again once downloaded) or Try Again `secondary`, and Remove `ghost`. A link whose site asked to slow down says so under its status the same way. Channels, playlists and accounts sit above the rest under a Needs Attention heading of their own, whatever the grouping (the rest then under Other Links while the list isn't grouped): the link as the title, a 12 px secondary line with a `ListChecks` icon saying what it is and to pick from it, and Pick What to Download (`ListChecks`, `secondary`) in Start's place.
13. The history view puts Forget Selected (`ghost`), Move Selected… (`ghost`) and Show Selected in Folder (`secondary`) on its title's right while rows are ticked; what a move did shows as a status message above the list. Its toolbar is two lines: the search with the sort and its direction, then the filters (site, preset, kind, date range, with From and To date fields for Custom…, the Files list: All Files, Only Existing Files, Only Missing Files, and, once something is watched, All Watched Sources and each source by name, at most 14 rem wide). Then the select-all checkbox with the count, what the items listed take on disk and how many are ticked ("5000 items · 412.3 GB on disk · 12 selected", or "no files left on disk"), the size with how many files it counts as hover text and faded while a new search's is counted, and at the line's right end what Tsundoku keeps for the history ("Tsundoku's own data: 42.1 MB", each part as hover text; under the count when the line is too narrow), and one `Card` per video: its checkbox, the thumbnail (112 px: the one kept in the data folder, else the site's picture, else the empty tile), the title, "uploader · site · length", "preset · date · size" for the latest download, and a "File Missing" pill when its file is gone; Open File, Show in Folder and Open Source Page as icon buttons, then a `Menu` with Download Again, Move Files… and Forget. A video with several downloads has an accent "N Downloads" toggle that lists them under it, indented to the text. It's a list rather than a table: at the window's minimum width, eight columns leave no room for titles. Only the rows on screen are drawn, so the 12 px between rows is padding under each row rather than the list's gap.
14. When a newer Tsundoku is out, its banner is the first thing under the add bar, above the preview card: a `Card` with an accent arrow icon, "Tsundoku X is available.", Later (`ghost`) and Install and Restart (`primary`), then a progress bar while it downloads, then, under the text, a What's New toggle (`ExpandToggle`, as in the history) opening the release's notes (`ReleaseNotes`, at most 288 px tall, scrolling), or a Release Notes link (`LinkButton`, with an external link icon) when it has none. After an update, the What's New card sits under it, as a `Card` the same way: an accent sparkles icon, "What's New in Tsundoku X" and × (Close) at the top right, then the changes since the version that ran last, under the text and at most 288 px tall, each version under its own "Version X" in semibold when there are several. Opened from Settings → About's What's New, the same card browses every version instead: its title is "What's New in Tsundoku" followed, 8 px after it, by a select of the versions ("0.2.3 · Nov 21, 2026", the newest first, the running one picked to start with, and focused, so the arrow keys go through them), and under it that one version's changes, starting at their top. It takes the place of the card an update left, and its × closes both. `ReleaseNotes` shows CHANGELOG's Markdown as plain formatted text: its `###` headings in 12 px semibold secondary text, its lists with bullets, **bold**, `code` in the monospace font, and web links in the accent color.
15. Settings is a column of sections, which the sidebar lists under Settings and scrolls to: Downloads, Adding Links, Running in the Background (In the Background in the sidebar), Appearance, Keyboard Shortcuts, Posts of Images, Pacing, Cookies, Supported Sites, Tools, About. Posts of Images' File Name is a monospace field with Use Default (`ghost`) beside it while it differs, the Insert buttons under it, then "For example:" in 12 px muted text with the name filled with sample values in monospace. Keyboard Shortcuts is a `Card` holding a two-column list: the keys, then what they do in secondary text. Pacing and Cookies put their fields inside a sentence, on one line, rather than under a label: Pacing a line per site in a grid whose columns line the fields up, the site's name in medium weight and the words in secondary text ("Instagram from [3] to [5] seconds, [1] at once", the numbers right-aligned), then "At most [ ] Instagram files an hour" under a divider, with How Pacing Works (?) on the title's row; Cookies "Read Cookies from [Automatic]" with its hint under it. A switch that runs something which can fail on its own (Adding Links' Take Links from Click'n'Load Buttons, when another app has the port) keeps the user's choice ticked and says why under its hint, indented with it: a `danger` status message with Try Again (`sm`) as its fix, which tries again without turning the switch off.
16. The watched view is a column (`max-w-4xl`). Check All (`sm`, `secondary`) sits on its title's right. Its toolbar: a `SegmentedControl` of Sources and All New Items (with the count), then, for Sources, "Group By" in 12 px secondary text with None and Site as a `SegmentedControl`, the sort as a select with its direction as an icon button, as in the queue view, and Expand All or Collapse All (`sm`, `ghost`) on the right: Expand All also unfolds the headings folded away. The sources sit under `GroupedList` headings as in the queue view (Needs Attention first, for the sources whose last check failed, then a heading per site, or Other Sources under Needs Attention when ungrouped), each heading followed by its new count in 12 px medium accent text and, on the right, Check All Under it (`RefreshCw`, an icon button). One `Card` per source: a button holding a chevron (16 px, secondary; `ChevronRight` folded, `ChevronDown` open), its picture (`ListingPicture`, small: a 56 px circle, or a 96 px thumbnail for a playlist), its name in semibold, "Channel · youtube.com", "Checked 2 hours ago · 3 new · 12 downloaded · 2 skipped · Last post 3 weeks ago" (or "Checking…" first) in 12 px secondary text, wrapping, the new count in medium accent while above 0, and, open, where it saves and starts as in 12 px muted text; Check Now (`secondary`), Show Downloads in History (`History`, off while nothing was downloaded), Edit and a `Menu` (Stop Watching, in the danger color) as icon buttons on the right. Folded, with downloads, the latest of them follow as a strip under the name's column (past the chevron and the picture): 48 px high pictures (`Thumbnail`, the kept one else the site's), 84 px wide for a video, square for a post, 6 px apart, on one row cut to whole pictures (the row's fixed height hides those that wrap), each a button opening its file, dimmed to 40% and off when the file is gone, its title and "Downloaded <day>" as hover text. A failed check's message and fixes sit under it, folded or not. Edit opens Save In (monospace, with Choose Folder…) and Videos Start As over a line, with Discard and Save. Open, new items follow over a line, as the picker shows them (`ListedItems`, in its view, which they share): a select-all box with "2 of 3 selected" and the view's controls on its right, the items, then Skip All (`ghost`) and Download Selected (`primary`). Under them, Skipped Before as an `ExpandToggle`, opening the same picker with nothing ticked and Download Selected (`secondary`). All New Items is one `Card` holding that picker with every source's new items, each tile's mark under it naming its source ("Downloaded · NASA" when it has one), then Skip All and Download Selected; the column then takes the view's height (at least 20 rem, the view scrolling past that), the card shrinking to it and the items scrolling on their own (`ListedItems`' `fill`) rather than stopping at 28 rem or half the window, so the buttons stay in sight; with none, "Nothing new on any source. Check All looks for more." in secondary text. The picker's Watch (`secondary`) sits at the left of its bottom row and opens the Watch form in its place, over a line: a title, What Counts as New Now as radio rows (the latest count in a 64 px field inside its row), Save In, Videos Start As (not for an account), then Cancel (`ghost`) and Watch (`primary`).
17. The presets view is a list (14 rem, staying in view) beside the selected preset's editor, a `Card` of labeled fields, which save on their own, with "Saving…" or "Saved." in 12 px muted text at the bottom right, and the preset's actions at the top right: Set as Default (`sm`), then Duplicate and Reset (a built-in) or Delete as icon buttons.
18. The rules view is a column (`max-w-3xl`): a sentence on how rules work, then Test a Link, a `Card` with a field and Test, under which, once read, the video's title and "uploader · site", then a two-column list in 12 px (labels in medium secondary text): Saves In (a `DestinationLine`), Starts As and Copied Links, each with "· Rule N" in muted text that scrolls to the rule; then one `Card` per rule: a grip (`GripVertical`, muted, with a Drag to Move tooltip; an empty space on the first rule) to drag it, its number in a 24 px `--color-input` circle, its name in semibold when it has one, the rule as a sentence and, under it, its folder in 12 px monospace secondary text; an "Always First" pill on the first rule, whose sentence is "Everything else", and an "Off" pill when it's off; then Move Up, Move Down, Edit and Duplicate (icon buttons; the first rule has an empty space for Duplicate, so the buttons line up) and a `Menu` (Turn Off or On, Delete). The rule being edited becomes a `Card` headed by its name (else "Rule N"), with a Name field (`max-w-sm`, "Rule N" as its placeholder), then its conditions (a row each: what to check, the comparison, the value, in monospace for a pattern, Match Case (`CaseSensitive`, an icon button that turns `secondary` with accent text and `aria-pressed` while on, its tooltip naming what a click does: Match Case or Ignore Case; an empty space for the kind and the preset), and × to remove; each row after the first starting with a `sm` secondary AND or OR button, in capitals, in a 3.5 rem column the first row leaves empty, for how that condition joins the one above, with a tooltip on what a click does) and Add Condition, or for the first rule a line on what it takes, Save In with Choose Folder… and, under it, `PlaceholderButtons`: "Insert" and a `sm` button for each placeholder ({site}, {uploader}, {account}, {playlist}, {title}, {id}, {kind}, {preset}, {year}, {month}, {date}, {added}, then {1}, {2}… while the rule has a Matches pattern with groups, in monospace, wrapping) that puts it at the field's cursor, and Discard and Save at the bottom right; the other rows' actions are off meanwhile, and their grips dimmed. The editor's fields have no hints: what each means is in How Rules Work, a `HelpMenu` (`CircleHelp`) on the title's right, before Add Rule, under the fields' own labels. While a rule is dragged, a 2 px accent line in the gap shows where it will go. Under the list, the download folder in muted text with Open Folder and Change Download Folder (`ghost`, `sm`).
19. A rule's row also says what it does besides picking a folder, in 12 px secondary text under the folder ("Starts as Audio MP3 · Downloads right away", "Leaves out placeholders without a value"); a rule that picks no folder has no folder line. Its editor, except the first rule's, has File Name under Save In (`components/PostNameInput.tsx`, as Settings → Posts of Images has it: the monospace field, `PlaceholderButtons` and the example, which shows Settings' name while the field is empty, and "Using Default" in the empty field; `{1}`… buttons after the post's while the rule has a pattern with groups), then When a Placeholder Has No Value (a `SegmentedControl`: Write Unknown, Leave Out; the first rule has it too, under Save In), then Start As (a select, Leave as Is first; turned off, with the reason as its hint, its only one, when the rule checks the kind or the preset) Download Right Away (a `SegmentedControl`: Leave as Is, On, Off) and Unpack Archives (a `SegmentedControl`: Default, Skip, Smart, In Place, Folder).
20. Settings → Downloads' Download Folder has Choose Folder… and Open Folder beside its field, then Use Default (`ghost`) once one is chosen. Settings → About is one `Card` of rows split by lines: the version with What's New, which opens the What's New card under the add bar, and Check for Updates, the data folder with Open Folder, then Back Up and Restore with Back Up… and Restore…. Each row has its name in medium weight and a line of secondary text on the left, its `sm` buttons on the right, and what came of them as a status message under it.

---

## Buttons

Use `<Button>` from `tsundoku/src/components/Button.tsx`: a `variant` and a `size`, plus `loading`.

### Variants

| Variant | Intent | Look |
| --- | --- | --- |
| `primary` | The view's main action (Paste Link, Download) | Accent fill |
| `secondary` | Other actions | Input fill, line border |
| `ghost` | Quiet actions in rows, dialog Cancel | Transparent until hovered |
| `danger` | Confirming a destructive action | Danger fill |

### Sizes

| Size | Look | Use |
| --- | --- | --- |
| `sm` | 28 px tall, 12 px text | Inside rows and cards |
| `md` | 32 px tall, 14 px text | The default |
| `icon` | 32 px square | Icon only, through `IconButton` (below) |

### Button Rules

> [!tip]
> For an action in flight, pass `loading` rather than changing the label to "Downloading…": the button disables itself and lays a spinner over its hidden label, so its width never changes.

- `type` defaults to `button`; a form's submit button passes `type="submit"`.
- Full-width rows stay plain `<button>`s, such as the sidebar's navigation items (the selected one gets a background and an accent pill on its left edge).
- Inside a card, actions sit in a row under the content, `sm` size: the main one first (`primary` when it's the obvious next step, such as Download for a tool that isn't set up, otherwise `secondary`), then `ghost` ones.

### Icons or Text

- **An icon** for an action repeated on every row or card of a list, when a common icon says it plainly: starting, removing, stopping, resuming, retrying, cancelling, opening a file or a page, showing a folder, editing, moving, duplicating, deleting.
- **Text** for a view's main and page-wide actions (Paste Link, which has its icon too, Add, Download, Start All, Start Selected, Pause All, Resume All, Retry Failed, Clear Finished, Add Rule), Save and Discard, the fixes under a message (Update yt-dlp, Set Up Cookies), Settings, the first-run setup, and any action no icon says plainly (Set as Default, Choose Folder, Use Default Folder).
- An icon-only button is an `IconButton` (`tsundoku/src/components/IconButton.tsx`): `ghost` by default, `secondary` for a row's main action. Its `label`, from `strings.ts`, is both its tooltip and its `aria-label`, so the two never differ. The tooltip listens on a wrapper, so a disabled one still says what it is.
- Any other control that shows only an icon or a color (the accent swatches) gets the same `Tooltip` (below) rather than a native `title`.
- One action keeps one icon everywhere (lucide names):

| Action | Icon |
| --- | --- |
| Start, Download Again (a pending link) | `Download` |
| Open File | `Play` |
| Resume | `Play` |
| Stop | `CircleStop` |
| Retry, Try Again | `RefreshCw` |
| Try Now | `RefreshCw` |
| Cancel, Remove, Dismiss, Close | `X` |
| Show in Folder | `FolderOpen` |
| Open Source Page | `ExternalLink` |
| Show Log, Hide Log | `FileText` |
| Show Downloads in History (a watched source) | `History` |
| Check Now, Check All Under a group (Watched) | `RefreshCw` |
| Copy Details, then Copied for two seconds | `ClipboardCopy`, then `Check` |
| Paste Link (the add bar), Paste a Link (the ways to add links) | `ClipboardPaste` |
| Type a Link (the add bar) | `TextCursorInput` |
| Pick Files, Hide Files (a post's strip or a shared folder's list in Pending) | `ListChecks` |
| Pick What to Download (a Pending link under Needs Attention, opening its list in the preview card) | `ListChecks` |
| List, Masonry, Gallery (a picker's view) | `LayoutList`, `LayoutDashboard`, `LayoutGrid` |
| Smaller Pictures, Larger Pictures (a picker's size) | `Minus`, `Plus` |
| Expand, Collapse (a folder in a shared folder's list, a source's downloads in the Queue) | `ChevronRight`, `ChevronDown` |
| Pause All, Resume All, Retry Failed, then Cancel All or Clear Finished (a source's heading in the Queue) | `CircleStop`, `Play`, `RefreshCw`, `X` |
| In a download picture's right-click menu, beside the row's own icons: Open Check, Enter Password, Show Details, Hide Details, Download Next, Change Folder…, Use the Usual Folder | `ShieldCheck`, `KeyRound`, `PanelTopOpen`, `ChevronUp`, `ListStart`, `FolderInput`, `RotateCcw` |
| Edit | `Pencil` |
| Move Up, Move Down | `ArrowUp`, `ArrowDown` |
| Duplicate | `Copy` |
| Reset | `RotateCcw` |
| Delete | `Trash2` |
| More actions (a `Menu`) | `MoreHorizontal` |
| Sort direction, ascending or descending | `ArrowUpNarrowWide`, `ArrowDownWideNarrow` |

---

## Feedback

### Progress

- `ProgressBar`: 4 px tall, accent on `--color-line`. With a value it fills; without one, a third-width slider moves across, for work whose length isn't known.
- Under the bar, a status text in 12 px `--color-fg-secondary` says what's happening ("Downloading… 12.3 MB of 86.8 MB"), with tabular figures so it doesn't jitter.
- While an action runs, its buttons give way to Cancel.

### Menus

- `Menu`: a ghost icon button (three dots by default) opening a list of actions under it, right-aligned to the button, on `--color-bg` with a strong line border and a shadow. Escape or a click elsewhere closes it. A destructive item is in the danger color.
- Its button is an `IconButton`, labeled with what it opens.
- `HelpMenu` (`tsundoku/src/components/HelpMenu.tsx`) is the same with a `CircleHelp` icon, for a view whose fields need more explaining than a hint can hold (Rules, on the title's right; Settings → Archives, on its section's title row, through `Section`'s `actions`): its panel (`role="dialog"`, 28 rem wide, scrolling when taller than the window, 16 px padding) holds a heading in semibold per field, under the field's own label, and its paragraphs in `--color-fg-secondary`. Both are built on `Popover` (`tsundoku/src/components/Popover.tsx`).

### Tooltips

- `Tooltip` (`tsundoku/src/components/Tooltip.tsx`) names an icon-only control: 12 px `--color-fg` text on `--color-bg`, a strong line border, 4 px corners and a shadow, like a small `Menu` list, so it follows both themes. It's drawn in a portal over everything, 6 px under the control, centered on it, kept 8 px from the window's edges, and over the control when there's no room below.
- It shows after the pointer rests on the control for 500 ms, or when the keyboard's focus reaches it (not after a click). Within 300 ms of one closing, the next shows at once, so a row of icons reads quickly. A press, Escape, scrolling, resizing or leaving closes it; after a press it stays away until the pointer leaves.
- Other hover texts (truncated titles and paths, the sidebar's shortcuts) stay native `title`s.

### Undo Toasts

- After a removal that can be undone (Pending's Remove, the queue's Clear Finished), `UndoToast` (`tsundoku/src/components/UndoToast.tsx`) shows a toast centered at the bottom of the content layer: what was done, as a short sentence without a period ("Removed 12 links"), then Undo (`sm`, `secondary`) and Dismiss (an `X` icon button). It looks like a `Menu`'s list: `--color-bg`, a strong line border and a shadow.
- It stays 8 seconds, waiting while the pointer is over it or it has the keyboard's focus; Escape or Dismiss closes it. A new one replaces it, so only the latest removal can be undone.
- A view offers one through `useOfferUndo` (`tsundoku/src/undo.ts`). The removal itself happens at once; Undo puts things back. When Undo fails, the toast shows why as a `danger` status message and stays until dismissed.
- Its region is `aria-live`, so screen readers read each toast as it appears.

### Status Messages

- `StatusMessage` reports an outcome or a notice: `success` (check icon in `--color-success`), `warning` (triangle icon in `--color-warning`), `danger` (alert icon in `--color-danger`) or `neutral` (no icon, secondary text).
- The message is a sentence in `--color-fg`. A `detail` (an error's technical cause, a file's path) follows in 12 px `--color-fg-muted`, selectable for copying. Actions about the message, such as Open File, sit in a row under it, `sm` size.
- Lists of details join their parts with " · " (`joinDetails`), skipping the ones that are unknown.
- A failure that has a fix (`FailureFixes`) shows it as buttons under the message: Update yt-dlp (and Try Nightly) when a site's page couldn't be read, Set Up Cookies when a video needs a signed-in account.
- Errors read as what happened plus what to do ("Couldn't reach GitHub. Check your connection, then try again."). Cancelling is the user's choice and shows nothing.

---

## Forms

- Each field is a `Field`: a medium-weight label, the control, then a hint in 12 px muted text, or its error in the danger color instead.
- Text fields and selects share one look (`INPUT_CLASSES` in `components/styles.ts`): 32 px tall, `--color-input`, a line border, the accent border when focused. Selects are native, so their list follows the theme; its options get `--color-bg` and `--color-fg` (`index.css`), since the select's see-through background would otherwise leave the list white in dark mode.
- Code-like values (file name templates, yt-dlp arguments, languages) use the monospace font, Cascadia Mono.
- A field that takes placeholders has `PlaceholderButtons` (`components/PlaceholderButtons.tsx`) under it: "Insert" in 12 px muted text, then a `sm` button per placeholder, in monospace, whose hover text says what it becomes. A click puts it at the field's cursor, over any selection, without taking the focus.
- Edits that take effect at once (a toggle in Settings) save right away; an editor with many fields keeps a draft until Save, with Discard beside it, and asks before leaving unsaved changes, except the preset editor, which saves its fields 600 ms after the last change and before another preset opens, asking only when they can't be saved.
- Confirmations (overwriting a file, deleting a preset, closing while downloading) use Windows' own dialog (`confirmWithUser`), with the action as its button label.
- A link that needs a password is asked with `PasswordForm` (`components/PasswordForm.tsx`): a 14 rem password field ("Password" as its placeholder) and Unlock (`secondary`, off while the field is empty, `loading` while it's tried), Enter submitting. On the preview card, it sits under a `KeyRound` icon (16 px, secondary), "This link needs a password." and the link in 12 px muted text, with × at the top right; a wrong password turns the sentence `--color-danger` and empties the field. On a pending link's row it's `sm` (the field 28 px tall, 12 px text) under a 12 px line with the icon, the sentence and "Starts once it's unlocked" when it was started, without taking the focus. On a queue row, the hold's `neutral` status message has Enter Password (`sm`, `secondary`), which shows the `sm` form in its place. An archive's password is asked with the same `sm` form under the failed row's message, with Password Settings (`ghost`, `sm`) beside it and a 12 px muted line under it saying whether the password is kept. After passwords were tried by themselves, the sentence says how many ("None of the 3 Tsundoku tried opened it."), in the usual color: only a password the person typed turns it `--color-danger`.

---

## Post Files

- A post of several files shows them as a strip (`components/PostFilesStrip.tsx`): over it, a checkbox for all of them with "4 of 6 Files", indeterminate while some are ticked, and on its right the picker's view controls (see [Picker Views](#picker-views)), the post files' own; then the files in that view, scrolling past `min(28rem, 50vh)`. Gallery to start with, its pictures square. A tile (`MediaTiles`) is the file's picture (`object-cover` on `--color-input`; a `Film` or `Image` icon in muted text when there's none, such as an X video), its checkbox at the top left, its number in the post at the bottom left on `--color-bg`, and, for a video, a `Play` mark at the bottom right on `--color-accent-fill`. An unticked file is dimmed to 35%. The whole tile ticks it; Tab and Space work, and the focused tile gets the accent outline. In List, a row per file: its checkbox, a 40 px square of its picture (the `Play` mark in its corner for a video), "Image 3" or "Video 2" (muted while unticked, the file's name as its hover text), its mark and its size when the site gives one.
- A post downloaded before marks each tile under it in 12 px: "Downloaded" in muted text after a `Check` in `--color-success`, or "Not Picked" in the accent, medium weight, with a 2 px accent ring around the tile. A sentence over the strip says how many weren't picked last time, and that those are the ones ticked while they are; once the ticks are others, it says only how many.
- A post of files that aren't all images or videos (a folder on Gofile or Pixeldrain: `post.others` above 0) shows them in the shared folder's list instead (`components/PostFilesPicker.tsx`), flat: each row has its number in the post in 12 px muted tabular text before its name (else "File 3"), and its size when the site gives one; the count leaves the size out when a file has none. Its marks are the strip's: "Downloaded" and "Not Picked".
- A post of one file has no strip. The preview card's Download is off while nothing is ticked, and while it's still asking which files were left out. In Pending, a row's ticks show at once and are saved as they change.

---

## Shared Folder Files

- A file host's link of several files (a shared folder, an Internet Archive item, a transfer) shows them as a tree (`components/SharedFilesList.tsx`): over it, on one line, a checkbox for all of them with "12 of 340 Files · 1.2 GB" (the files ticked and their size, tabular figures), indeterminate while some are ticked, and on the right, for more than ten files, a Filter Files field (14 rem). Then a list that scrolls on its own, at most `min(24rem, 45vh)` tall, of 28 px rows, each hovered with `--color-input`; only the rows on screen are drawn (`useVirtualRows`), so thousands of files stay quick.
- A folder's row: a chevron that folds it (`ChevronDown` open, `ChevronRight` folded, labeled Collapse or Expand with the folder's name), its checkbox (ticked when all its files are, indeterminate when some are; ticking it ticks everything under it), a muted `Folder` icon, its name in medium weight, and on the right "24 files · 1.2 GB" in 12 px secondary text. A file's row: its checkbox, its name truncated (its whole path as the hover text; muted while unticked), and its size on the right in 12 px secondary tabular text. Each level is indented 16 px, a folder's files one level in from its checkbox. The label, not the chevron, ticks: the whole row is clickable.
- The tree starts inside the folder every file is in (the card's title names it): folders first, then files, each in the order the host gives them. A list of more than 200 files starts with its folders folded.
- While the filter has text, only the files whose path from the top holds it show, ignoring case, under their folders, all open and not foldable; the box above ticks only those, with "Select the Files Shown" as its hover text, and a folder's box the files shown under it. The count still counts the whole folder. Nothing matching shows "No files match." in muted text.
- A folder downloaded before marks each file before its size in 12 px: "Downloaded" in muted text after a `Check` in `--color-success`, or "Not Downloaded" in the accent, medium weight ("not downloaded" rather than "not picked": a file the owner added since is one too). A `neutral` status message over the list says how many weren't downloaded before, and those are the ones ticked; in Pending, when the ticks aren't those files (a link read before its folder was downloaded, or ticks changed since), it says only how many.
- A link of one file has no list. The preview card's Download is off while nothing is ticked, and while it's still asking which files were left out. In Pending, a row's ticks show at once and are saved as they change.

---

## Picker Views

- A picker of pictures (a listing's items, a post's files; `components/PickerViewControls.tsx`) offers List, Masonry and Gallery as an icon-only `SegmentedControl` on the right of its select-all line, which goes under it when there's no room. A picker without pictures (a shared folder's files, a post of files that aren't all pictures) stays a list, without them.
- In Masonry and Gallery, − and + (Smaller Pictures, Larger Pictures) sit before it, in a box styled like the `SegmentedControl` (24 px buttons, 14 px icons, a `Tooltip` each), each off at its end of the six sizes. They're gone in List, and the views stay where they are.
- A size is the narrowest a picture may be (64, 88, 120, 160, 200 or 256 px, `TILE_WIDTHS`): as many columns fit as can at that width, and they share what's left, so the pictures fill each row, 8 px apart. The scrollbar's room is always kept, so it never changes how many columns fit. On a picture narrower than 100 px, the labels in its corners shrink to 75%, and under 80 px they're left out, so they don't cover it.
- Gallery is a grid of pictures cut to one shape. Masonry shows each picture in its own shape (kept between 2:5 and 3:1; until it loads, 16:9 for a video, 4:5 for a post, square for a post's file; square for one without a picture, such as an X video), dealt into the columns left to right, a row at a time, so the first ones stay at the top.
- Each picker remembers its view and size in the settings (`pickers`): a listing's items (the preview card and Watched) start in Gallery at 120 px, a post's files in Gallery at 88 px, the strip's size before it had views.
- The Queue's source headings have the same controls, for their downloads (see the queue view in [Layout](#layout)): one choice for every heading, starting in Gallery at 120 px. Gallery cuts an account's pictures square and a channel's or playlist's to 16:9.

---

## Choices

- **A few options side by side** (an update channel): `SegmentedControl`, the selected option filled with the accent. An option with an `icon` (a picker's view) shows only its 14 px icon, its label being its `Tooltip` and `aria-label`; a worded one may have a `hint` as its hover text (a picker's types).
- **One option on or off beside such choices** (By Status in a list's grouping, a picker's Hide Downloaded): `ToggleButton`, one button in the `SegmentedControl`'s frame, filled with the accent while on (`aria-pressed`).
- **A longer list** (a video quality, the preset to download with): a native select.
- **A list of values the person types** (Settings → Passwords' list): `TagInput` (`tsundoku/src/components/TagInput.tsx`), a field styled as an input holding a tag per value (24 px, a line border on `--color-bg`, 12 px monospace, a × that turns `--color-fg` on hover) and a text field after them; Enter adds what's typed, and so does leaving the field. Commas and spaces don't split, since a value may hold them.
- **Options with a line of explanation each** (a tool's copy at setup): radio rows, the whole row clickable, the label in medium weight and the explanation in 12 px secondary text, wrapped (a long path anywhere) so all of it shows.
- **Many items to pick from** (a channel's videos, in the preview card's `ListingPicker`): the card's header as for one video (a channel's or account's picture is a 96 px circle), then a select-all checkbox with "12 of 50 selected" and the picker's view controls on its right (see [Picker Views](#picker-views)), then the items in that view, scrolling on their own (at most 28 rem or half the window). In List, one row per item, the whole row clickable: its checkbox, the thumbnail (96 px), the title and "date · length" in 12 px secondary text. An item the history or the queue has is greyed (muted title, thumbnail at half opacity) with a pill on the right (Downloaded, In Queue: a line border, 12 px secondary text, rounded), and an item in the queue can't be ticked. Rows off screen aren't laid out (`content-visibility: auto`), so hundreds scroll smoothly. In Masonry and Gallery, a tile per item as a post's files have them: its checkbox at the top left, its length (or a post's number of files after an `Images` icon, or a `Play` mark for a post of one video) at the bottom right, dimmed to 35% while unticked, Downloaded (after a `Check` in `--color-success`) or In Queue under it in 12 px muted text, and its title, details and why it's marked as its hover text. Gallery cuts videos' pictures to 16:9 and posts' to squares. Before the view controls, the type filter when the items are of several types (`ItemTypeFilter`: a worded `SegmentedControl`, "All", then "Images (25)", "Videos (13)", "Mixed (12)", "Files (2)" for the types there are, each with its meaning as hover text), then Hide Downloaded, a one-button `ToggleButton` (the `SegmentedControl`'s frame, filled with the accent while on); while they hide some, the count reads "12 of 40 selected · 10 hidden", and when it hides them all, "Everything listed so far is downloaded or in the queue." in secondary text takes the list's place. When only the newest were listed, a centered row under the items holds Load More, Load 50 New and Load All (n) (`secondary`, `sm`; Load All's hint says how many it would list, or that it's counting, or that the site doesn't say); while Load 50 New or Load All runs, "Listing… 150 of 8,634 posts · 12 new" in secondary tabular text and Stop (`secondary`, `sm`) replace them, with a 12 px secondary line under them on Instagram saying why it's slow. While the site makes a load wait (X), "Paused by X, resuming at 4:46 AM (12:31 left)" takes that text's place, counting down every second, beside the same Stop (for Load More too), and the line under it says that it goes on by itself, that the posts listed can be picked meanwhile and how long the rest takes at most. On an X account too big to list in one go, a centered 12 px secondary line (at most 36 rem wide) under the three buttons says how X paces it and how long Load All takes at most, and under the count while it lists, how long is left. Once Load 50 New ends, the list scrolls smoothly to the first new item it listed. A failed load shows a `danger` status message over the row, which stays for another try. At the bottom right, Videos Start As with the preset picker when there are videos, then Download Selected with the count (`primary`).

---

## Keyboard Shortcuts

- A control with a shortcut names it in its hover text, after its label in parentheses: "Queue (Ctrl+1)" (`strings.shortcuts.withKeys`).
- Keys are written as Windows writes them: Ctrl+V, Ctrl+Alt+D, Delete.
- In a list, keys sit in a `<kbd>` pill: 12 px monospace on `--color-input`, with a line border and 4 px corners.
- A shortcut takes Ctrl alone or no modifier, so it's never one of the Clipboard Shortcut's choices, and Ctrl+V and Delete leave text fields alone.

---

## Numbers and Dates

- Sizes in 1024-based units with one decimal from KB up ("22.5 MB"), as Windows shows them (`formatBytes`).
- Lengths as a player shows them ("0:19", "1:02:03") (`formatDuration`).
- Dates in the reader's own format, medium style ("Apr 24, 2005") (`formatCalendarDate` for a calendar date, `formatDay` for a moment).

---

## App Icon

- A placeholder: three books piled up (積ん読) on an ai-iro rounded square, cream, amber and light indigo, drawn bold enough to read at 16 px.
- `tsundoku/src-tauri/icons/source.svg` is the source; `npm run tauri icon src-tauri/icons/source.svg` in `tsundoku/` makes every size (delete the `android` and `ios` folders it adds). A designed icon is in [TODO](./TODO.md).

---

## Labels and Casing

- **Title Case:** buttons, toggles, navigation, headings, dialog titles and buttons, placeholders, column headers and short names. Minor words (a, an, the, and, or, of, to, in, on, for, with, as, at, by, from) stay lowercase unless first or last, and hyphenated words take both capitals.
- **Sentence case:** descriptions, messages, empty-state explanations and status texts.
- **As they are:** yt-dlp's own names (format IDs, codecs, options) and site names.

---

## Motion

- Color changes use Tailwind's `transition-colors` (150 ms).
- A progress bar's fill eases to each new value over 150 ms; its indeterminate slider loops every 1.4 s (`animate-indeterminate`, defined in `index.css`).
- A tooltip fades in over 100 ms (`animate-tooltip-in`).
- Settings slides up or down the sidebar when it opens or closes, over 250 ms (`cubic-bezier(0.2, 0, 0, 1)`), its sections fading in over the same time (`useSettingsSlide` in `Sidebar.tsx`). Not when Windows asks for fewer animations (`prefers-reduced-motion`).
- Nothing else animates yet.

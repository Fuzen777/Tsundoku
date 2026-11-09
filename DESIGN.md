---
type: Agent Instructions
---
# Tsundoku Design Reference

## Guiding Principles

- **A Windows 11 app.** Mica backdrop, Segoe UI Variable, a sidebar and a content layer with a rounded top-left corner, as in Windows' own Settings app: raised above the window in light mode, sunk below it in dark mode, as in RJ Library. It follows Windows' light or dark theme, unless Settings → Appearance picks one.
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
| Content layer | `--color-surface` | The content area, panels: raised above the window in light mode, sunk below it in dark mode |
| Card | `--color-card` | Cards, one step above the content area |
| Interactive background | `--color-input`, `--color-input-hover` | Inputs, secondary buttons, hovered and selected rows, icon tiles |
| Borders | `--color-line`, `--color-line-strong` | Dividers and outlines; strong for emphasis |
| Text | `--color-fg`, `--color-fg-secondary`, `--color-fg-muted` | Primary text; supporting details; labels, empty states and placeholders |
| Accent | `--color-accent`, `--color-accent-hover` | Ai-iro (藍色), Japanese indigo, for marks and text: focus outlines, the selected navigation item's pill, progress bars, links, checkboxes |
| Accent fill | `--color-accent-fill`, `--color-accent-fill-hover`, `--color-on-accent` | A filled background with `--color-on-accent` text: primary buttons, the selected segment, count badges |

Settings → Appearance → Accent Color swaps the four accent tokens for another hue (`[data-accent]` in `index.css`): Indigo, the one above and the default, then Teal, Green, Amber, Rose and Purple. Each keeps the same roles: a light shade for marks and text in dark mode, a deep one for fills under white text in both. Its picker is a row of 28 px round swatches, each filled with its own accent (the swatch carries `data-accent` itself), the selected one with a check and a secondary-colored outline 2 px off it.
| Destructive | `--color-danger` | For text and icons: an error under a field, a destructive menu item, an error message's icon |
| Danger fill | `--color-danger-fill`, `--color-danger-fill-hover`, `--color-on-danger` | The danger button, confirming a destructive action |
| Attention | `--color-warning` | Warnings that don't block: the icon of the duplicate warning, and of Deno missing |
| Success | `--color-success` | Something finished well: the icon of a success message |

### Rules

> [!info]
> With Mica, the window itself is the background: surfaces are translucent so it shows through. `<html data-backdrop="mica">` makes the page transparent; `solid` (Windows 10) paints `--color-bg` instead.

> [!tip]
> When one color would serve two distinct roles, add a token rather than sharing one.

### Dark Mode

Dark mode follows RJ Library's dark theme:

1. Neutral grays with no tint: white or black at an opacity, and gray text.
2. The content area (`--color-surface`, black at 45%) is darker than the window around it, and cards and inputs are lighter again (white at 4% and 7.5%).
3. Lines are white at 14% (24% for `--color-line-strong`).
4. Filled buttons, the selected segment and count badges are dark (`--color-accent-fill`, `--color-danger-fill`) with white text; the light `--color-accent` and `--color-danger` are for marks and text.
5. Every layer is translucent, so Mica still shows through; without it, they sit over `--color-bg` the same way.

---

## Layout

1. The app name (積ん読 beside it) and the add bar run across the top.
2. The sidebar is on the left: the views at the top, Settings at the bottom.
3. The current view sits in the content layer (`--color-surface`, a line border on its top and left, rounded top-left corner).
4. A view is a `Page`: its title, then content that scrolls on its own. Within it, a `Section` groups related content under a 14 px semibold title, and a `Card` raises one item (`--color-card`, a line border, 8 px corners).
5. An empty list shows an `EmptyState`: a 40 px icon, a Title Case title and a one-sentence explanation. The empty Queue and Pending add the ways to add links under it (`WaysToAdd`): a `Card` at most 32 rem wide, one row per way, each a 16 px muted icon, its name, where it stands in 12 px secondary text, and one `sm` button (Paste, Choose File…, Turn On, Set Up or Change), or a check and On in the success color when there's nothing left to do. The queue lists the ways by hand first, Pending the ones that work on their own.
6. The first-run setup replaces the sidebar and the add bar: the app name, then the content layer across the whole width, holding a centered column (`max-w-2xl`) with the title, a sentence, its cards, and the buttons at the bottom right. Its first screen has a card per choice (where the tools go, their caches, the download folder), each an `IconHeading` and radio rows; its second, a card per tool, with Back while no tool is set up.
7. A tool is shown with `ToolHeading`: its icon on a 36 px tile (`--color-input`), its name, and what it does. `IconHeading` is the same for anything else, such as a first-run choice.
8. A view's page-wide buttons (such as Clear Finished) sit on its title's right, `sm` size.
9. The sidebar shows a count beside Queue (downloads not done yet) and Pending (links waiting): an accent pill with the number, hidden at zero.
10. The preview card opens between the add bar and the current view when a link is submitted or dropped, and closes with its × at the top right, whose hover text is Cancel while the link is read, Dismiss after a failure and Close otherwise. It's a `Card`: the thumbnail (176 px wide, 16:9, an empty `--color-input` tile when there's none), then the title (two lines at most), uploader and site, and a line of details (length, upload date, best quality), with × at the top right; then its notices as status messages (the duplicate warning with Open File and Show in Folder as icon buttons under it); for a post of several files, the strip of its files (see [Post Files](#post-files)); then a bottom row: where the download will be saved (a folder icon, the path in 12 px secondary text, truncated, and "· Rule 2", "· Preset's Folder" or "· Download Folder" in muted text, which opens where it's set), then, when a rule picked the preset, "Rule 3" in 12 px muted text, which opens the rules, then the preset picker and Download on the right.
11. The queue view's page-wide buttons are Pause All, Resume All, Retry Failed and Clear Finished, each shown while a download is in a state it acts on. It has a toolbar over its list: the number of downloads, then how the list is grouped and sorted (`ArrangeControls`: "Group By" in 12 px secondary text, Status as a one-button toggle styled like a `SegmentedControl` option, a `SegmentedControl` for None, Site, Folder and Rule, a select for the sort, and the direction as an icon button, as in the history). Grouped, each group has a heading (`GroupedList`), a button that folds the group: a chevron, a folder icon before a folder's path, the label in 12 px semibold secondary text and the number it holds in muted text; groups are 16 px apart, rows 8 px. Grouped by status and something else, a status's group holds the other groups under 12 px medium headings indented 16 px. A queue row is a `Card`: the thumbnail (96 px), then the title, a line of details (preset, stage, size, speed, time left), the progress bar while it runs and its error if it failed; its actions on the right as icon buttons, the main one (Stop, Resume, Retry, Open File) `secondary` and the rest (Show in Folder, Show Log, Copy Details on a failed one, Cancel) `ghost`, then on a waiting row a `Menu` with Download Next. A waiting one held back (its site asked to slow down, or the hourly limit is reached) says why under its details, as a `neutral` status message with the time left as its detail ("Instagram asked to slow down. Continuing at 2:32 PM.", "11:56 left"), counting down. A row that waits to try again, or for its site, has Try Now (`ghost`) after Stop.
12. The pending view puts the Watch Clipboard checkbox and Start All on its title's right (`sm`, `primary`), or Remove Selected (`ghost`) and Start Selected (`primary`) while links are ticked. While copied links or links from dropped files are checked, a line with a spinner counts each above the list. Under the title, a toolbar: the select-all checkbox with the number of links, how the list is grouped and sorted (as in the queue view), and on the right "Videos Start As" with the preset picker, which goes to a second line at the window's minimum width. Grouped, the links sit under headings as in the queue view. Then one `Card` per link, like a queue row: its checkbox, the thumbnail (96 px), the title (or the link until it's read), "uploader · site · length · date · quality" (or "Waiting to be read" / a spinner and "Reading…", with "Starts once it's read"), then, once read, where it will be saved, as on the preview card, then its notices (already downloaded, already in the queue, why it couldn't be read with its fixes); its actions on the right: Pick Files (an icon button) for a post of several files, which opens the strip of its files under its notices, then the preset picker for a video, with "Rule 3" before it as on the preview card, then icon buttons: Start (Download Again once downloaded) or Try Again `secondary`, and Remove `ghost`. A link whose site asked to slow down says so under its status the same way.
13. The history view puts Forget Selected (`ghost`), Move Selected… (`ghost`) and Show Selected in Folder (`secondary`) on its title's right while rows are ticked; what a move did shows as a status message above the list. Its toolbar is two lines: the search with the sort and its direction, then the filters (site, preset, kind, date range, with From and To date fields for Custom…, and the Files list: All Files, Only Existing Files, Only Missing Files). Then the select-all checkbox with the count ("5000 videos · 12 selected"), and one `Card` per video: its checkbox, the thumbnail (112 px: the one kept in the data folder, else the site's picture, else the empty tile), the title, "uploader · site · length", "preset · date · size" for the latest download, and a "File Missing" pill when its file is gone; Open File, Show in Folder and Open Source Page as icon buttons, then a `Menu` with Download Again, Move Files… and Forget. A video with several downloads has an accent "N Downloads" toggle that lists them under it, indented to the text. It's a list rather than a table: at the window's minimum width, eight columns leave no room for titles. Only the rows on screen are drawn, so the 12 px between rows is padding under each row rather than the list's gap.
14. When a newer Tsundoku is out, its banner is the first thing under the add bar, above the preview card: a `Card` with an accent arrow icon, "Tsundoku X is available.", Later (`ghost`) and Install and Restart (`primary`), then a progress bar while it downloads, then, under the text, a What's New toggle (`ExpandToggle`, as in the history) opening the release's notes (`ReleaseNotes`, at most 288 px tall, scrolling), or a Release Notes link (`LinkButton`, with an external link icon) when it has none. After an update, the What's New card sits under it, as a `Card` the same way: an accent sparkles icon, "What's New in Tsundoku X" and × (Close) at the top right, then the changes since the version that ran last, under the text and at most 288 px tall, each version under its own "Version X" in semibold when there are several. `ReleaseNotes` shows CHANGELOG's Markdown as plain formatted text: its `###` headings in 12 px semibold secondary text, its lists with bullets, **bold**, `code` in the monospace font, and web links in the accent color.
15. Settings is a column of sections: Downloads, Adding Links, Running in the Background, Appearance, Keyboard Shortcuts, Posts of Images, Instagram, Cookies, Supported Sites, Tools, About. Posts of Images' File Name is a monospace field with Use Default (`ghost`) beside it while it differs, the Insert buttons under it, then "For example:" in 12 px muted text with the name filled with sample values in monospace. Keyboard Shortcuts is a `Card` holding a two-column list: the keys, then what they do in secondary text.
16. The presets view is a list (14 rem, staying in view) beside the selected preset's editor, a `Card` of labeled fields with Save and Discard at the bottom right and the preset's actions at the top right: Set as Default (`sm`), then Duplicate and Reset (a built-in) or Delete as icon buttons.
17. The rules view is a column (`max-w-3xl`): a sentence on how rules work, then one `Card` per rule: its number in a 24 px `--color-input` circle, the rule as a sentence and, under it, its folder in 12 px monospace secondary text; an "Always Last" pill on the last rule, whose sentence is "Everything else", and an "Off" pill when it's off; then Move Up, Move Down and Edit (icon buttons) and a `Menu` (Turn Off or On, Delete). The rule being edited becomes a `Card` with its conditions (a row each: what to check, the comparison, the value, in monospace for a pattern, and × to remove) and Add Condition, or for the last rule a line on what it takes, Save In with Choose Folder… and, under it, `PlaceholderButtons`: "Insert" and a `sm` button for each placeholder ({site}, {uploader}, {account}, {playlist}, {title}, {id}, {kind}, {preset}, {year}, {month}, {date}, {added}, then {1}, {2}… while the rule has a Matches pattern with groups, in monospace, wrapping) that puts it at the field's cursor, and Discard and Save at the bottom right; the other rows' actions are off meanwhile. Add Rule sits on the title's right. Under the list, the download folder in muted text with Change Download Folder.
18. A rule's row also says what it does besides picking a folder, in 12 px secondary text under the folder ("Starts as Audio MP3 · Downloads right away"); a rule that picks no folder has no folder line. Its editor, except the last rule's, has Start As under Save In (a select, Leave as Is first; turned off, with the reason as its hint, when the rule checks the kind or the preset) and Download Right Away (a `SegmentedControl`: Leave as Is, On, Off).
19. Settings → About is one `Card` of rows split by lines: the version with Check for Updates, the data folder with Open Folder, then Back Up and Restore with Back Up… and Restore…. Each row has its name in medium weight and a line of secondary text on the left, its `sm` buttons on the right, and what came of them as a status message under it.

---

## Buttons

Use `<Button>` from `tsundoku/src/components/Button.tsx`: a `variant` and a `size`, plus `loading`.

### Variants

| Variant | Intent | Look |
| --- | --- | --- |
| `primary` | The view's main action (Add, Download) | Accent fill |
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
- **Text** for a view's main and page-wide actions (Add, Download, Start All, Start Selected, Pause All, Resume All, Retry Failed, Clear Finished, Add Rule), Save and Discard, the fixes under a message (Update yt-dlp, Set Up Cookies), Settings, the first-run setup, and any action no icon says plainly (Set as Default, Choose Folder, Use Default Folder).
- An icon-only button is an `IconButton` (`tsundoku/src/components/IconButton.tsx`): `ghost` by default, `secondary` for a row's main action. Its `label`, from `strings.ts`, is both its hover text and its `aria-label`, so the two never differ. The hover text sits on a wrapper, so a disabled one still says what it is.
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
| Copy Details, then Copied for two seconds | `ClipboardCopy`, then `Check` |
| Pick Files, Hide Files (a post's strip in Pending) | `Images` |
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
- Edits that take effect at once (a toggle in Settings) save right away; an editor with many fields (a preset) keeps a draft until Save, with Discard beside it, and asks before leaving unsaved changes.
- Confirmations (overwriting a file, deleting a preset, closing while downloading) use Windows' own dialog (`confirmWithUser`), with the action as its button label.

---

## Post Files

- A post of several files shows them as a strip (`components/PostFilesStrip.tsx`): over it, a checkbox for all of them with "4 of 6 Files", indeterminate while some are ticked; then a tile per file, 88 px square, wrapping, and scrolling past 256 px. A tile is the file's picture (`object-cover` on `--color-input`; a `Film` or `Image` icon in muted text when there's none, such as an X video), its checkbox at the top left, its number in the post at the bottom left on `--color-bg`, and, for a video, a `Play` mark at the bottom right on `--color-accent-fill`. An unticked file is dimmed to 35%. The whole tile ticks it; Tab and Space work, and the focused tile gets the accent outline.
- A post downloaded before marks each tile under it in 12 px: "Downloaded" in muted text after a `Check` in `--color-success`, or "Not Picked" in the accent, medium weight, with a 2 px accent ring around the tile. A sentence over the strip says how many weren't picked last time, and those are the ones ticked.
- A post of one file has no strip. The preview card's Download is off while nothing is ticked, and while it's still asking which files were left out. In Pending, a row's ticks are saved as they change.

---

## Choices

- **A few options side by side** (an update channel): `SegmentedControl`, the selected option filled with the accent.
- **A longer list** (a video quality, the preset to download with): a native select.
- **Options with a line of explanation each** (a tool's copy at setup): radio rows, the whole row clickable, the label in medium weight and the explanation in 12 px secondary text, wrapped (a long path anywhere) so all of it shows.
- **Many items to pick from** (a channel's videos, in the preview card's `ListingPicker`): the card's header as for one video (a channel's or account's picture is a 96 px circle), then a select-all checkbox with "12 of 50 selected", then a list that scrolls on its own (at most 28 rem or half the window), one row per item, the whole row clickable: its checkbox, the thumbnail (96 px), the title and "date · length" in 12 px secondary text. An item the history or the queue has is greyed (muted title, thumbnail at half opacity) with a pill on the right (Downloaded, In Queue: a line border, 12 px secondary text, rounded), and an item in the queue can't be ticked. Rows off screen aren't laid out (`content-visibility: auto`), so hundreds scroll smoothly. At the bottom right, Videos Start As with the preset picker when there are videos, then Download Selected with the count (`primary`).

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
- Nothing else animates yet.

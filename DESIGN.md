---
type: Agent Instructions
---
# Tsundoku Design Reference

## Guiding Principles

- **A Windows 11 app.** Mica backdrop, Segoe UI Variable, a sidebar and a raised content layer with a rounded top-left corner, as in Windows' own Settings app. It follows Windows' light or dark theme.
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

### Role Mapping

| Role | Token | Usage |
| --- | --- | --- |
| Window background | `--color-bg` | Painted only without Mica (Windows 10) |
| Raised layer | `--color-surface` | The content area, panels |
| Card | `--color-card` | Cards, one step above the content area |
| Interactive background | `--color-input`, `--color-input-hover` | Inputs, secondary buttons, hovered and selected rows, icon tiles |
| Borders | `--color-line`, `--color-line-strong` | Dividers and outlines; strong for emphasis |
| Text | `--color-fg`, `--color-fg-secondary`, `--color-fg-muted` | Primary text; supporting details; labels, empty states and placeholders |
| Accent | `--color-accent`, `--color-accent-hover`, `--color-on-accent` | Ai-iro (藍色), Japanese indigo: primary buttons, the selected navigation item, focus rings |
| Destructive | `--color-danger`, `--color-danger-hover`, `--color-on-danger` | Confirming a destructive action |
| Attention | `--color-warning` | Warnings that don't block: the icon of the duplicate warning, and of Deno missing |
| Success | `--color-success` | Something finished well: the icon of a success message |

### Rules

> [!info]
> With Mica, the window itself is the background: surfaces are translucent so it shows through. `<html data-backdrop="mica">` makes the page transparent; `solid` (Windows 10) paints `--color-bg` instead.

> [!tip]
> When one color would serve two distinct roles, add a token rather than sharing one.

---

## Layout

1. The app name (積ん読 beside it) and the add bar run across the top.
2. The sidebar is on the left: the views at the top, Settings at the bottom.
3. The current view sits in the raised layer (`--color-surface`, a line border on its top and left, rounded top-left corner).
4. A view is a `Page`: its title, then content that scrolls on its own. Within it, a `Section` groups related content under a 14 px semibold title, and a `Card` raises one item (`--color-card`, a line border, 8 px corners).
5. An empty list shows an `EmptyState`: a 40 px icon, a Title Case title and a one-sentence explanation.
6. The first-run setup replaces the sidebar and the add bar: the app name, then the raised layer across the whole width, holding a centered column (`max-w-2xl`) with the title, a sentence, one card per tool, and the buttons at the bottom right.
7. A tool is shown with `ToolHeading`: its icon on a 36 px tile (`--color-input`), its name, and what it does.
8. A view's page-wide buttons (such as Clear Finished) sit on its title's right, `sm` size.
9. The sidebar shows a count beside Queue (downloads not done yet): an accent pill with the number, hidden at zero.
10. The preview card opens between the add bar and the current view when a link is submitted or dropped, and closes with its × (or Cancel, or Dismiss). It's a `Card`: the thumbnail (176 px wide, 16:9, an empty `--color-input` tile when there's none), then the title (two lines at most), uploader and site, and a line of details (length, upload date, best quality), with × at the top right; then its notices as status messages; then the preset picker and Download at the bottom right.
11. A queue row is a `Card`: the thumbnail (96 px), then the title, a line of details (preset, stage, size, speed, time left), the progress bar while it runs and its error if it failed; its actions on the right, `sm` size, the main one (Stop, Resume, Retry, Open File) `secondary` and the rest `ghost`.
12. The presets view is a list (14 rem, staying in view) beside the selected preset's editor, a `Card` of labeled fields with Save and Discard at the bottom right and the preset's actions at the top right.

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
| `icon` | 32 px square | Icon only; always with an `aria-label` from `strings.ts` |

### Button Rules

> [!tip]
> For an action in flight, pass `loading` rather than changing the label to "Downloading…": the button disables itself and lays a spinner over its hidden label, so its width never changes.

- `type` defaults to `button`; a form's submit button passes `type="submit"`.
- Full-width rows stay plain `<button>`s, such as the sidebar's navigation items (the selected one gets a background and an accent pill on its left edge).
- Inside a card, actions sit in a row under the content, `sm` size: the main one first (`primary` when it's the obvious next step, such as Download for a tool that isn't set up, otherwise `secondary`), then `ghost` ones.

---

## Feedback

### Progress

- `ProgressBar`: 4 px tall, accent on `--color-line`. With a value it fills; without one, a third-width slider moves across, for work whose length isn't known.
- Under the bar, a status text in 12 px `--color-fg-secondary` says what's happening ("Downloading… 12.3 MB of 86.8 MB"), with tabular figures so it doesn't jitter.
- While an action runs, its buttons give way to Cancel.

### Status Messages

- `StatusMessage` reports an outcome or a notice: `success` (check icon in `--color-success`), `warning` (triangle icon in `--color-warning`), `danger` (alert icon in `--color-danger`) or `neutral` (no icon, secondary text).
- The message is a sentence in `--color-fg`. A `detail` (an error's technical cause, a file's path) follows in 12 px `--color-fg-muted`, selectable for copying. Actions about the message, such as Open File, sit in a row under it, `sm` size.
- Lists of details join their parts with " · " (`joinDetails`), skipping the ones that are unknown.
- Errors read as what happened plus what to do ("Couldn't reach GitHub. Check your connection, then try again."). Cancelling is the user's choice and shows nothing.

---

## Forms

- Each field is a `Field`: a medium-weight label, the control, then a hint in 12 px muted text, or its error in the danger color instead.
- Text fields and selects share one look (`INPUT_CLASSES` in `components/styles.ts`): 32 px tall, `--color-input`, a line border, the accent border when focused. Selects are native, so their list follows Windows' theme.
- Code-like values (file name templates, yt-dlp arguments, languages) use the monospace font, Cascadia Mono.
- Edits that take effect at once (a toggle in Settings) save right away; an editor with many fields (a preset) keeps a draft until Save, with Discard beside it, and asks before leaving unsaved changes.
- Confirmations (overwriting a file, deleting a preset, closing while downloading) use Windows' own dialog (`confirmWithUser`), with the action as its button label.

---

## Choices

- **A few options side by side** (an update channel): `SegmentedControl`, the selected option filled with the accent.
- **A longer list** (a video quality, the preset to download with): a native select.
- **Options with a line of explanation each** (a tool's copy at setup): radio rows, the whole row clickable, the label in medium weight and the explanation in 12 px secondary text, truncated with the full text in a tooltip.

---

## Numbers and Dates

- Sizes in 1024-based units with one decimal from KB up ("22.5 MB"), as Windows shows them (`formatBytes`).
- Lengths as a player shows them ("0:19", "1:02:03") (`formatDuration`).
- Dates in the reader's own format, medium style ("Apr 24, 2005") (`formatCalendarDate` for a calendar date, `formatDay` for a moment).

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

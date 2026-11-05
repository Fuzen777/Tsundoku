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
| Raised layer | `--color-surface` | The content area, cards, panels |
| Interactive background | `--color-input`, `--color-input-hover` | Inputs, secondary buttons, hovered and selected rows |
| Borders | `--color-line`, `--color-line-strong` | Dividers and outlines; strong for emphasis |
| Text | `--color-fg`, `--color-fg-secondary`, `--color-fg-muted` | Primary text; supporting details; labels, empty states and placeholders |
| Accent | `--color-accent`, `--color-accent-hover`, `--color-on-accent` | Ai-iro (藍色), Japanese indigo: primary buttons, the selected navigation item, focus rings |
| Destructive | `--color-danger`, `--color-danger-hover`, `--color-on-danger` | Confirming a destructive action |
| Attention | `--color-warning` | Warnings that don't block, such as the duplicate warning |

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
4. A view is a `Page`: its title, then content that scrolls on its own.
5. An empty list shows an `EmptyState`: a 40 px icon, a Title Case title and a one-sentence explanation.

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

---

## Labels and Casing

- **Title Case:** buttons, toggles, navigation, headings, dialog titles and buttons, placeholders, column headers and short names. Minor words (a, an, the, and, or, of, to, in, on, for, with, as, at, by, from) stay lowercase unless first or last, and hyphenated words take both capitals.
- **Sentence case:** descriptions, messages, empty-state explanations and status texts.
- **As they are:** yt-dlp's own names (format IDs, codecs, options) and site names.

---

## Motion

- Color changes use Tailwind's `transition-colors` (150 ms). Nothing else animates yet.

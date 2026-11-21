import { Fragment, type ReactNode } from "react"
import { ArrowDownWideNarrow, ArrowUpNarrowWide, ChevronDown, ChevronRight, Folder } from "lucide-react"
import { bundled, groupPath, sharedBundles, type ItemGroup, type ListEntry } from "../arrange"
import type { ListArrangement } from "../bindings/ListArrangement"
import type { ListGroup } from "../bindings/ListGroup"
import type { ListSort } from "../bindings/ListSort"
import { useFoldedGroups, type FoldedGroups, type GroupedView } from "../foldedGroups"
import { strings } from "../strings"
import { SelectInput } from "./Field"
import { IconButton } from "./IconButton"
import { SegmentedControl, ToggleButton } from "./SegmentedControl"

const text = strings.lists

const GROUP_OPTIONS = (["none", "site", "destination", "rule"] as const satisfies readonly ListGroup[]).map((group) => ({
  value: group,
  label: text.groups[group],
}))
const SORTS: readonly ListSort[] = ["added", "title", "uploader", "site"]

interface ArrangeControlsProps {
  arrangement: ListArrangement
  onChange: (changes: Partial<ListArrangement>) => void
  disabled?: boolean
}

/**
 * How the Pending and Queue views group and sort their lists: Status, a switch of its own, then Site, Folder or Rule
 * side by side, then the sort and its direction, as the history's.
 */
export function ArrangeControls({ arrangement, onChange, disabled = false }: ArrangeControlsProps) {
  const { descending, byStatus } = arrangement
  return (
    <div className="flex flex-wrap items-center gap-2">
      <span className="text-xs text-fg-secondary">{text.groupBy}</span>
      <ToggleButton
        label={text.byStatus}
        hint={text.byStatusHint}
        pressed={byStatus}
        disabled={disabled}
        onChange={(pressed) => onChange({ byStatus: pressed })}
      />
      <SegmentedControl
        label={text.groupBy}
        options={GROUP_OPTIONS}
        value={arrangement.group}
        disabled={disabled}
        onChange={(group) => onChange({ group })}
      />
      <SelectInput
        aria-label={text.sortBy}
        title={text.sortBy}
        value={arrangement.sort}
        disabled={disabled}
        onChange={(event) => onChange({ sort: event.target.value as ListSort })}
      >
        {SORTS.map((sort) => (
          <option key={sort} value={sort}>
            {text.sorts[sort]}
          </option>
        ))}
      </SelectInput>
      <IconButton
        label={descending ? text.descending : text.ascending}
        icon={descending ? ArrowDownWideNarrow : ArrowUpNarrowWide}
        variant="secondary"
        disabled={disabled}
        onClick={() => onChange({ descending: !descending })}
      />
    </div>
  )
}

interface GroupedListProps<Item> {
  /** Which view's list it is: each remembers the groups folded away in it. */
  view: GroupedView
  groups: readonly ItemGroup<Item>[]
  /** Whether to show the groups' headings: off when the list isn't grouped. */
  grouped: boolean
  keyOf: (item: Item) => number
  renderItem: (item: Item) => ReactNode
  /** With it, the items that follow one another outside a source's heading are drawn together, and so are a source's
   * (the Downloads view's pictures); without it, each is `renderItem`'s. */
  renderItems?: (items: readonly Item[]) => ReactNode
  /** With it, each heading has a box that ticks or unticks every item in its group (Pending). */
  selection?: GroupSelection<Item>
  /** What a heading shows after its count, such as a group's new items and its actions (Watched). */
  headingExtra?: (group: ItemGroup<Item>) => ReactNode
  /** The view's folded groups, when the view changes them too (Watched's Expand All); else the list keeps its own. */
  folds?: FoldedGroups
  /** With it, the items from one source sit together under a heading of their own wherever they fall (the Queue's
   * downloads picked from one channel). */
  bundles?: Bundling<Item>
}

/** How a list keeps the items from one source together. */
export interface Bundling<Item> {
  /** The source an item came from, or null for an item added on its own. */
  keyOf: (item: Item) => string | null
  /** Draws a source's items in one part of the list: its heading, then `rows` unless it's folded. */
  render: (bundle: ShownBundle<Item>) => ReactNode
}

/** A source's items as a list shows them, with whether they're folded away. */
export interface ShownBundle<Item> {
  items: readonly Item[]
  folded: boolean
  onFold: () => void
  /** Its items' rows (or `renderItems`'). */
  rows: ReactNode
}

/** A list's ticked items, for the boxes on its groups' headings. */
export interface GroupSelection<Item> {
  isSelected: (item: Item) => boolean
  /** Ticks (`selected`) or unticks every item of a group. */
  onSelect: (items: readonly Item[], selected: boolean) => void
}

/**
 * A list of cards, under a heading per group when it's grouped: the group's name (a folder's path with its icon) and
 * how many it holds. A status's group holds the other grouping's groups under smaller headings. Clicking a heading
 * folds its group away; each view remembers which while the app runs. With `selection`, a box before each heading
 * ticks its whole group. With `bundles`, the items of a source two or more share sit together under its own heading in
 * each group they're in, where the first of them comes; it folds the same way. Each heading's section is marked with its
 * key as `data-chapter`, which the sidebar's chapters go to (`chaptersOf`).
 */
export function GroupedList<Item>({
  view,
  groups,
  grouped,
  keyOf,
  renderItem,
  renderItems,
  selection,
  headingExtra,
  folds,
  bundles,
}: GroupedListProps<Item>) {
  const ownFolds = useFoldedGroups(view)
  const { collapsed, toggle } = folds ?? ownFolds
  const row = (item: Item) => <Fragment key={keyOf(item)}>{renderItem(item)}</Fragment>
  // A source with one item in the whole list shows it on its own; one with more shows each under its heading, even one
  // alone in its group (grouped by status, say).
  const shared = bundles ? sharedBundles(groups.flatMap((group) => group.items), bundles.keyOf) : null
  // `path` names the group the items are in, so a source's items folded in one group stay unfolded in another.
  const rows = (items: readonly Item[], path: string) => {
    const sharedKeyOf = (item: Item) => {
      const key = bundles?.keyOf(item) ?? null
      return key !== null && shared?.has(key) ? key : null
    }
    const entries: ListEntry<Item>[] = bundles && shared ? bundled(items, sharedKeyOf) : items.map((item) => ({ item }))
    const drawn: ReactNode[] = []
    // Items outside a source's heading that follow one another, drawn together by `renderItems`.
    let run: Item[] = []
    const endRun = () => {
      if (run.length === 0) return
      drawn.push(<Fragment key={`${path}/run:${drawn.length}`}>{renderItems?.(run)}</Fragment>)
      run = []
    }
    for (const entry of entries) {
      if (!entry.bundle) {
        if (renderItems) run.push(entry.item)
        else drawn.push(row(entry.item))
        continue
      }
      endRun()
      const key = `${path}/bundle:${entry.bundle.key}`
      drawn.push(
        <Fragment key={key}>
          {bundles?.render({
            items: entry.bundle.items,
            folded: collapsed.has(key),
            onFold: () => toggle(key),
            rows: renderItems ? renderItems(entry.bundle.items) : entry.bundle.items.map(row),
          })}
        </Fragment>,
      )
    }
    endRun()
    return drawn
  }
  if (!grouped) return <div className="flex flex-col gap-2">{rows(groups.flatMap((group) => group.items), "")}</div>

  const section = (group: ItemGroup<Item>, path: string, nested: boolean): ReactNode => {
    const key = groupPath(path, group.key)
    // Items in no group of the second grouping sit under their status's heading without one of their own.
    if (nested && group.key === "") return <Fragment key={key}>{rows(group.items, key)}</Fragment>
    const isCollapsed = collapsed.has(key)
    const Chevron = isCollapsed ? ChevronRight : ChevronDown
    return (
      <section key={key} data-chapter={key} className={`flex scroll-mt-2 flex-col ${nested ? "gap-2" : "gap-3"}`}>
        <div className={`flex min-w-0 items-center gap-2 ${nested ? "pl-4" : ""}`}>
          {selection && <GroupCheckbox items={group.items} selection={selection} label={group.label} />}
          <button
            type="button"
            aria-expanded={!isCollapsed}
            aria-label={isCollapsed ? text.expand(group.label) : text.collapse(group.label)}
            onClick={() => toggle(key)}
            className={`flex min-w-0 items-center gap-1.5 rounded text-left text-xs text-fg-secondary hover:text-fg focus-visible:outline-2 focus-visible:outline-accent ${
              nested ? "font-medium" : "font-semibold"
            }`}
          >
            <Chevron size={14} className="shrink-0" aria-hidden />
            {group.folder && <Folder size={14} className="shrink-0" aria-hidden />}
            <span className="truncate" title={group.label}>
              {group.label}
            </span>
            <span className="shrink-0 font-normal text-fg-muted tabular-nums">{group.items.length}</span>
          </button>
          {headingExtra?.(group)}
        </div>
        {!isCollapsed &&
          (group.subgroups ? (
            <div className="flex flex-col gap-3">{group.subgroups.map((subgroup) => section(subgroup, key, true))}</div>
          ) : (
            rows(group.items, key)
          ))}
      </section>
    )
  }
  return <div className="flex flex-col gap-4">{groups.map((group) => section(group, "", false))}</div>
}

/** A group heading's box: ticked when all its items are, part-ticked when some are; it ticks or unticks them all. */
function GroupCheckbox<Item>({
  items,
  selection,
  label,
}: {
  items: readonly Item[]
  selection: GroupSelection<Item>
  label: string
}) {
  const selectedCount = items.filter(selection.isSelected).length
  const all = items.length > 0 && selectedCount === items.length
  return (
    <input
      type="checkbox"
      className="size-4 shrink-0 accent-accent"
      aria-label={text.selectGroup(label)}
      title={text.selectGroup(label)}
      checked={all}
      ref={(box) => {
        if (box) box.indeterminate = selectedCount > 0 && !all
      }}
      onChange={() => selection.onSelect(items, !all)}
    />
  )
}

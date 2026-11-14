import { Fragment, useState, type ReactNode } from "react"
import { ArrowDownWideNarrow, ArrowUpNarrowWide, ChevronDown, ChevronRight, Folder } from "lucide-react"
import type { ItemGroup } from "../arrange"
import type { ListArrangement } from "../bindings/ListArrangement"
import type { ListGroup } from "../bindings/ListGroup"
import type { ListSort } from "../bindings/ListSort"
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
  view: "pending" | "queue"
  groups: readonly ItemGroup<Item>[]
  /** Whether to show the groups' headings: off when the list isn't grouped. */
  grouped: boolean
  keyOf: (item: Item) => number
  renderItem: (item: Item) => ReactNode
  /** With it, each heading has a box that ticks or unticks every item in its group (Pending). */
  selection?: GroupSelection<Item>
}

/** A list's ticked items, for the boxes on its groups' headings. */
export interface GroupSelection<Item> {
  isSelected: (item: Item) => boolean
  /** Ticks (`selected`) or unticks every item of a group. */
  onSelect: (items: readonly Item[], selected: boolean) => void
}

/** The groups folded away in each view, by heading, kept while the app runs so leaving a view doesn't unfold them. */
const collapsedByView: Record<GroupedListProps<unknown>["view"], ReadonlySet<string>> = {
  pending: new Set(),
  queue: new Set(),
}

/**
 * A list of cards, under a heading per group when it's grouped: the group's name (a folder's path with its icon) and
 * how many it holds. A status's group holds the other grouping's groups under smaller headings. Clicking a heading
 * folds its group away; each view remembers which while the app runs. With `selection`, a box before each heading
 * ticks its whole group.
 */
export function GroupedList<Item>({ view, groups, grouped, keyOf, renderItem, selection }: GroupedListProps<Item>) {
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(collapsedByView[view])
  const toggle = (key: string) =>
    setCollapsed((current) => {
      const next = new Set(current)
      if (!next.delete(key)) next.add(key)
      collapsedByView[view] = next
      return next
    })
  const rows = (items: readonly Item[]) =>
    items.map((item) => <Fragment key={keyOf(item)}>{renderItem(item)}</Fragment>)
  if (!grouped) return <div className="flex flex-col gap-2">{rows(groups.flatMap((group) => group.items))}</div>

  const section = (group: ItemGroup<Item>, path: string, nested: boolean): ReactNode => {
    const key = `${path}/${group.key}`
    // Items in no group of the second grouping sit under their status's heading without one of their own.
    if (nested && group.key === "") return <Fragment key={key}>{rows(group.items)}</Fragment>
    const isCollapsed = collapsed.has(key)
    const Chevron = isCollapsed ? ChevronRight : ChevronDown
    return (
      <section key={key} className={`flex flex-col ${nested ? "gap-2" : "gap-3"}`}>
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
        </div>
        {!isCollapsed &&
          (group.subgroups ? (
            <div className="flex flex-col gap-3">{group.subgroups.map((subgroup) => section(subgroup, key, true))}</div>
          ) : (
            rows(group.items)
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

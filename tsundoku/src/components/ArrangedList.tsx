import { Fragment, useState, type ReactNode } from "react"
import { ArrowDownWideNarrow, ArrowUpNarrowWide, ChevronDown, ChevronRight, Folder } from "lucide-react"
import type { ItemGroup } from "../arrange"
import type { ListArrangement } from "../bindings/ListArrangement"
import type { ListGroup } from "../bindings/ListGroup"
import type { ListSort } from "../bindings/ListSort"
import { strings } from "../strings"
import { SelectInput } from "./Field"
import { IconButton } from "./IconButton"
import { SegmentedControl } from "./SegmentedControl"

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
      <div className="inline-flex rounded-md border border-line bg-input p-0.5">
        <button
          type="button"
          aria-pressed={byStatus}
          title={text.byStatusHint}
          disabled={disabled}
          onClick={() => onChange({ byStatus: !byStatus })}
          className={`h-6 rounded px-3 text-xs font-medium transition-colors focus-visible:outline-2 focus-visible:outline-accent disabled:opacity-40 ${
            byStatus ? "bg-accent-fill text-on-accent" : "text-fg-secondary hover:text-fg"
          }`}
        >
          {text.byStatus}
        </button>
      </div>
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
  groups: readonly ItemGroup<Item>[]
  /** Whether to show the groups' headings: off when the list isn't grouped. */
  grouped: boolean
  keyOf: (item: Item) => number
  renderItem: (item: Item) => ReactNode
}

/**
 * A list of cards, under a heading per group when it's grouped: the group's name (a folder's path with its icon) and
 * how many it holds. A status's group holds the other grouping's groups under smaller headings. Clicking a heading
 * folds its group away, until the view is left.
 */
export function GroupedList<Item>({ groups, grouped, keyOf, renderItem }: GroupedListProps<Item>) {
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(new Set())
  const toggle = (key: string) =>
    setCollapsed((current) => {
      const next = new Set(current)
      if (!next.delete(key)) next.add(key)
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
        <button
          type="button"
          aria-expanded={!isCollapsed}
          aria-label={isCollapsed ? text.expand(group.label) : text.collapse(group.label)}
          onClick={() => toggle(key)}
          className={`flex min-w-0 items-center gap-1.5 rounded text-left text-xs text-fg-secondary hover:text-fg focus-visible:outline-2 focus-visible:outline-accent ${
            nested ? "pl-4 font-medium" : "font-semibold"
          }`}
        >
          <Chevron size={14} className="shrink-0" aria-hidden />
          {group.folder && <Folder size={14} className="shrink-0" aria-hidden />}
          <span className="truncate" title={group.label}>
            {group.label}
          </span>
          <span className="shrink-0 font-normal text-fg-muted tabular-nums">{group.items.length}</span>
        </button>
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

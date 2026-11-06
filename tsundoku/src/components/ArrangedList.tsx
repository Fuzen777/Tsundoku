import { Fragment, type ReactNode } from "react"
import { ArrowDownWideNarrow, ArrowUpNarrowWide, Folder } from "lucide-react"
import type { ItemGroup } from "../arrange"
import type { ListArrangement } from "../bindings/ListArrangement"
import type { ListGroup } from "../bindings/ListGroup"
import type { ListSort } from "../bindings/ListSort"
import { strings } from "../strings"
import { SelectInput } from "./Field"
import { IconButton } from "./IconButton"

const text = strings.lists

const GROUPS: readonly ListGroup[] = ["none", "destination", "site", "status"]
const SORTS: readonly ListSort[] = ["added", "title", "uploader", "site"]

interface ArrangeControlsProps {
  arrangement: ListArrangement
  onChange: (changes: Partial<ListArrangement>) => void
  disabled?: boolean
}

/** How the Pending and Queue views group and sort their lists: a select for each, then the direction, as the
 * history's sort. */
export function ArrangeControls({ arrangement, onChange, disabled = false }: ArrangeControlsProps) {
  const { descending } = arrangement
  return (
    <div className="flex items-center gap-2">
      <SelectInput
        aria-label={text.groupBy}
        title={text.groupBy}
        value={arrangement.group}
        disabled={disabled}
        onChange={(event) => onChange({ group: event.target.value as ListGroup })}
      >
        {GROUPS.map((group) => (
          <option key={group} value={group}>
            {text.groups[group]}
          </option>
        ))}
      </SelectInput>
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

/** A list of cards, under a heading per group when it's grouped: the group's name (a folder's path with its icon)
 * and how many it holds. */
export function GroupedList<Item>({ groups, grouped, keyOf, renderItem }: GroupedListProps<Item>) {
  const rows = (items: readonly Item[]) =>
    items.map((item) => <Fragment key={keyOf(item)}>{renderItem(item)}</Fragment>)
  if (!grouped) return <div className="flex flex-col gap-2">{rows(groups.flatMap((group) => group.items))}</div>
  return (
    <div className="flex flex-col gap-4">
      {groups.map((group) => (
        <section key={group.key} className="flex flex-col gap-2">
          <h3 className="flex min-w-0 items-center gap-1.5 text-xs font-semibold text-fg-secondary">
            {group.folder && <Folder size={14} className="shrink-0" />}
            <span className="truncate" title={group.label}>
              {group.label}
            </span>
            <span className="shrink-0 font-normal text-fg-muted tabular-nums">{group.items.length}</span>
          </h3>
          {rows(group.items)}
        </section>
      ))}
    </div>
  )
}

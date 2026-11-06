import type { ListArrangement } from "./bindings/ListArrangement"
import type { ListSort } from "./bindings/ListSort"

/*
  Grouping and sorting the Pending and Queue views' lists, as each view's toolbar chose (ListArrangement, saved in
  settings.json). Each view says what an item's title, site… are and which group it falls in; this does the rest.
*/

/** A list as it's stored: ungrouped, oldest first. Shown until the saved arrangement is read. */
export const STORED_ORDER: ListArrangement = { group: "none", sort: "added", descending: false }

/** What sorting needs to know about an item. */
export interface SortFacts {
  /** The order items were added in, which breaks ties. */
  id: number
  /** In seconds since 1970. */
  addedAt: number
  title: string
  uploader: string | null
  /** The site as people know it ("youtube.com"), or null while it isn't known. */
  site: string | null
}

/** The group an item falls in. Groups come in `rank` order, then by label; items of one group share its `key`. */
export interface GroupPlace {
  key: string
  label: string
  rank: number
  /** The label is a folder's path, shown with a folder icon. */
  folder?: boolean
}

export interface ItemGroup<Item> extends GroupPlace {
  items: Item[]
}

const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" })

/**
 * Sorts the items, then splits them into groups, in the order the items come in within each. Missing values (an
 * uploader not given, a site not known yet) come last whichever the direction, and ties go by when items were added,
 * in the same direction. Ungrouped (`groupOf` null), the list is one group with an empty key and label.
 */
export function arrange<Item>(
  items: readonly Item[],
  arrangement: ListArrangement,
  factsOf: (item: Item) => SortFacts,
  groupOf: ((item: Item) => GroupPlace) | null,
): ItemGroup<Item>[] {
  const direction = arrangement.descending ? -1 : 1
  const sorted = items
    .map((item) => ({ item, facts: factsOf(item) }))
    .sort((first, second) => {
      const firstValue = sortValue(first.facts, arrangement.sort)
      const secondValue = sortValue(second.facts, arrangement.sort)
      if (firstValue === null || secondValue === null) {
        if (firstValue !== secondValue) return firstValue === null ? 1 : -1
      } else {
        const order = compareValues(firstValue, secondValue)
        if (order !== 0) return order * direction
      }
      return (first.facts.id - second.facts.id) * direction
    })
    .map(({ item }) => item)

  if (!groupOf) return [{ key: "", label: "", rank: 0, items: sorted }]
  const groups = new Map<string, ItemGroup<Item>>()
  for (const item of sorted) {
    const place = groupOf(item)
    const group = groups.get(place.key)
    if (group) group.items.push(item)
    else groups.set(place.key, { ...place, items: [item] })
  }
  return [...groups.values()].sort(
    (first, second) => first.rank - second.rank || collator.compare(first.label, second.label),
  )
}

function sortValue(facts: SortFacts, sort: ListSort): string | number | null {
  switch (sort) {
    case "added":
      return facts.addedAt
    case "title":
      return facts.title
    case "uploader":
      return facts.uploader
    case "site":
      return facts.site
  }
}

function compareValues(first: string | number, second: string | number): number {
  if (typeof first === "number" && typeof second === "number") return first - second
  return collator.compare(String(first), String(second))
}

/** The group of a folder. Windows doesn't tell letter case apart in paths, so neither do the groups. */
export function folderGroup(folder: string): GroupPlace {
  return { key: `folder:${folder.toLowerCase()}`, label: folder, rank: 0, folder: true }
}

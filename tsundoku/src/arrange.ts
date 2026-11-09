import type { Destination } from "./bindings/Destination"
import type { ListArrangement } from "./bindings/ListArrangement"
import type { ListSort } from "./bindings/ListSort"
import { strings } from "./strings"

/*
  Grouping and sorting the Pending and Queue views' lists, as each view's toolbar chose (ListArrangement, saved in
  settings.json). Each view says what an item's title, site… are and which group it falls in; this does the rest.
*/

/** A list as it's stored: ungrouped, oldest first. Shown until the saved arrangement is read. */
export const STORED_ORDER: ListArrangement = { group: "none", byStatus: false, sort: "added", descending: false }

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
  /** Within a status's group, the items grouped again by the list's other grouping. */
  subgroups?: ItemGroup<Item>[]
}

/** An item in no group of the second grouping, such as a link not read yet when grouping by site: shown under its
 * status's heading without one of its own. */
export const NO_GROUP: GroupPlace = { key: "", label: "", rank: -1 }

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

/**
 * Arranges the items as `arrange` does, grouped first by `statusOf` when given, then by `groupOf` within each of
 * those groups (or alone, without `statusOf`).
 */
export function arrangeNested<Item>(
  items: readonly Item[],
  arrangement: ListArrangement,
  factsOf: (item: Item) => SortFacts,
  statusOf: ((item: Item) => GroupPlace) | null,
  groupOf: ((item: Item) => GroupPlace) | null,
): ItemGroup<Item>[] {
  if (!statusOf) return arrange(items, arrangement, factsOf, groupOf)
  const byStatus = arrange(items, arrangement, factsOf, statusOf)
  if (!groupOf) return byStatus
  return byStatus.map((group) => ({ ...group, subgroups: arrange(group.items, arrangement, factsOf, groupOf) }))
}

/**
 * The part of a rule's folder that doesn't change from one download to the next: the folders before the first one
 * with a placeholder, joined to the download folder when it isn't a full path. `Downloads\{site}` gives `Downloads`.
 */
export function fixedFolder(template: string, downloadFolder: string): string {
  const network = template.startsWith("\\\\")
  const parts = (network ? template.slice(2) : template).split(/[\\/]/)
  const placeholder = parts.findIndex((part) => /\{[^{}]+\}/.test(part))
  const fixed = (placeholder === -1 ? parts : parts.slice(0, placeholder)).filter(Boolean)
  const joined = (network ? "\\\\" : "") + fixed.join("\\")
  if (network || /^[A-Za-z]:/.test(template)) return joined || template
  return joined ? `${downloadFolder.replace(/[\\/]+$/, "")}\\${joined}` : downloadFolder
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
/** The groups holding the first `count` items, the last of them cut short: a long list shown a page at a time. */
export function firstItems<Item>(groups: readonly ItemGroup<Item>[], count: number): ItemGroup<Item>[] {
  const shown: ItemGroup<Item>[] = []
  let left = count
  for (const group of groups) {
    if (left <= 0) break
    if (group.items.length <= left) shown.push(group)
    else
      shown.push({
        ...group,
        items: group.items.slice(0, left),
        subgroups: group.subgroups && firstItems(group.subgroups, left),
      })
    left -= group.items.length
  }
  return shown
}

export function folderGroup(folder: string): GroupPlace {
  return { key: `folder:${folder.toLowerCase()}`, label: folder, rank: 0, folder: true }
}

/** Ranks that put a group after every rule's group, and after every group of the list's grouping. */
const AFTER_RULES = 1_000_000
export const AFTER_GROUPS = 2_000_000

/**
 * The group of a download by what picks its folder: the rule, with the part of its folder that doesn't change, then
 * the preset's own folder, then the download folder. `ruleFolders` are the rules' folders in their order.
 */
export function ruleGroup(destination: Destination, ruleFolders: readonly string[], downloadFolder: string): GroupPlace {
  const { source } = destination
  switch (source.kind) {
    case "rule": {
      const template = ruleFolders[source.number - 1]
      const folder = template ? fixedFolder(template, downloadFolder) : destination.folder
      return { key: `rule:${source.number}`, label: strings.lists.ruleGroup(source.number, folder), rank: source.number }
    }
    case "preset":
      return {
        key: `preset:${destination.folder.toLowerCase()}`,
        label: strings.lists.presetFolderGroup(destination.folder),
        rank: AFTER_RULES,
      }
    case "downloadFolder":
      return {
        key: "download-folder",
        label: strings.lists.downloadFolderGroup(destination.folder),
        rank: AFTER_RULES + 1,
      }
  }
}

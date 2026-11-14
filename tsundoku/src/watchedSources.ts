import type { ItemGroup } from "./arrange"
import type { WatchedArrangement } from "./bindings/WatchedArrangement"
import type { WatchedItem } from "./bindings/WatchedItem"
import type { WatchedSort } from "./bindings/WatchedSort"
import type { WatchedSource } from "./bindings/WatchedSource"
import { itemKey } from "./listingPicker"
import { strings } from "./strings"

/*
  How the Watched view lays out its sources: sorted and grouped as its toolbar says (WatchedArrangement, saved in
  settings.json), with the sources whose last check failed above the rest, and every source's new items in one list for
  All New Items.
*/

/** The arrangement until the saved one is read: a card each, grouped by site, the ones with the most new items first. */
export const DEFAULT_WATCHED_ARRANGEMENT: WatchedArrangement = {
  show: "sources",
  group: "site",
  sort: "mostNew",
  descending: true,
}

/** The sorts in the order the toolbar lists them. */
export const WATCHED_SORTS: readonly WatchedSort[] = ["mostNew", "name", "lastChecked", "latestPost", "added"]

/** The sources grouped and sorted, and whether the groups get headings. */
export interface ArrangedSources {
  groups: ItemGroup<WatchedSource>[]
  /** Off when there's one group of everything: ungrouped, with no failed check. */
  headings: boolean
}

const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" })

/** Whether a source's last check failed: it sits under Needs Attention, above the rest, until a check works. */
export function needsAttention(source: WatchedSource): boolean {
  return source.lastError !== null
}

/**
 * Sorts the sources, then groups them: Needs Attention first whatever the grouping, then a group per site (by its
 * name), or the rest together, under Other Sources while some need attention.
 */
export function arrangeSources(sources: readonly WatchedSource[], arrangement: WatchedArrangement): ArrangedSources {
  const sorted = sortSources(sources, arrangement.sort, arrangement.descending)
  const failed = sorted.filter(needsAttention)
  const rest = sorted.filter((source) => !needsAttention(source))
  const groups: ItemGroup<WatchedSource>[] = []
  if (failed.length > 0) {
    groups.push({ key: "attention", label: strings.watched.needsAttention, rank: -1, items: failed })
  }
  if (arrangement.group === "site") {
    const bySite = new Map<string, ItemGroup<WatchedSource>>()
    for (const source of rest) {
      const key = `site:${source.siteName.toLowerCase()}`
      const group = bySite.get(key)
      if (group) group.items.push(source)
      else bySite.set(key, { key, label: source.siteName, rank: 0, items: [source] })
    }
    groups.push(...[...bySite.values()].sort((first, second) => collator.compare(first.label, second.label)))
  } else if (rest.length > 0) {
    groups.push({ key: "others", label: failed.length > 0 ? strings.watched.otherSources : "", rank: 0, items: rest })
  }
  return { groups, headings: arrangement.group === "site" || failed.length > 0 }
}

/**
 * The sources in the sort's order. A value a source doesn't have (never checked, no dates) comes last whichever the
 * direction, and ties go by name, then by when it was watched, A and the earliest first.
 */
export function sortSources(sources: readonly WatchedSource[], sort: WatchedSort, descending: boolean): WatchedSource[] {
  const direction = descending ? -1 : 1
  return [...sources].sort((first, second) => {
    const firstValue = sortValue(first, sort)
    const secondValue = sortValue(second, sort)
    if (firstValue === null || secondValue === null) {
      if (firstValue !== secondValue) return firstValue === null ? 1 : -1
    } else {
      const order =
        typeof firstValue === "number" && typeof secondValue === "number"
          ? firstValue - secondValue
          : collator.compare(String(firstValue), String(secondValue))
      if (order !== 0) return order * direction
    }
    return collator.compare(first.name, second.name) || first.addedAt - second.addedAt || first.id - second.id
  })
}

function sortValue(source: WatchedSource, sort: WatchedSort): string | number | null {
  switch (sort) {
    case "mostNew":
      return source.newItems.length
    case "name":
      return source.name
    case "lastChecked":
      return source.lastCheckedAt
    case "latestPost":
      return source.latestPost
    case "added":
      return source.addedAt
  }
}

/** A new item in All New Items, with the source that found it. */
export interface SourcedItem extends WatchedItem {
  sourceId: number
  /** The source's name, with its site when another source has the same name ("NASA on x.com"). */
  sourceName: string
}

/**
 * Every source's new items in one list, the latest posted first (an item without a date by when a check found it). An
 * item two sources found shows once, as the first source watched has it, which is the one Download Selected queues it
 * from.
 */
export function allNewItems(sources: readonly WatchedSource[]): SourcedItem[] {
  const seen = new Set<string>()
  const items: SourcedItem[] = []
  const labels = sourceLabels(sources)
  for (const source of [...sources].sort((first, second) => first.id - second.id)) {
    const sourceName = labels.get(source.id) ?? source.name
    for (const item of source.newItems) {
      const key = itemKey(item)
      if (seen.has(key)) continue
      seen.add(key)
      items.push({ ...item, sourceId: source.id, sourceName })
    }
  }
  return items
    .map((item, index) => ({ item, index, day: postedDay(item) }))
    .sort((first, second) => second.day.localeCompare(first.day) || first.index - second.index)
    .map(({ item }) => item)
}

/**
 * What to call each source where several are named together (All New Items, the history's filter), by ID: its name, or
 * its name and site when another source has the same name ("NASA on x.com").
 */
export function sourceLabels(sources: readonly WatchedSource[]): Map<number, string> {
  const named = new Map<string, number>()
  for (const { name } of sources) named.set(name.toLowerCase(), (named.get(name.toLowerCase()) ?? 0) + 1)
  return new Map(
    sources.map((source) => [
      source.id,
      (named.get(source.name.toLowerCase()) ?? 0) > 1
        ? strings.watched.sourceOnSite(source.name, source.siteName)
        : source.name,
    ]),
  )
}

/** The day an item was posted (YYYY-MM-DD), else the day a check found it, for sorting. */
function postedDay({ video, approximateDate, firstSeenAt }: WatchedItem): string {
  return video.uploadDate ?? approximateDate ?? new Date(firstSeenAt * 1000).toISOString().slice(0, 10)
}

/** How many new items wait across these sources. */
export function newCount(sources: readonly WatchedSource[]): number {
  return sources.reduce((total, source) => total + source.newItems.length, 0)
}

/** Whether a source's card is open, unless it was opened or closed by hand: open while it has new items. */
export function isExpanded(source: WatchedSource, chosen: ReadonlyMap<number, boolean>): boolean {
  return chosen.get(source.id) ?? source.newItems.length > 0
}

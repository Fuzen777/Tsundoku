import { folderName, groupPath, type GroupPlace } from "./arrange"
import type { HistoryArrangement } from "./bindings/HistoryArrangement"
import type { HistoryGroup } from "./bindings/HistoryGroup"
import type { HistoryGroupCount } from "./bindings/HistoryGroupCount"
import type { HistorySort } from "./bindings/HistorySort"
import type { HistoryVideo } from "./bindings/HistoryVideo"
import type { Chapter } from "./chapters"
import { formatMonth } from "./format"
import { strings } from "./strings"

/*
  The History view's list: the videos the core reads a page at a time, in the order it gives them, grouped by it (each
  group's videos together, in the groups' order: `history_groups`), as entries a list that draws only what's on screen
  lays out: a heading per group, then a row per video, or a block of pictures in Masonry and Gallery.
*/

/** How the history is arranged until the settings are read, as the core starts it. */
export const DEFAULT_HISTORY_ARRANGEMENT: HistoryArrangement = { group: "none", sort: "downloaded", descending: true }

/** The history's groupings, in the toolbar's order. */
export const HISTORY_GROUPS: readonly HistoryGroup[] = ["none", "site", "destination", "uploader", "month"]

/** The history's sorts, in the toolbar's order. */
export const HISTORY_SORTS: readonly { value: HistorySort; label: string }[] = (
  ["downloaded", "title", "uploader", "site", "duration", "size", "preset"] as const
).map((sort) => ({ value: sort, label: strings.history.sorts[sort] }))

/** One of the history's groups, with where its videos start among the query's, and how many it holds. */
export interface HistoryGroupShown extends GroupPlace {
  start: number
  count: number
}

/** The core's groups as the view shows them, each starting where the ones before it end. */
export function historyGroups(group: HistoryGroup, counts: readonly HistoryGroupCount[]): HistoryGroupShown[] {
  let start = 0
  return counts.map((counted, rank) => {
    const shown = { ...groupPlace(group, counted), rank, start, count: counted.count }
    start += counted.count
    return shown
  })
}

function groupPlace(group: HistoryGroup, { key, label }: HistoryGroupCount): Omit<GroupPlace, "rank"> {
  const named = label ?? ""
  switch (group) {
    case "destination": {
      // A drive's own folder ends with its separator.
      const folder = /^[A-Za-z]:$/.test(named) ? `${named}\\` : named
      return { key: `folder:${key}`, label: folder, folder: true, chapter: folderName(folder) }
    }
    case "uploader":
      return key === null
        ? { key: "no-uploader", label: strings.lists.noUploader }
        : { key: `uploader:${key}`, label: named }
    case "month":
      return { key: `month:${key}`, label: formatMonth(named) }
    case "site":
    case "none":
      return { key: `${group}:${key}`, label: named }
  }
}

/** The groups as the sidebar's chapters, with how many videos each holds. */
export function historyChapters(groups: readonly HistoryGroupShown[]): Chapter[] {
  return groups.map((group) => ({
    id: groupPath("", group.key),
    title: group.chapter ?? group.label,
    hint: group.label,
    count: group.count,
  }))
}

/** One entry of the list: a group's heading (its chapter's ID as `chapter`), a video's row, or a block of pictures. */
export type HistoryEntry =
  | { kind: "heading"; key: string; group: HistoryGroupShown; chapter: string }
  | { kind: "row"; key: string; video: HistoryVideo }
  | { kind: "tiles"; key: string; videos: HistoryVideo[] }

interface EntriesOptions {
  /** The videos read so far: the first of the query's, in its order. */
  videos: readonly HistoryVideo[]
  /** The query's groups, or null ungrouped. */
  groups: readonly HistoryGroupShown[] | null
  /** The folded groups, by chapter ID: their headings without their videos. */
  folded: ReadonlySet<string>
  /** In Masonry and Gallery, how many pictures a block holds (whole lines of them); null for a row each. */
  perBlock: number | null
}

/**
 * The list's entries for the videos read so far. A group's heading comes once its first video is read, and the groups
 * after one not read whole wait until it is, so the list ends where reading has got to and grows as it goes on.
 */
export function historyEntries({ videos, groups, folded, perBlock }: EntriesOptions): HistoryEntry[] {
  const entries: HistoryEntry[] = []
  const add = (part: readonly HistoryVideo[], groupKey: string) => {
    if (perBlock === null) {
      for (const video of part) entries.push({ kind: "row", key: `video:${video.id}`, video })
      return
    }
    for (let at = 0; at < part.length; at += perBlock) {
      // Named by its place, so the last block keeps its key (and its height) as it fills.
      const key = `tiles:${groupKey}:${at / perBlock}:${perBlock}`
      entries.push({ kind: "tiles", key, videos: part.slice(at, at + perBlock) })
    }
  }
  if (!groups) {
    add(videos, "")
    return entries
  }
  for (const group of groups) {
    if (videos.length <= group.start) break
    const chapter = groupPath("", group.key)
    entries.push({ kind: "heading", key: `heading:${chapter}`, group, chapter })
    const end = group.start + group.count
    if (!folded.has(chapter)) add(videos.slice(group.start, Math.min(end, videos.length)), group.key)
    if (videos.length < end) break
  }
  return entries
}

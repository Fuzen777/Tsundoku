import { describe, expect, it } from "vitest"
import type { WatchedItem } from "./bindings/WatchedItem"
import type { WatchedSource } from "./bindings/WatchedSource"
import {
  allNewItems,
  arrangeSources,
  checkFoundItems,
  DEFAULT_WATCHED_ARRANGEMENT,
  isExpanded,
  latestDownloadsQuery,
  newCount,
  sortSources,
  stripShape,
} from "./watchedSources"

function item(id: string, dates: { uploadDate?: string; approximateDate?: string; firstSeenAt?: number } = {}): WatchedItem {
  return {
    video: {
      site: "youtube",
      siteName: "youtube.com",
      videoId: id,
      url: `https://www.youtube.com/watch?v=${id}`,
      title: `Video ${id}`,
      uploader: null,
      durationSeconds: 10,
      uploadDate: dates.uploadDate ?? null,
      thumbnailUrl: null,
      bestHeight: null,
      post: null,
      files: null,
    },
    approximateDate: dates.approximateDate ?? null,
    firstSeenAt: dates.firstSeenAt ?? 0,
    lastDownload: null,
    inQueue: false,
  }
}

function source(id: number, facts: Partial<WatchedSource> = {}): WatchedSource {
  return {
    id,
    url: `https://www.youtube.com/@source${id}/videos`,
    kind: "channel",
    name: `Source ${id}`,
    siteName: "youtube.com",
    thumbnailUrl: null,
    folder: null,
    presetId: null,
    addedAt: 1000 + id,
    lastCheckedAt: 2000 + id,
    lastError: null,
    checking: false,
    downloadedCount: 0,
    latestPost: null,
    newItems: [],
    skippedItems: [],
    ...facts,
  }
}

const ids = (sources: readonly WatchedSource[]) => sources.map((entry) => entry.id)
const failed = { kind: "failed", detail: "boom" } as const

describe("sortSources", () => {
  const sources = [
    source(1, { name: "beta", newItems: [item("a")], latestPost: "2026-11-01", lastCheckedAt: null }),
    source(2, { name: "Alpha", newItems: [item("b"), item("c")], latestPost: null, lastCheckedAt: 50 }),
    source(3, { name: "gamma", newItems: [], latestPost: "2026-11-09", lastCheckedAt: 90 }),
    source(4, { name: "alpha 2", newItems: [item("d")], latestPost: "2025-01-01", lastCheckedAt: 70 }),
  ]

  it("puts the most new first, ties by name", () => {
    expect(ids(sortSources(sources, "mostNew", true))).toEqual([2, 4, 1, 3])
    expect(ids(sortSources(sources, "mostNew", false))).toEqual([3, 4, 1, 2])
  })

  it("sorts names as people read them", () => {
    expect(ids(sortSources(sources, "name", false))).toEqual([2, 4, 1, 3])
    expect(ids(sortSources(sources, "name", true))).toEqual([3, 1, 4, 2])
  })

  it("leaves what a source doesn't have last, whichever the direction", () => {
    expect(ids(sortSources(sources, "lastChecked", true))).toEqual([3, 4, 2, 1])
    expect(ids(sortSources(sources, "lastChecked", false))).toEqual([2, 4, 3, 1])
    expect(ids(sortSources(sources, "latestPost", true))).toEqual([3, 1, 4, 2])
  })

  it("sorts by when each was watched", () => {
    expect(ids(sortSources(sources, "added", false))).toEqual([1, 2, 3, 4])
    expect(ids(sortSources(sources, "added", true))).toEqual([4, 3, 2, 1])
  })
})

describe("arrangeSources", () => {
  const youtube = source(1, { name: "A channel" })
  const instagram = source(2, { name: "An account", kind: "account", siteName: "instagram.com" })
  const broken = source(3, { name: "Broken", siteName: "x.com", lastError: failed })

  it("groups by site, under Needs Attention's failed checks", () => {
    const { groups, headings } = arrangeSources([youtube, instagram, broken], DEFAULT_WATCHED_ARRANGEMENT)
    expect(headings).toBe(true)
    expect(groups.map((group) => [group.label, ids(group.items)])).toEqual([
      ["Needs Attention", [3]],
      ["instagram.com", [2]],
      ["youtube.com", [1]],
    ])
  })

  it("lists everything without headings when ungrouped and nothing failed", () => {
    const { groups, headings } = arrangeSources([youtube, instagram], {
      ...DEFAULT_WATCHED_ARRANGEMENT,
      group: "none",
      sort: "name",
      descending: false,
    })
    expect(headings).toBe(false)
    expect(groups.map((group) => ids(group.items))).toEqual([[1, 2]])
  })

  it("puts the rest under Other Sources when ungrouped and a check failed", () => {
    const { groups, headings } = arrangeSources([youtube, broken], { ...DEFAULT_WATCHED_ARRANGEMENT, group: "none" })
    expect(headings).toBe(true)
    expect(groups.map((group) => [group.label, ids(group.items)])).toEqual([
      ["Needs Attention", [3]],
      ["Other Sources", [1]],
    ])
  })

  it("has no groups for no sources", () => {
    expect(arrangeSources([], DEFAULT_WATCHED_ARRANGEMENT).groups).toEqual([])
  })
})

describe("allNewItems", () => {
  it("puts every source's new items together, the latest posted first, each once", () => {
    const first = source(1, {
      name: "First",
      newItems: [item("old", { uploadDate: "2026-01-01" }), item("shared", { approximateDate: "2026-11-03" })],
    })
    const second = source(2, {
      name: "Second",
      newItems: [
        item("shared", { uploadDate: "2026-11-03" }),
        item("undated", { firstSeenAt: Date.parse("2026-11-05T12:00:00Z") / 1000 }),
        item("newest", { uploadDate: "2026-11-10" }),
      ],
    })
    const items = allNewItems([second, first])
    expect(items.map((entry) => [entry.video.videoId, entry.sourceName])).toEqual([
      ["newest", "Second"],
      ["undated", "Second"],
      ["shared", "First"],
      ["old", "First"],
    ])
  })

  it("names a source with its site when another one has the same name", () => {
    const sources = [
      source(1, { name: "NASA", siteName: "youtube.com", newItems: [item("a")] }),
      source(2, { name: "nasa", siteName: "x.com", newItems: [item("b")] }),
      source(3, { name: "3Blue1Brown", newItems: [item("c")] }),
    ]
    expect(allNewItems(sources).map((entry) => entry.sourceName).sort()).toEqual([
      "3Blue1Brown",
      "NASA on youtube.com",
      "nasa on x.com",
    ])
  })
})

describe("newCount and isExpanded", () => {
  const quiet = source(1)
  const busy = source(2, { newItems: [item("a"), item("b")] })

  it("counts the new items across the sources", () => {
    expect(newCount([quiet, busy])).toBe(2)
  })

  it("opens a source with new items, unless it was opened or closed by hand", () => {
    expect(isExpanded(quiet, new Map())).toBe(false)
    expect(isExpanded(busy, new Map())).toBe(true)
    expect(isExpanded(busy, new Map([[2, false]]))).toBe(false)
    expect(isExpanded(quiet, new Map([[1, true]]))).toBe(true)
  })
})

describe("checkFoundItems", () => {
  const before = [source(1, { newItems: [item("a")] }), source(2)]

  it("tells when a source has a new item it didn't have", () => {
    expect(checkFoundItems(before, [source(1, { newItems: [item("b"), item("a")] }), source(2)])).toBe(true)
    expect(checkFoundItems(before, [source(1, { newItems: [item("a")] }), source(2, { newItems: [item("c")] })])).toBe(
      true,
    )
  })

  it("is quiet while nothing new came: the same items, fewer, or a check that found none", () => {
    expect(checkFoundItems(before, before)).toBe(false)
    expect(checkFoundItems(before, [source(1), source(2)])).toBe(false)
    expect(checkFoundItems(before, [source(1, { newItems: [item("a")], lastCheckedAt: 9000 }), source(2)])).toBe(false)
  })

  it("doesn't count a source added since, whose new items came with the adding", () => {
    expect(checkFoundItems(before, [...before, source(3, { newItems: [item("d")] })])).toBe(false)
  })
})

describe("the latest downloads strip", () => {
  it("asks the history for what the source met, the latest downloaded first, ungrouped, with no other filter", () => {
    expect(latestDownloadsQuery(7)).toEqual({
      search: null,
      site: null,
      preset: null,
      kind: null,
      files: null,
      downloadedFrom: null,
      downloadedUntil: null,
      watchedSource: 7,
      group: "none",
      sort: "downloaded",
      descending: true,
    })
  })

  it("shows a post's picture square and a video's wide", () => {
    expect(stripShape({ post: { images: 3, videos: 0, others: 0 } })).toBe("square")
    expect(stripShape({ post: null })).toBe("wide")
  })
})

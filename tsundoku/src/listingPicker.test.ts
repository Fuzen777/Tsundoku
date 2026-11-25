import { describe, expect, it } from "vitest"
import type { ListedItem } from "./bindings/ListedItem"
import type { PastDownload } from "./bindings/PastDownload"
import {
  filesIn,
  firstNewSince,
  initialSelection,
  isKnown,
  itemKey,
  itemType,
  mediaCount,
  ofType,
  pickerItems,
  picksFor,
  restEstimate,
  selectedItems,
  selectEvery,
  shownItems,
  typeCounts,
  typeInEffect,
  withItem,
  keptTicks,
  splitByTicks,
  undone,
  withKeptTicks,
  withMoreItems,
  withQueued,
  withSkipped,
  withTicks,
  discardedKeys,
  keyOf,
  type PickedPreset,
  type PickerItems,
} from "./listingPicker"
import { strings } from "./strings"

function item(videoId: string, fields: Partial<ListedItem> = {}, post = false): ListedItem {
  return {
    video: {
      site: post ? "instagram" : "youtube",
      siteName: post ? "instagram.com" : "youtube.com",
      videoId,
      url: `https://example.com/${videoId}`,
      title: videoId,
      uploader: null,
      durationSeconds: null,
      uploadDate: null,
      thumbnailUrl: null,
      bestHeight: null,
      post: post ? { images: 2, videos: 0, others: 0 } : null,
      files: null,
    },
    approximateDate: null,
    lastDownload: null,
    inQueue: false,
    ...fields,
  }
}

function downloadedWith(preset: string): PastDownload {
  return {
    id: 1,
    preset,
    filePath: "D:\\Videos\\a.mkv",
    fileSize: null,
    format: null,
    downloadedAt: 0,
    fileExists: true,
    fileCount: 1,
  }
}

const bestVideo: PickedPreset = { id: "best-video", name: "Best Video" }
const bestAudio: PickedPreset = { id: "best-audio", name: "Best Audio" }

const items = [
  item("new"),
  item("downloaded", { lastDownload: downloadedWith("Best Video") }),
  item("queued", { inQueue: true }),
  item("post", {}, true),
]

describe("a Fanbox post locked behind a plan", () => {
  const locked: ListedItem = {
    ...item("locked"),
    video: { ...item("locked").video, site: "fanbox", fanbox: { fee: 500, locked: true } },
  }
  const open = item("open")
  const listed = [open, locked]

  it("can't be ticked, by hand or with Select All, and stays out of what downloads", () => {
    expect(selectEvery(new Set(), listed, true)).toEqual(new Set([itemKey(open)]))
    expect(withItem(new Set(), locked, true)).toEqual(new Set())
    const ticked = new Set(listed.map(itemKey))
    expect(selectedItems(listed, ticked)).toEqual([open])
    expect(splitByTicks(listed, ticked)).toEqual({ ticked: [open], leftOut: [locked] })
  })
})

describe("the picker's selection", () => {
  it("ticks new items to start with", () => {
    expect([...initialSelection(items)]).toEqual(["youtube new", "instagram post"])
    expect(items.map(isKnown)).toEqual([false, true, true, false])
  })

  it("ticks every item but queued ones, or none", () => {
    expect([...selectEvery(new Set(), items, true)]).toEqual(["youtube new", "youtube downloaded", "instagram post"])
    expect(selectEvery(new Set(["youtube new"]), items, false).size).toBe(0)
  })

  it("leaves the ticks of items it isn't given", () => {
    const ticked = selectEvery(new Set(["youtube other"]), items.slice(0, 2), true)
    expect([...ticked]).toEqual(["youtube other", "youtube new", "youtube downloaded"])
    expect([...selectEvery(ticked, items.slice(0, 2), false)]).toEqual(["youtube other"])
  })

  it("lets a downloaded item be ticked, not a queued one", () => {
    let selection = withItem(new Set(), items[1], true)
    expect(selection.has(itemKey(items[1]))).toBe(true)
    selection = withItem(selection, items[2], true)
    expect(selection.has(itemKey(items[2]))).toBe(false)
    selection = withItem(selection, items[1], false)
    expect(selection.size).toBe(0)
  })

  it("leaves out an item queued since it was ticked", () => {
    const selection = selectEvery(new Set(), items, true)
    const nowQueued = items.map((listed) => (listed.video.videoId === "new" ? { ...listed, inQueue: true } : listed))
    expect(selectedItems(nowQueued, selection).map((listed) => listed.video.videoId)).toEqual(["downloaded", "post"])
  })
})

describe("what Download Selected queues", () => {
  it("gives each video its preset and a post none, in the listing's order", () => {
    const picks = picksFor(items, selectEvery(new Set(), items, true), () => bestAudio)
    expect(picks?.map((pick) => [pick.video.videoId, pick.presetId, pick.overwrite])).toEqual([
      ["new", "best-audio", false],
      ["downloaded", "best-audio", false],
      ["post", null, false],
    ])
  })

  it("replaces a file made before with the same preset", () => {
    const picks = picksFor(items, selectEvery(new Set(), items, true), () => bestVideo)
    expect(picks?.find((pick) => pick.video.videoId === "downloaded")?.overwrite).toBe(true)
  })

  it("follows the preset each video starts with", () => {
    const picks = picksFor(items, selectEvery(new Set(), items, true), (listed) =>
      listed.video.videoId === "new" ? bestAudio : bestVideo,
    )
    expect(picks?.map((pick) => pick.presetId)).toEqual(["best-audio", "best-video", null])
  })

  it("waits while a ticked video's preset isn't known", () => {
    expect(picksFor(items, selectEvery(new Set(), items, true), () => null)).toBeNull()
    expect(picksFor(items, new Set([itemKey(items[3])]), () => null)).toHaveLength(1)
  })
})

describe("withMoreItems", () => {
  const ids = (list: readonly ListedItem[]) => list.map((entry) => entry.video.videoId)

  it("adds the items after the first ones once each, ticking the new ones", () => {
    const start = pickerItems([item("a"), item("b")])
    const more = [item("b"), item("c"), item("d", { lastDownload: downloadedWith("Best Video") }), item("c")]
    const next = withMoreItems(start, more, "channel")
    expect(ids(next.items)).toEqual(["a", "b", "c", "d"])
    expect([...next.selection].sort()).toEqual([itemKey(item("a")), itemKey(item("b")), itemKey(item("c"))].sort())
  })

  it("keeps the ticks made, and ticks the new arrivals even when nothing was ticked", () => {
    const start = withItem(pickerItems([item("a"), item("b")]).selection, item("a"), false)
    const next = withMoreItems({ items: [item("a"), item("b")], selection: start, undo: [] }, [item("c")], "channel")
    expect(next.selection.has(itemKey(item("a")))).toBe(false)
    expect(next.selection.has(itemKey(item("c")))).toBe(true)
    const none = withMoreItems({ items: [item("a")], selection: new Set(), undo: [] }, [item("c")], "channel")
    expect([...none.selection]).toEqual([itemKey(item("c"))])
  })

  it("puts an account's posts in date order, newest first, those without a date last", () => {
    const dated = (videoId: string, uploadDate: string | null) => {
      const post = item(videoId, {}, true)
      return { ...post, video: { ...post.video, uploadDate } }
    }
    // A pinned post comes first, older than the newest.
    const start = pickerItems([dated("pinned", "2024-01-01"), dated("new", "2026-11-01"), dated("older", "2026-10-01")])
    const next = withMoreItems(start, [dated("oldest", "2023-05-05"), dated("undated", null)], "account")
    expect(ids(next.items)).toEqual(["new", "older", "pinned", "oldest", "undated"])
  })

  it("answers the same picker when nothing new came", () => {
    const start = pickerItems([item("a")])
    expect(withMoreItems(start, [item("a")], "channel")).toBe(start)
  })
})

describe("withQueued", () => {
  it("marks the items queued In Downloads and unticks them, leaving the others as they were", () => {
    const start = pickerItems([item("a"), item("b"), item("c")])
    const next = withQueued(start, new Set([itemKey(item("a")), itemKey(item("c"))]))
    expect(next.items.map((entry) => entry.inQueue)).toEqual([true, false, true])
    expect([...next.selection]).toEqual([itemKey(item("b"))])
    expect(start.items[0].inQueue).toBe(false)
  })
})

describe("splitByTicks", () => {
  const ids = (list: readonly ListedItem[]) => list.map((entry) => entry.video.videoId)
  const items = [
    item("ticked"),
    item("unticked"),
    item("downloaded", { lastDownload: downloadedWith("Best Video") }),
    item("queued", { inQueue: true }),
  ]

  it("lists what's ticked on top, in order, and folds the rest away", () => {
    const selection = new Set([itemKey(items[0]), itemKey(items[2]), itemKey(items[3])])
    const { ticked, leftOut } = splitByTicks(items, selection)
    expect(ids(ticked)).toEqual(["ticked", "downloaded"])
    expect(ids(leftOut)).toEqual(["unticked", "queued"])
  })
})

describe("undo", () => {
  const keys = (picker: PickerItems) => [...picker.selection].sort()
  const [a, b, c] = [item("a"), item("b"), item("c")].map(itemKey)

  it("keeps each change of ticks made by hand, and undoes the latest first", () => {
    const start = pickerItems([item("a"), item("b"), item("c")])
    const unticked = withTicks(start, new Set([a, c]))
    const none = withTicks(unticked, new Set())
    expect(none.undo).toEqual([
      { ticked: [], unticked: [b] },
      { ticked: [], unticked: [a, c] },
    ])
    const back = undone(none)
    expect(keys(back)).toEqual([a, c])
    expect(keys(undone(back))).toEqual([a, b, c].sort())
    expect(undone(start)).toBe(start)
    expect(withTicks(start, new Set(start.selection))).toBe(start)
  })

  it("keeps the log through loads and downloads, which aren't changes by hand", () => {
    const start = withTicks(pickerItems([item("a"), item("b")]), new Set([a]))
    const loaded = withMoreItems(start, [item("c")], "channel")
    expect(loaded.undo).toBe(start.undo)
    expect(withQueued(loaded, new Set([a])).undo).toBe(start.undo)
  })
})

describe("withKeptTicks", () => {
  it("opens again with the ticks made, ticks what's listed since as new, and can still undo", () => {
    const first = withTicks(pickerItems([item("a"), item("b")]), new Set([itemKey(item("b"))]))
    const kept = keptTicks(first)
    const again = withKeptTicks(pickerItems([item("new"), item("a"), item("b")]), kept)
    expect([...again.selection].sort()).toEqual([itemKey(item("b")), itemKey(item("new"))].sort())
    expect([...undone(again).selection].sort()).toEqual(
      [itemKey(item("a")), itemKey(item("b")), itemKey(item("new"))].sort(),
    )
  })
})

describe("shownItems", () => {
  it("hides what the history or the queue has only when asked", () => {
    expect(shownItems(items, false)).toBe(items)
    expect(shownItems(items, true).map((entry) => entry.video.videoId)).toEqual(["new", "post"])
  })
})

describe("the type filter", () => {
  const post = (videoId: string, images: number, videos: number, others = 0) => {
    const listed = item(videoId, {}, true)
    return { ...listed, video: { ...listed.video, post: { images, videos, others } } }
  }
  const account = [
    post("photos", 3, 0),
    post("clip", 0, 1),
    post("both", 2, 1),
    post("zip", 0, 0, 1),
    post("more", 1, 0),
  ]

  it("tells what each item holds", () => {
    expect(account.map(itemType)).toEqual(["images", "videos", "mixed", "files", "images"])
    expect(itemType(item("video"))).toBe("videos")
    expect(itemType(post("unknown", 0, 0))).toBe("images")
    expect(itemType(post("zip and picture", 1, 0, 1))).toBe("mixed")
  })

  it("counts the types there are, in the filter's order", () => {
    expect(typeCounts(account)).toEqual([
      { type: "images", count: 2 },
      { type: "videos", count: 1 },
      { type: "mixed", count: 1 },
      { type: "files", count: 1 },
    ])
    expect(typeCounts([item("a"), post("b", 0, 2)])).toEqual([{ type: "videos", count: 2 }])
  })

  it("keeps the type chosen only while the items are of several types and some are of it", () => {
    const counts = typeCounts(account)
    expect(typeInEffect("images", counts)).toBe("images")
    expect(typeInEffect("images", typeCounts([post("clip", 0, 1), post("both", 2, 1)]))).toBe("all")
    expect(typeInEffect("videos", typeCounts([post("clip", 0, 1)]))).toBe("all")
  })

  it("shows the items of the type", () => {
    expect(ofType(account, "all")).toBe(account)
    expect(ofType(account, "images").map((entry) => entry.video.videoId)).toEqual(["photos", "more"])
  })
})

describe("an account that counts its photos and videos", () => {
  const post = (videoId: string, images: number, videos: number, others = 0) => {
    const listed = item(videoId, {}, true)
    return { ...listed, video: { ...listed.video, post: { images, videos, others } } }
  }
  const posts = [post("photos", 4, 0), post("clip", 0, 1), post("both", 2, 1), post("zip", 0, 0, 1)]
  const postNoun = strings.picker.nouns.post
  const text = strings.picker

  it("counts the files of the posts listed, one for a video or a post whose files aren't known", () => {
    expect(filesIn(posts)).toBe(9)
    expect(filesIn([item("video"), post("unknown", 0, 0)])).toBe(2)
    expect(filesIn([])).toBe(0)
  })

  it("sets the site's count against the files listed, unless the posts' count is known", () => {
    expect(mediaCount({ mediaTotal: 372 }, posts, null)).toEqual({ total: 372, listed: 9 })
    expect(mediaCount({ mediaTotal: null }, posts, null)).toBeNull()
    expect(mediaCount({ mediaTotal: 372 }, posts, 369)).toBeNull()
  })

  it("says the count in the site's words on Load All, never as posts", () => {
    expect(text.loadAll(null, 372)).toBe("Load All (372 Photos & Videos)")
    expect(text.loadAll(null, 28259)).toBe(`Load All (${(28259).toLocaleString()} Photos & Videos)`)
    expect(text.loadAll(null, 1)).toBe("Load All (1 Photo or Video)")
    expect(text.loadAll(8634)).toBe(`Load All (${(8634).toLocaleString()})`)
    expect(text.loadAll(8634, 372)).toBe(`Load All (${(8634).toLocaleString()})`)
    expect(text.loadAll(null)).toBe("Load All")
  })

  it("explains the count in Load All's hint", () => {
    expect(text.loadAllHint(50, null, postNoun, null, { total: 372, listed: 86 })).toBe(
      "Lists everything, a page at a time. The site counts 372 photos and videos here, not posts: the 50 posts listed hold 86 of them. Its count includes some it doesn't list, GIFs for one. Stop keeps what's listed by then.",
    )
    expect(text.loadAllHint(1, null, postNoun, null, { total: 372, listed: 4 })).toContain(
      "not posts: the 1 post listed holds 4 of them.",
    )
    // Posts that hold as many as the site counts say nothing of how far the list is.
    expect(text.loadAllHint(50, null, postNoun, null, { total: 60, listed: 64 })).toBe(
      "Lists everything, a page at a time. The site counts 60 photos and videos here, not posts. Its count includes some it doesn't list, GIFs for one. Stop keeps what's listed by then.",
    )
    // Other listings keep their hints.
    expect(text.loadAllHint(50, null, postNoun, null)).toBe(
      "Lists everything, a page at a time. The site doesn't say how many posts there are. Stop keeps what's listed by then.",
    )
    expect(text.loadAllHint(50, 8634, postNoun, null)).toBe(
      `Lists the other ${(8584).toLocaleString()} posts, a page at a time. Stop keeps what's listed by then.`,
    )
  })

  it("counts up against the site's count while listing", () => {
    expect(text.loadingAll(150, null, postNoun, 12, { total: 372, listed: 212 })).toBe(
      "Listing… 150 posts · 212 of 372 photos & videos · 12 new",
    )
    expect(text.loadingAll(369, null, postNoun, 300, { total: 372, listed: 372 })).toBe(
      "Listing… 369 posts · 372 photos & videos · 300 new",
    )
    expect(text.loadingAll(369, null, postNoun, 300, { total: 372, listed: 380 })).toBe(
      "Listing… 369 posts · 380 photos & videos · 300 new",
    )
    expect(text.loadingAll(150, 8634, postNoun, 12)).toBe(`Listing… 150 of ${(8634).toLocaleString()} posts · 12 new`)
    expect(text.loadingAll(150, null, strings.picker.nouns.video, 12)).toBe("Listing… 150 videos · 12 new")
  })
})

describe("a site that lists only so far at a time", () => {
  const pace = { items: 1000, pauseSeconds: 900 }
  const postNoun = strings.picker.nouns.post
  const text = strings.picker

  it("estimates the rest by the waits it takes", () => {
    // 28,196 photos and videos left: 28 stretches of 1,000 filled, a quarter of an hour's wait each.
    expect(restEstimate(pace, { total: 28_260, listed: 64 })).toBe(28 * 15)
    expect(restEstimate(pace, { total: 2400, listed: 50 })).toBe(30)
    expect(restEstimate(pace, { total: 1050, listed: 50 })).toBe(15)
  })

  it("estimates nothing when the rest fits in a stretch, or nothing says how much there is", () => {
    expect(restEstimate(pace, { total: 372, listed: 51 })).toBeNull()
    expect(restEstimate(pace, { total: 1049, listed: 50 })).toBeNull()
    // The posts listed hold more than the site counts.
    expect(restEstimate(pace, { total: 60, listed: 64 })).toBeNull()
    expect(restEstimate(null, { total: 28_260, listed: 64 })).toBeNull()
    expect(restEstimate(pace, null)).toBeNull()
    expect(restEstimate({ items: 0, pauseSeconds: 900 }, { total: 28_260, listed: 64 })).toBeNull()
  })

  it("says a stretch of time to the quarter of an hour, and to the hour from 4 hours", () => {
    expect(text.roughTime(15)).toBe("15 minutes")
    expect(text.roughTime(45)).toBe("45 minutes")
    expect(text.roughTime(60)).toBe("1 hour")
    expect(text.roughTime(75)).toBe("1 hour 15 minutes")
    expect(text.roughTime(150)).toBe("2 hours 30 minutes")
    expect(text.roughTime(225)).toBe("3 hours 45 minutes")
    expect(text.roughTime(240)).toBe("4 hours")
    expect(text.roughTime(420)).toBe("7 hours")
    expect(text.roughTime(435)).toBe("7 hours")
    expect(text.roughTime(450)).toBe("8 hours")
    // Never no time at all.
    expect(text.roughTime(3)).toBe("15 minutes")
  })

  it("says who paused the listing, until when and how long is left", () => {
    expect(text.pausedBySite("X", "1:42 AM", "12:31 left")).toBe("Paused by X, resuming at 1:42 AM (12:31 left)")
    expect(text.pausedBySite("X", "1:42 AM", null)).toBe("Paused by X, resuming at 1:42 AM")
    expect(text.pausedNote(postNoun, "5 hours")).toBe(
      "It goes on by itself, and the posts listed can be picked meanwhile. By X's count, the rest takes up to 5 hours.",
    )
    expect(text.pausedNote(postNoun, null)).toBe("It goes on by itself, and the posts listed can be picked meanwhile.")
  })

  it("says how the site paces a big account, before and while listing it", () => {
    expect(text.paceBefore(1000, 15, postNoun, "7 hours")).toBe(
      `X lists about ${(1000).toLocaleString()} posts, then makes the next ones wait up to 15 minutes. By its count, Load All takes up to 7 hours here, going on by itself after each wait; it ends sooner when X lists fewer posts than it counts. What's listed is kept, so it can be stopped and picked up again later.`,
    )
    expect(text.paceWhile(1000, 15, postNoun, "30 minutes")).toBe(
      `X lists about ${(1000).toLocaleString()} posts, then makes the next ones wait up to 15 minutes: up to 30 minutes left by its count.`,
    )
  })
})

describe("firstNewSince", () => {
  it("finds the first new item Load New listed, past the known ones and those listed before", () => {
    const listed = [
      item("old new"),
      item("downloaded", { lastDownload: downloadedWith("Best Video") }),
      item("queued", { inQueue: true }),
      item("first new"),
      item("second new"),
    ]
    const before = new Set([itemKey(listed[0])])
    expect(firstNewSince(listed, before)?.video.videoId).toBe("first new")
    expect(firstNewSince(listed.slice(0, 3), before)).toBeUndefined()
  })
})

describe("items skipped before", () => {
  const skipped = new Set([keyOf({ site: "youtube", videoId: "b" })])
  const downloaded = { id: 1 } as PastDownload

  it("start unticked, as known items do, and so do loaded ones", () => {
    const picker = pickerItems([item("a"), item("b"), item("c", { lastDownload: downloaded })], skipped)
    expect([...picker.selection]).toEqual([itemKey(item("a"))])
    const more = withMoreItems(picker, [item("d"), item("b2")], "channel", new Set([itemKey(item("d"))]))
    expect(more.selection.has(itemKey(item("d")))).toBe(false)
    expect(more.selection.has(itemKey(item("b2")))).toBe(true)
  })

  it("are what Download leaves unticked of the new items, kept once, those taken leaving", () => {
    const discarded = discardedKeys([item("a"), item("b", { lastDownload: downloaded }), item("c", { inQueue: true })])
    expect(discarded).toEqual([{ site: "youtube", videoId: "a" }])
    const key = (videoId: string) => ({ site: "youtube", videoId })
    expect(withSkipped([key("x"), key("y")], [key("a"), key("x")], [key("y")])).toEqual([key("a"), key("x")])
  })
})

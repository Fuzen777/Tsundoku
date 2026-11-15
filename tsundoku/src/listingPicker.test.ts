import { describe, expect, it } from "vitest"
import type { ListedItem } from "./bindings/ListedItem"
import type { PastDownload } from "./bindings/PastDownload"
import {
  initialSelection,
  isKnown,
  itemKey,
  itemType,
  ofType,
  pickerItems,
  picksFor,
  selectedItems,
  selectEvery,
  shownItems,
  typeCounts,
  typeInEffect,
  withItem,
  withMoreItems,
  type PickedPreset,
} from "./listingPicker"

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

  it("adds the items after the first ones once each, ticking the new ones when anything was ticked", () => {
    const start = pickerItems([item("a"), item("b")])
    const more = [item("b"), item("c"), item("d", { lastDownload: downloadedWith("Best Video") }), item("c")]
    const next = withMoreItems(start, more, "channel", true)
    expect(ids(next.items)).toEqual(["a", "b", "c", "d"])
    expect([...next.selection].sort()).toEqual([itemKey(item("a")), itemKey(item("b")), itemKey(item("c"))].sort())
  })

  it("keeps the ticks made, and leaves added items unticked when nothing was ticked", () => {
    const start = withItem(pickerItems([item("a"), item("b")]).selection, item("a"), false)
    const next = withMoreItems({ items: [item("a"), item("b")], selection: start }, [item("c")], "channel", true)
    expect(next.selection.has(itemKey(item("a")))).toBe(false)
    expect(next.selection.has(itemKey(item("c")))).toBe(true)
    const none = withMoreItems({ items: [item("a")], selection: new Set() }, [item("c")], "channel", false)
    expect(none.selection.size).toBe(0)
  })

  it("puts an account's posts in date order, newest first, those without a date last", () => {
    const dated = (videoId: string, uploadDate: string | null) => {
      const post = item(videoId, {}, true)
      return { ...post, video: { ...post.video, uploadDate } }
    }
    // A pinned post comes first, older than the newest.
    const start = pickerItems([dated("pinned", "2024-01-01"), dated("new", "2026-11-01"), dated("older", "2026-10-01")])
    const next = withMoreItems(start, [dated("oldest", "2023-05-05"), dated("undated", null)], "account", true)
    expect(ids(next.items)).toEqual(["new", "older", "pinned", "oldest", "undated"])
  })

  it("answers the same picker when nothing new came", () => {
    const start = pickerItems([item("a")])
    expect(withMoreItems(start, [item("a")], "channel", true)).toBe(start)
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

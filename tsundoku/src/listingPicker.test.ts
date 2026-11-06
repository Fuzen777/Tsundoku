import { describe, expect, it } from "vitest"
import type { ListedItem } from "./bindings/ListedItem"
import type { PastDownload } from "./bindings/PastDownload"
import {
  initialSelection,
  isKnown,
  itemKey,
  picksFor,
  selectedItems,
  selectEvery,
  withItem,
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
      post: post ? { images: 2, videos: 0 } : null,
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
    expect([...selectEvery(items, true)]).toEqual(["youtube new", "youtube downloaded", "instagram post"])
    expect(selectEvery(items, false).size).toBe(0)
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
    const selection = selectEvery(items, true)
    const nowQueued = items.map((listed) => (listed.video.videoId === "new" ? { ...listed, inQueue: true } : listed))
    expect(selectedItems(nowQueued, selection).map((listed) => listed.video.videoId)).toEqual(["downloaded", "post"])
  })
})

describe("what Download Selected queues", () => {
  it("gives each video its preset and a post none, in the listing's order", () => {
    const picks = picksFor(items, selectEvery(items, true), () => bestAudio)
    expect(picks?.map((pick) => [pick.video.videoId, pick.presetId, pick.overwrite])).toEqual([
      ["new", "best-audio", false],
      ["downloaded", "best-audio", false],
      ["post", null, false],
    ])
  })

  it("replaces a file made before with the same preset", () => {
    const picks = picksFor(items, selectEvery(items, true), () => bestVideo)
    expect(picks?.find((pick) => pick.video.videoId === "downloaded")?.overwrite).toBe(true)
  })

  it("follows the preset each video starts with", () => {
    const picks = picksFor(items, selectEvery(items, true), (listed) =>
      listed.video.videoId === "new" ? bestAudio : bestVideo,
    )
    expect(picks?.map((pick) => pick.presetId)).toEqual(["best-audio", "best-video", null])
  })

  it("waits while a ticked video's preset isn't known", () => {
    expect(picksFor(items, selectEvery(items, true), () => null)).toBeNull()
    expect(picksFor(items, new Set([itemKey(items[3])]), () => null)).toHaveLength(1)
  })
})

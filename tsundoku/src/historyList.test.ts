import { describe, expect, it } from "vitest"
import type { HistoryVideo } from "./bindings/HistoryVideo"
import { historyChapters, historyEntries, historyGroups } from "./historyList"

/** Stand-ins: only their IDs matter here. */
function videos(count: number, from = 1): HistoryVideo[] {
  return Array.from({ length: count }, (_, index) => ({ id: from + index }) as HistoryVideo)
}

const counts = [
  { key: "youtube", label: "youtube.com", count: 3 },
  { key: "vimeo", label: "vimeo.com", count: 2 },
  { key: "x", label: "x.com", count: 4 },
]

describe("the history's groups", () => {
  it("start where the ones before them end", () => {
    const groups = historyGroups("site", counts)
    expect(groups.map(({ key, label, start, count }) => ({ key, label, start, count }))).toEqual([
      { key: "site:youtube", label: "youtube.com", start: 0, count: 3 },
      { key: "site:vimeo", label: "vimeo.com", start: 3, count: 2 },
      { key: "site:x", label: "x.com", start: 5, count: 4 },
    ])
  })

  it("name folders by their path, with their own name in the sidebar, a drive's with its separator", () => {
    const [videosFolder, drive] = historyGroups("destination", [
      { key: "d:\\videos\\zed", label: "D:\\Videos\\Zed", count: 1 },
      { key: "e:", label: "E:", count: 1 },
    ])
    expect(videosFolder).toMatchObject({ label: "D:\\Videos\\Zed", folder: true, chapter: "Zed" })
    expect(drive).toMatchObject({ label: "E:\\", chapter: "E:" })
  })

  it("name the videos without an uploader, and months in words", () => {
    expect(historyGroups("uploader", [{ key: null, label: null, count: 2 }])[0]).toMatchObject({
      key: "no-uploader",
      label: "No Uploader",
    })
    expect(historyGroups("month", [{ key: "2026-11", label: "2026-11", count: 2 }])[0].label).toMatch(/2026/)
  })

  it("are the sidebar's chapters, with their counts", () => {
    expect(historyChapters(historyGroups("site", counts))[1]).toEqual({
      id: "/site:vimeo",
      title: "vimeo.com",
      hint: "vimeo.com",
      count: 2,
    })
  })
})

describe("the history's entries", () => {
  const groups = historyGroups("site", counts)
  const kinds = (entries: ReturnType<typeof historyEntries>) =>
    entries.map((entry) => (entry.kind === "heading" ? entry.chapter : entry.kind === "row" ? entry.video.id : entry.key))

  it("are a row a video ungrouped", () => {
    expect(kinds(historyEntries({ videos: videos(3), groups: null, folded: new Set(), perBlock: null }))).toEqual([
      1, 2, 3,
    ])
  })

  it("head each group read, and stop at the first group not read whole", () => {
    const entries = historyEntries({ videos: videos(4), groups, folded: new Set(), perBlock: null })
    expect(kinds(entries)).toEqual(["/site:youtube", 1, 2, 3, "/site:vimeo", 4])
  })

  it("leave a folded group's videos out, keeping its heading", () => {
    const entries = historyEntries({ videos: videos(9), groups, folded: new Set(["/site:vimeo"]), perBlock: null })
    expect(kinds(entries)).toEqual(["/site:youtube", 1, 2, 3, "/site:vimeo", "/site:x", 6, 7, 8, 9])
  })

  it("show no heading for a group whose first video isn't read yet", () => {
    const entries = historyEntries({ videos: videos(3), groups, folded: new Set(), perBlock: null })
    expect(kinds(entries)).toEqual(["/site:youtube", 1, 2, 3])
  })

  it("put pictures in blocks within each group, named by their place", () => {
    const entries = historyEntries({ videos: videos(9), groups, folded: new Set(), perBlock: 2 })
    expect(kinds(entries)).toEqual([
      "/site:youtube",
      "tiles:site:youtube:0:2",
      "tiles:site:youtube:1:2",
      "/site:vimeo",
      "tiles:site:vimeo:0:2",
      "/site:x",
      "tiles:site:x:0:2",
      "tiles:site:x:1:2",
    ])
    const last = entries.at(-1)
    expect(last?.kind === "tiles" && last.videos.map((video) => video.id)).toEqual([8, 9])
  })
})

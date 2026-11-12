import { describe, expect, it } from "vitest"
import type { SharedFile } from "./bindings/SharedFile"
import type { VideoInfo } from "./bindings/VideoInfo"
import { pickSummary } from "./postFiles"
import {
  defaultSharedPick,
  folderPaths,
  folderState,
  matchingFiles,
  pickedBytes,
  sharedPickedOf,
  sharedTree,
  shownPaths,
  visibleRows,
  withFiles,
  withSharedPicked,
  type SharedRow,
} from "./sharedFiles"

const file = (path: string, size = 1024): SharedFile => ({ path, size, picked: true })

/** A folder as hosts list it, its own name first: a file, a nested folder, a file of the top, another folder. */
const pack: SharedFile[] = [
  file("Pack/Art/cover.png"),
  file("Pack/Art/Sketches/one.png"),
  file("Pack/readme.txt", 10),
  file("Pack/Art/back.png"),
  file("Pack/Music/01 Intro.flac", 2048),
]

/** The rows as "  name" lines, two spaces a level, folders with a slash, for reading the tree at a glance. */
function outline(rows: SharedRow[]): string[] {
  return rows.map((row) =>
    row.kind === "folder" ? `${"  ".repeat(row.depth)}${row.folder.name}/` : `${"  ".repeat(row.depth)}${row.name}`,
  )
}

describe("sharedTree", () => {
  it("starts inside the folder every file is in, folders first, each in the host's order", () => {
    const tree = sharedTree(pack)
    expect(outline(visibleRows(tree, pack, new Set(), null))).toEqual([
      "Art/",
      "  Sketches/",
      "    one.png",
      "  cover.png",
      "  back.png",
      "Music/",
      "  01 Intro.flac",
      "readme.txt",
    ])
    expect(tree.allFiles).toHaveLength(5)
    expect(tree.folders[0].allFiles).toEqual([0, 1, 3])
    expect(tree.folders[0].bytes).toBe(3 * 1024)
    expect(folderPaths(tree)).toEqual(["Art", "Art/Sketches", "Music"])
  })

  it("keeps the top level when the files aren't all in one folder", () => {
    const loose = [file("a.zip"), file("Pack/b.zip")]
    expect(outline(visibleRows(sharedTree(loose), loose, new Set(), null))).toEqual(["Pack/", "  b.zip", "a.zip"])
    expect(shownPaths(loose)).toEqual(["a.zip", "Pack/b.zip"])
  })

  it("drops only one level", () => {
    const deep = [file("Pack/Inner/a.zip"), file("Pack/Inner/b.zip")]
    expect(outline(visibleRows(sharedTree(deep), deep, new Set(), null))).toEqual(["Inner/", "  a.zip", "  b.zip"])
  })
})

describe("visibleRows", () => {
  const tree = sharedTree(pack)

  it("leaves out what a folded folder holds", () => {
    expect(outline(visibleRows(tree, pack, new Set(["Art"]), null))).toEqual([
      "Art/",
      "Music/",
      "  01 Intro.flac",
      "readme.txt",
    ])
    expect(visibleRows(tree, pack, new Set(["Art"]), null)[0]).toMatchObject({ kind: "folder", open: false })
  })

  it("shows only the files that match a filter, under every folder holding one, all open", () => {
    const matching = matchingFiles(shownPaths(pack), "PNG")
    expect(outline(visibleRows(tree, pack, new Set(["Art", "Art/Sketches"]), matching))).toEqual([
      "Art/",
      "  Sketches/",
      "    one.png",
      "  cover.png",
      "  back.png",
    ])
    const byFolder = matchingFiles(shownPaths(pack), "music/")
    expect(outline(visibleRows(tree, pack, new Set(), byFolder))).toEqual(["Music/", "  01 Intro.flac"])
    expect(visibleRows(tree, pack, new Set(), matchingFiles(shownPaths(pack), "nothing"))).toEqual([])
  })

  it("filters by the path the list shows, not the folder every file is in", () => {
    expect(matchingFiles(shownPaths(pack), "pack")?.size).toBe(0)
    expect(matchingFiles(shownPaths(pack), "  ")).toBeNull()
  })
})

describe("folderState and withFiles", () => {
  const tree = sharedTree(pack)
  const art = tree.folders[0]

  it("says whether a folder's files are all ticked, some, or none", () => {
    expect(folderState(art, new Set([0, 1, 3]), null)).toBe("all")
    expect(folderState(art, new Set([1]), null)).toBe("some")
    expect(folderState(art, new Set([2, 4]), null)).toBe("none")
  })

  it("goes by the files shown while filtering", () => {
    const covers = matchingFiles(shownPaths(pack), "cover")
    expect(folderState(art, new Set([0]), covers)).toBe("all")
  })

  it("ticks or unticks everything under a folder", () => {
    expect([...withFiles(new Set([2]), art.allFiles, true)].sort()).toEqual([0, 1, 2, 3])
    expect([...withFiles(new Set([0, 1, 2]), art.allFiles, false)]).toEqual([2])
  })
})

describe("picks", () => {
  const folder: VideoInfo = {
    site: "mega",
    siteName: "mega.nz",
    videoId: "Pack",
    url: "https://mega.nz/folder/Pack#key",
    title: "Pack",
    uploader: null,
    durationSeconds: null,
    uploadDate: null,
    thumbnailUrl: null,
    bestHeight: null,
    post: null,
    files: { count: 5, bytes: 5130 },
    sharedFiles: pack,
  }

  it("ticks every file of a folder never downloaded, or only those its downloads didn't take", () => {
    expect([...defaultSharedPick(pack, null)]).toEqual([0, 1, 2, 3, 4])
    expect([...defaultSharedPick(pack, [])]).toEqual([0, 1, 2, 3, 4])
    expect([...defaultSharedPick(pack, [1, 4])]).toEqual([1, 4])
  })

  it("unticks the files not picked, and reads them back", () => {
    const picked = withSharedPicked(folder, new Set([2, 4]))
    expect(picked.sharedFiles?.map((file) => file.picked)).toEqual([false, false, true, false, true])
    expect([...sharedPickedOf(picked.sharedFiles ?? [])]).toEqual([2, 4])
    expect(pickedBytes(pack, new Set([2, 4]))).toBe(2058)
    expect(folder.sharedFiles?.every((file) => file.picked)).toBe(true)
  })

  it("sums up a download that leaves files out, and says nothing for the whole folder", () => {
    expect(pickSummary(withSharedPicked(folder, new Set([0, 4])))).toBe("2 of 5 files, 3.0 KB")
    expect(pickSummary(folder)).toBeNull()
    expect(pickSummary({ ...folder, sharedFiles: undefined })).toBeNull()
  })
})

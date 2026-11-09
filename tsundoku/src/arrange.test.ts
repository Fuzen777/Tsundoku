import { describe, expect, it } from "vitest"
import {
  arrange,
  arrangeNested,
  firstItems,
  fixedFolder,
  folderGroup,
  NO_GROUP,
  STORED_ORDER,
  type GroupPlace,
  type SortFacts,
} from "./arrange"
import type { ListArrangement } from "./bindings/ListArrangement"

function item(id: number, facts: Partial<SortFacts> = {}): SortFacts {
  return { id, addedAt: 1000 + id, title: `Video ${id}`, uploader: null, site: null, ...facts }
}

const ids = (items: SortFacts[]) => items.map((entry) => entry.id)
const sortedIds = (items: SortFacts[], arrangement: ListArrangement) =>
  ids(arrange(items, arrangement, (entry) => entry, null)[0].items)

describe("arrange", () => {
  it("keeps the stored order by default, in one group", () => {
    const items = [item(1), item(2), item(3)]
    expect(arrange(items, STORED_ORDER, (entry) => entry, null)).toEqual([
      { key: "", label: "", rank: 0, items },
    ])
  })

  it("sorts newest first, ties going by when they were added", () => {
    const items = [item(1, { addedAt: 5 }), item(2, { addedAt: 9 }), item(3, { addedAt: 5 })]
    expect(sortedIds(items, { group: "none", byStatus: false, sort: "added", descending: true })).toEqual([2, 3, 1])
    expect(sortedIds(items, { group: "none", byStatus: false, sort: "added", descending: false })).toEqual([1, 3, 2])
  })

  it("sorts titles as people read them, ignoring case and taking numbers whole", () => {
    const items = [item(1, { title: "episode 10" }), item(2, { title: "Episode 9" }), item(3, { title: "a clip" })]
    expect(sortedIds(items, { group: "none", byStatus: false, sort: "title", descending: false })).toEqual([3, 2, 1])
    expect(sortedIds(items, { group: "none", byStatus: false, sort: "title", descending: true })).toEqual([1, 2, 3])
  })

  it("puts missing values last whichever the direction", () => {
    const items = [item(1), item(2, { uploader: "jawed" }), item(3, { uploader: "Blender" })]
    expect(sortedIds(items, { group: "none", byStatus: false, sort: "uploader", descending: false })).toEqual([3, 2, 1])
    expect(sortedIds(items, { group: "none", byStatus: false, sort: "uploader", descending: true })).toEqual([2, 3, 1])
  })

  it("groups in rank order, then by label, each group keeping the sort", () => {
    const places: Record<number, GroupPlace> = {
      1: { key: "b", label: "youtube.com", rank: 0 },
      2: { key: "later", label: "Not Read Yet", rank: 1 },
      3: { key: "a", label: "instagram.com", rank: 0 },
      4: { key: "b", label: "youtube.com", rank: 0 },
    }
    const items = [item(1), item(2), item(3), item(4)]
    const arrangement: ListArrangement = { group: "site", byStatus: false, sort: "added", descending: true }
    const groups = arrange(items, arrangement, (entry) => entry, (entry) => places[entry.id])
    expect(groups.map((group) => [group.label, ids(group.items)])).toEqual([
      ["instagram.com", [3]],
      ["youtube.com", [4, 1]],
      ["Not Read Yet", [2]],
    ])
  })
})

describe("folderGroup", () => {
  it("puts the same folder in one group whatever its letter case", () => {
    expect(folderGroup(String.raw`D:\Videos\YouTube`).key).toBe(folderGroup(String.raw`d:\videos\youtube`).key)
    expect(folderGroup(String.raw`D:\Videos\YouTube`).label).toBe(String.raw`D:\Videos\YouTube`)
  })
})

describe("arrangeNested", () => {
  const items = [item(1), item(2), item(3), item(4)]
  const facts = (entry: { id: number }): SortFacts => ({
    id: entry.id,
    addedAt: entry.id,
    title: `t${entry.id}`,
    uploader: null,
    site: null,
  })
  const status = (entry: { id: number }): GroupPlace =>
    entry.id <= 2 ? { key: "waiting", label: "Waiting", rank: 0 } : { key: "done", label: "Done", rank: 1 }
  const parity = (entry: { id: number }): GroupPlace =>
    entry.id === 4 ? NO_GROUP : { key: `p${entry.id % 2}`, label: entry.id % 2 ? "Odd" : "Even", rank: 0 }

  it("groups by status, then by the other grouping within each", () => {
    const groups = arrangeNested(items, STORED_ORDER, facts, status, parity)
    expect(groups.map((group) => [group.key, group.subgroups?.map((sub) => [sub.key, ids(sub.items)])])).toEqual([
      [
        "waiting",
        [
          ["p0", [2]],
          ["p1", [1]],
        ],
      ],
      [
        "done",
        [
          ["", [4]],
          ["p1", [3]],
        ],
      ],
    ])
  })

  it("groups by one of them alone", () => {
    expect(arrangeNested(items, STORED_ORDER, facts, status, null).map((group) => group.subgroups)).toEqual([
      undefined,
      undefined,
    ])
    expect(arrangeNested(items, STORED_ORDER, facts, null, parity).map((group) => group.key)).toEqual(["", "p0", "p1"])
  })
})

describe("fixedFolder", () => {
  const downloads = String.raw`C:\Users\You\Downloads\Tsundoku`
  it("keeps the folders before the first placeholder", () => {
    expect(fixedFolder(String.raw`D:\Videos\{site}\{uploader}`, downloads)).toBe(String.raw`D:\Videos`)
    expect(fixedFolder(String.raw`D:\Videos\YouTube`, downloads)).toBe(String.raw`D:\Videos\YouTube`)
    expect(fixedFolder(String.raw`D:\{site}`, downloads)).toBe(String.raw`D:`)
    expect(fixedFolder(String.raw`\\nas\videos\{site}`, downloads)).toBe(String.raw`\\nas\videos`)
  })

  it("joins a folder that isn't a full path to the download folder", () => {
    expect(fixedFolder(String.raw`Music\{uploader}`, downloads)).toBe(String.raw`C:\Users\You\Downloads\Tsundoku\Music`)
    expect(fixedFolder("{site}", downloads)).toBe(downloads)
  })
})

describe("firstItems", () => {
  const groups = [
    { key: "a", label: "A", rank: 0, items: [item(1), item(2)] },
    { key: "b", label: "B", rank: 1, items: [item(3), item(4), item(5)] },
    { key: "c", label: "C", rank: 2, items: [item(6)] },
  ]

  it("keeps the groups holding the first items, the last one cut short", () => {
    const shown = firstItems(groups, 3)
    expect(shown.map((group) => [group.key, ids(group.items)])).toEqual([
      ["a", [1, 2]],
      ["b", [3]],
    ])
  })

  it("keeps everything when there are fewer items", () => {
    expect(firstItems(groups, 200)).toEqual(groups)
  })

  it("cuts the subgroups of the last group short too", () => {
    const nested = [
      {
        key: "s",
        label: "S",
        rank: 0,
        items: [item(1), item(2), item(3)],
        subgroups: [
          { key: "x", label: "X", rank: 0, items: [item(1)] },
          { key: "y", label: "Y", rank: 0, items: [item(2), item(3)] },
        ],
      },
    ]
    const shown = firstItems(nested, 2)
    expect(shown[0].subgroups?.map((sub) => [sub.key, ids(sub.items)])).toEqual([
      ["x", [1]],
      ["y", [2]],
    ])
  })
})

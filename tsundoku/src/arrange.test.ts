import { describe, expect, it } from "vitest"
import { arrange, firstItems, folderGroup, STORED_ORDER, type GroupPlace, type SortFacts } from "./arrange"
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
    expect(sortedIds(items, { group: "none", sort: "added", descending: true })).toEqual([2, 3, 1])
    expect(sortedIds(items, { group: "none", sort: "added", descending: false })).toEqual([1, 3, 2])
  })

  it("sorts titles as people read them, ignoring case and taking numbers whole", () => {
    const items = [item(1, { title: "episode 10" }), item(2, { title: "Episode 9" }), item(3, { title: "a clip" })]
    expect(sortedIds(items, { group: "none", sort: "title", descending: false })).toEqual([3, 2, 1])
    expect(sortedIds(items, { group: "none", sort: "title", descending: true })).toEqual([1, 2, 3])
  })

  it("puts missing values last whichever the direction", () => {
    const items = [item(1), item(2, { uploader: "jawed" }), item(3, { uploader: "Blender" })]
    expect(sortedIds(items, { group: "none", sort: "uploader", descending: false })).toEqual([3, 2, 1])
    expect(sortedIds(items, { group: "none", sort: "uploader", descending: true })).toEqual([2, 3, 1])
  })

  it("groups in rank order, then by label, each group keeping the sort", () => {
    const places: Record<number, GroupPlace> = {
      1: { key: "b", label: "youtube.com", rank: 0 },
      2: { key: "later", label: "Not Read Yet", rank: 1 },
      3: { key: "a", label: "instagram.com", rank: 0 },
      4: { key: "b", label: "youtube.com", rank: 0 },
    }
    const items = [item(1), item(2), item(3), item(4)]
    const arrangement: ListArrangement = { group: "site", sort: "added", descending: true }
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
})

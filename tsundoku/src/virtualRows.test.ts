import { describe, expect, it } from "vitest"
import { layOutRows, rowsBetween } from "./virtualRows"

describe("layOutRows", () => {
  it("uses measured heights where known and the estimate elsewhere", () => {
    const layout = layOutRows([1, 2, 3], new Map([["2", 150]]), 100)
    expect(layout).toEqual({ starts: [0, 100, 250], total: 350 })
  })

  it("lays out no rows", () => {
    expect(layOutRows([], new Map(), 100)).toEqual({ starts: [], total: 0 })
  })
})

describe("rowsBetween", () => {
  // Ten rows of 100 px.
  const layout = layOutRows([0, 1, 2, 3, 4, 5, 6, 7, 8, 9], new Map(), 100)

  it("takes every row even partly in view, with the space of those above and below", () => {
    expect(rowsBetween(layout, 250, 420)).toEqual({ first: 2, last: 4, before: 200, after: 500 })
  })

  it("starts at the first row and ends at the last", () => {
    expect(rowsBetween(layout, -300, 150)).toEqual({ first: 0, last: 1, before: 0, after: 800 })
    expect(rowsBetween(layout, 850, 5000)).toEqual({ first: 8, last: 9, before: 800, after: 0 })
  })

  it("draws a row when the view starts exactly where it does", () => {
    expect(rowsBetween(layout, 300, 300)).toEqual({ first: 3, last: 3, before: 300, after: 600 })
  })

  it("keeps the last row when scrolled past the end", () => {
    expect(rowsBetween(layout, 2000, 2500)).toEqual({ first: 9, last: 9, before: 900, after: 0 })
  })

  it("draws nothing without rows", () => {
    expect(rowsBetween(layOutRows([], new Map(), 100), 0, 500)).toEqual({ first: 0, last: -1, before: 0, after: 0 })
  })
})

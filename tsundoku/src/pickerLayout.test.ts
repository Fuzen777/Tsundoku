import { describe, expect, it } from "vitest"
import type { PickerLayout } from "./bindings/PickerLayout"
import type { PickerView } from "./bindings/PickerView"
import {
  columnsFor,
  intoColumns,
  lineEnd,
  ownShape,
  resized,
  sizeStep,
  TILE_GAP,
  TILE_WIDTHS,
  tileWidthIn,
} from "./pickerLayout"

function layout(view: PickerView, size: number): PickerLayout {
  return { view, size, hideDownloaded: false }
}

describe("sizeStep", () => {
  it("keeps a size within the steps", () => {
    expect(sizeStep(2)).toBe(2)
    expect(sizeStep(-3)).toBe(0)
    expect(sizeStep(99)).toBe(TILE_WIDTHS.length - 1)
    expect(sizeStep(1.6)).toBe(2)
    expect(sizeStep(Number.NaN)).toBe(0)
  })
})

describe("resized", () => {
  it("goes one step smaller or larger, keeping the view", () => {
    expect(resized(layout("masonry", 2), 1)).toEqual(layout("masonry", 3))
    expect(resized(layout("gallery", 2), -1)).toEqual(layout("gallery", 1))
  })

  it("stops at either end", () => {
    expect(resized(layout("gallery", 0), -1)).toBeNull()
    expect(resized(layout("gallery", TILE_WIDTHS.length - 1), 1)).toBeNull()
  })

  it("steps from a size another version kept", () => {
    expect(resized(layout("gallery", 40), -1)).toEqual(layout("gallery", TILE_WIDTHS.length - 2))
  })
})

describe("columnsFor", () => {
  it("fits as many pictures as their narrowest width allows", () => {
    const width = TILE_WIDTHS[2]
    expect(columnsFor(width * 4 + TILE_GAP * 3, 2)).toBe(4)
    expect(columnsFor(width * 4 + TILE_GAP * 3 - 1, 2)).toBe(3)
  })

  it("fits fewer as the pictures grow", () => {
    expect(columnsFor(800, 0)).toBeGreaterThan(columnsFor(800, 3))
    expect(columnsFor(800, 3)).toBeGreaterThan(columnsFor(800, TILE_WIDTHS.length - 1))
  })

  it("always has a column, even before the picker is measured", () => {
    expect(columnsFor(0, 2)).toBe(1)
    expect(columnsFor(50, TILE_WIDTHS.length - 1)).toBe(1)
  })
})

describe("tileWidthIn", () => {
  it("shares what the gaps leave between the columns", () => {
    expect(tileWidthIn(100 * 3 + TILE_GAP * 2, 3)).toBe(100)
    expect(tileWidthIn(90, 1)).toBe(90)
  })
})

describe("intoColumns", () => {
  it("deals the items left to right, a row at a time", () => {
    expect(intoColumns([1, 2, 3, 4, 5, 6, 7], 3)).toEqual([
      [1, 4, 7],
      [2, 5],
      [3, 6],
    ])
  })

  it("leaves columns empty when there are fewer items", () => {
    expect(intoColumns(["a"], 3)).toEqual([["a"], [], []])
  })

  it("puts everything in one column when asked for none", () => {
    expect(intoColumns([1, 2], 0)).toEqual([[1, 2]])
  })
})

describe("ownShape", () => {
  it("is a picture's width over its height, kept from turning a long strip into a line", () => {
    expect(ownShape({ naturalWidth: 1920, naturalHeight: 1080 })).toBeCloseTo(16 / 9)
    expect(ownShape({ naturalWidth: 100, naturalHeight: 1000 })).toBe(0.4)
    expect(ownShape({ naturalWidth: 4000, naturalHeight: 100 })).toBe(3)
  })

  it("is null for a picture without a size", () => {
    expect(ownShape({ naturalWidth: 0, naturalHeight: 0 })).toBeNull()
  })
})

describe("lineEnd", () => {
  it("is the index after the last tile on the line holding a tile", () => {
    expect(lineEnd(0, 4)).toBe(4)
    expect(lineEnd(3, 4)).toBe(4)
    expect(lineEnd(4, 4)).toBe(8)
    expect(lineEnd(9, 4)).toBe(12)
  })

  it("puts every tile on a line of its own in one column", () => {
    expect(lineEnd(2, 1)).toBe(3)
    expect(lineEnd(2, 0)).toBe(3)
  })
})

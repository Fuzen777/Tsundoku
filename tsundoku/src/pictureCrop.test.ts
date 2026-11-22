import { describe, expect, it } from "vitest"
import { coverCrop, cropPlacement, keptInside, movedBy, sameCrop, zoomedTo, zoomOf } from "./pictureCrop"

const SQUARE = 1
const WIDE = 16 / 9

describe("coverCrop", () => {
  it("takes a wide picture's full height and its middle, for a circle", () => {
    expect(coverCrop(2, SQUARE)).toEqual({ x: 0.25, y: 0, width: 0.5, height: 1 })
  })

  it("takes a tall picture's full width and its middle", () => {
    expect(coverCrop(0.5, SQUARE)).toEqual({ x: 0, y: 0.25, width: 1, height: 0.5 })
  })

  it("keeps the frame's shape in the picture's pixels", () => {
    // A 4:5 post in a 16:9 frame: all of its width, and 9/16 of its width in height.
    const crop = coverCrop(0.8, WIDE)
    expect(crop.width).toBe(1)
    expect((crop.width * 0.8) / crop.height).toBeCloseTo(WIDE)
  })
})

describe("zooming", () => {
  it("zooms around the area's center, and says how far it is", () => {
    const zoomed = zoomedTo(coverCrop(1, SQUARE), 2, 1, SQUARE)
    expect(zoomed).toEqual({ x: 0.25, y: 0.25, width: 0.5, height: 0.5 })
    expect(zoomOf(zoomed, 1, SQUARE)).toBe(2)
  })

  it("stays between the whole picture and the most zoom, inside it", () => {
    const corner = { x: 0.5, y: 0.5, width: 0.5, height: 0.5 }
    expect(zoomedTo(corner, 0.2, 1, SQUARE)).toEqual({ x: 0, y: 0, width: 1, height: 1 })
    expect(zoomOf(zoomedTo(corner, 100, 1, SQUARE), 1, SQUARE)).toBe(8)
  })
})

describe("moving", () => {
  it("moves the area against the drag, kept inside the picture", () => {
    const crop = { x: 0.25, y: 0.25, width: 0.5, height: 0.5 }
    expect(movedBy(crop, 0.125, -0.25)).toEqual({ x: 0.1875, y: 0.375, width: 0.5, height: 0.5 })
    expect(movedBy(crop, 1, 1)).toEqual({ x: 0, y: 0, width: 0.5, height: 0.5 })
    expect(keptInside({ x: 0.9, y: -1, width: 0.5, height: 0.5 })).toEqual({ x: 0.5, y: 0, width: 0.5, height: 0.5 })
  })
})

describe("cropPlacement", () => {
  it("sizes and shifts the picture so the frame shows the area", () => {
    expect(cropPlacement({ x: 0.25, y: 0.5, width: 0.5, height: 0.25 })).toEqual({
      width: "200%",
      height: "400%",
      left: "-50%",
      top: "-200%",
    })
  })
})

it("tells areas apart past rounding", () => {
  const crop = { x: 0.1, y: 0.2, width: 0.3, height: 0.4 }
  expect(sameCrop(crop, { ...crop, x: 0.1 + 1e-6 })).toBe(true)
  expect(sameCrop(crop, { ...crop, x: 0.11 })).toBe(false)
})

import type { CSSProperties } from "react"
import type { PictureCrop } from "./bindings/PictureCrop"

/** How far in a picture can be zoomed: the frame then shows an eighth of what it shows at first. */
export const MOST_ZOOM = 8

/** A frame's shape, width over height: a channel's or an account's circle, or a playlist's 16:9 thumbnail. */
export function frameAspect(kind: "channel" | "playlist" | "account"): number {
  return kind === "playlist" ? 16 / 9 : 1
}

/** The area the frame shows at first: the whole picture's width or height, whichever fills the frame, centered, as
 * `object-fit: cover` shows it. `pictureAspect` is the picture's width over its height. */
export function coverCrop(pictureAspect: number, frame: number): PictureCrop {
  const { width, height } = coverSize(pictureAspect, frame)
  return { x: (1 - width) / 2, y: (1 - height) / 2, width, height }
}

/** The area's size at no zoom, as shares of the picture's width and height. */
function coverSize(pictureAspect: number, frame: number): { width: number; height: number } {
  return pictureAspect >= frame ? { width: frame / pictureAspect, height: 1 } : { width: 1, height: pictureAspect / frame }
}

/** How far in `crop` is zoomed: 1 for the whole picture's width or height. */
export function zoomOf(crop: PictureCrop, pictureAspect: number, frame: number): number {
  return coverSize(pictureAspect, frame).width / crop.width
}

/** The area zoomed to `zoom` (1 to `MOST_ZOOM`) around its center, kept inside the picture. */
export function zoomedTo(crop: PictureCrop, zoom: number, pictureAspect: number, frame: number): PictureCrop {
  const cover = coverSize(pictureAspect, frame)
  const clamped = Math.min(Math.max(zoom, 1), MOST_ZOOM)
  const width = cover.width / clamped
  const height = cover.height / clamped
  return keptInside({
    x: crop.x + crop.width / 2 - width / 2,
    y: crop.y + crop.height / 2 - height / 2,
    width,
    height,
  })
}

/** The area moved by a share of its own width and height (the frame's), kept inside the picture: a drag to the right
 * moves the picture right, so the area moves left. */
export function movedBy(crop: PictureCrop, right: number, down: number): PictureCrop {
  return keptInside({ ...crop, x: crop.x - right * crop.width, y: crop.y - down * crop.height })
}

/** The area with its edges moved in where it reaches past the picture. */
export function keptInside(crop: PictureCrop): PictureCrop {
  return {
    ...crop,
    x: Math.min(Math.max(crop.x, 0), Math.max(1 - crop.width, 0)),
    y: Math.min(Math.max(crop.y, 0), Math.max(1 - crop.height, 0)),
  }
}

/** Where a picture goes in its frame (`position: absolute`, the frame clipping it) so the frame shows `crop`: sized so
 * the area fills the frame, shifted by the area's edges, all as shares of the frame. */
export function cropPlacement(crop: PictureCrop): CSSProperties {
  return {
    width: `${100 / crop.width}%`,
    height: `${100 / crop.height}%`,
    left: `${(-crop.x / crop.width) * 100}%`,
    top: `${(-crop.y / crop.height) * 100}%`,
  }
}

/** Whether two areas are the same, to the 1/10,000th of the picture. */
export function sameCrop(first: PictureCrop, second: PictureCrop): boolean {
  const close = (a: number, b: number) => Math.abs(a - b) < 1e-4
  return (
    close(first.x, second.x) &&
    close(first.y, second.y) &&
    close(first.width, second.width) &&
    close(first.height, second.height)
  )
}

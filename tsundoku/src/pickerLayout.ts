import type { PickerLayout } from "./bindings/PickerLayout"
import type { PickerLayouts } from "./bindings/PickerLayouts"

/*
  How the pickers lay out what they offer (a listing's items, a post's files): List, Masonry or Gallery, and in the two
  views of pictures, how big the pictures are. Each picker's choice is kept in the settings (PickerLayouts).
*/

/** The narrowest a picture may be at each step of the size buttons, in pixels: a picker fits as many columns as it can
 * at that width, then shares what's left between them, so pictures fill the row. */
export const TILE_WIDTHS = [64, 88, 120, 160, 200, 256] as const

/** Below these widths, in pixels, a picture's corner labels shrink, then are left out, so a video's length doesn't
 * cover it. */
export const SMALL_TILE = 100
export const TINY_TILE = 80

/** The space between pictures, in pixels (`gap-2`). */
export const TILE_GAP = 8

/** The layouts until the settings are read, as the core starts them. */
export const DEFAULT_PICKER_LAYOUTS: PickerLayouts = {
  items: { view: "gallery", size: 2, hideDownloaded: false },
  postFiles: { view: "gallery", size: 1, hideDownloaded: false },
  queueSources: { view: "gallery", size: 2, hideDownloaded: false },
}

/** A size kept by an older or newer version, brought within this one's steps. */
export function sizeStep(size: number): number {
  if (!Number.isFinite(size)) return 0
  return Math.min(Math.max(Math.round(size), 0), TILE_WIDTHS.length - 1)
}

/** The layout one step smaller or larger, or null at the end of the steps. */
export function resized(layout: PickerLayout, by: 1 | -1): PickerLayout | null {
  const size = sizeStep(layout.size) + by
  return size < 0 || size >= TILE_WIDTHS.length ? null : { ...layout, size }
}

/** How many columns of pictures fit in `width` pixels at the layout's size: at least one. */
export function columnsFor(width: number, size: number): number {
  const tile = TILE_WIDTHS[sizeStep(size)]
  return Math.max(1, Math.floor((width + TILE_GAP) / (tile + TILE_GAP)))
}

/** How wide each of `columns` pictures is in `width` pixels: what's left after the gaps, shared. */
export function tileWidthIn(width: number, columns: number): number {
  return (width - TILE_GAP * (columns - 1)) / columns
}

/** Items dealt into `count` columns, left to right, a row at a time, so the first ones stay at the top of the picker
 * and each column keeps their order. */
export function intoColumns<Item>(items: readonly Item[], count: number): Item[][] {
  const columns: Item[][] = Array.from({ length: Math.max(1, count) }, () => [])
  items.forEach((item, index) => columns[index % columns.length].push(item))
  return columns
}

/** The narrowest and widest a picture's own shape is shown in Masonry, so a long strip stays a picture. */
const NARROWEST = 0.4
const WIDEST = 3

/** A loaded picture's own shape, as width over height, within what Masonry shows, or null when it has no size. */
export function ownShape(picture: { naturalWidth: number; naturalHeight: number }): number | null {
  const { naturalWidth, naturalHeight } = picture
  return naturalWidth > 0 && naturalHeight > 0 ? Math.min(Math.max(naturalWidth / naturalHeight, NARROWEST), WIDEST) : null
}

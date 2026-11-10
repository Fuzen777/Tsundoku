import type { ListedItem } from "./bindings/ListedItem"
import type { Listing } from "./bindings/Listing"
import type { QueuePick } from "./bindings/QueuePick"
import { strings } from "./strings"

/*
  What the picker of a channel, playlist or account ticks, and what Download Selected queues. Items are told apart by
  their site and ID, as the history tells videos apart.
*/

/** An item's key in the selection. */
export function itemKey({ video }: ListedItem): string {
  return `${video.site} ${video.videoId}`
}

/** Whether the item is in the queue already: it can't be ticked, since it's on its way. */
export function isQueued(item: ListedItem): boolean {
  return item.inQueue
}

/** Whether the history or the queue has the item: it's greyed and starts unticked. */
export function isKnown(item: ListedItem): boolean {
  return item.lastDownload !== null || item.inQueue
}

/** The items ticked to start with: the new ones. */
export function initialSelection(items: readonly ListedItem[]): Set<string> {
  return new Set(items.filter((item) => !isKnown(item)).map(itemKey))
}

/** Every item that can be ticked, downloaded ones included, or none. */
export function selectEvery(items: readonly ListedItem[], ticked: boolean): Set<string> {
  return new Set(ticked ? items.filter((item) => !isQueued(item)).map(itemKey) : [])
}

/** The selection with one item ticked or unticked. */
export function withItem(selection: ReadonlySet<string>, item: ListedItem, ticked: boolean): Set<string> {
  const next = new Set(selection)
  if (ticked && !isQueued(item)) next.add(itemKey(item))
  else next.delete(itemKey(item))
  return next
}

/** The ticked items, in the listing's order. One in the queue since it was ticked is left out. */
export function selectedItems(items: readonly ListedItem[], selection: ReadonlySet<string>): ListedItem[] {
  return items.filter((item) => !isQueued(item) && selection.has(itemKey(item)))
}

/** Whether the item downloads with a preset: a video, rather than a post or a file host's files. */
export function takesPreset({ video }: ListedItem): boolean {
  return !video.post && !video.files
}

/** A preset as the picks need it. */
export interface PickedPreset {
  id: string
  name: string
}

/**
 * What Download Selected queues, in the listing's order: each ticked video with the preset it starts with
 * (`presetFor`), a post without one, and whether the download replaces a file made before with the same preset. Null
 * while a ticked video's preset isn't known yet.
 */
export function picksFor(
  items: readonly ListedItem[],
  selection: ReadonlySet<string>,
  presetFor: (item: ListedItem) => PickedPreset | null,
): QueuePick[] | null {
  const picks: QueuePick[] = []
  for (const item of selectedItems(items, selection)) {
    if (!takesPreset(item)) {
      picks.push({ video: item.video, presetId: null, overwrite: false, folder: null })
      continue
    }
    const preset = presetFor(item)
    if (!preset) return null
    picks.push({
      video: item.video,
      presetId: preset.id,
      overwrite: item.lastDownload?.preset === preset.name,
      folder: null,
    })
  }
  return picks
}

/** What a listing's items are called: an account's posts, YouTube's videos, anything else's items. */
export function nounOf(listing: Pick<Listing, "kind" | "siteName">): keyof typeof strings.picker.nouns {
  if (listing.kind === "account") return "post"
  return listing.kind === "channel" || listing.siteName === "youtube.com" ? "video" : "item"
}

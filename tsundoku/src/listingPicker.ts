import type { ItemKey } from "./bindings/ItemKey"
import type { ListedItem } from "./bindings/ListedItem"
import type { Listing } from "./bindings/Listing"
import type { ListingKind } from "./bindings/ListingKind"
import type { ListingPace } from "./bindings/ListingPace"
import type { QueuePick } from "./bindings/QueuePick"
import type { QueueSource } from "./bindings/QueueSource"
import type { VideoInfo } from "./bindings/VideoInfo"
import { strings } from "./strings"

/*
  What the picker of a channel, playlist or account ticks, and what Download Selected queues. Items are told apart by
  their site and ID, as the history tells videos apart.
*/

/** An item's key in the selection. */
export function itemKey({ video }: ListedItem): string {
  return `${video.site} ${video.videoId}`
}

/** Whether the item is in the queue already: it's on its way. */
export function isQueued(item: ListedItem): boolean {
  return item.inQueue
}

/** Whether the item is a Fanbox post for a plan the account doesn't support: nothing of it can be downloaded. */
export function isLocked(item: ListedItem): boolean {
  return item.video.fanbox?.locked === true
}

/** Whether the item can't be ticked: it's in the queue already, or locked behind a plan. */
export function cannotTick(item: ListedItem): boolean {
  return isQueued(item) || isLocked(item)
}

/** Whether the history or the queue has the item: it's greyed and starts unticked. */
export function isKnown(item: ListedItem): boolean {
  return item.lastDownload !== null || item.inQueue
}

/** An item key of the core's (`ItemKey`) as the picker keys items (`itemKey`). */
export function keyOf({ site, videoId }: ItemKey): string {
  return `${site} ${videoId}`
}

/** Whether an item starts ticked: new (neither the history nor the queue has it) and not skipped the last time its
 * listing was downloaded from (`skipped`, by `itemKey`). */
export function startsTicked(item: ListedItem, skipped: ReadonlySet<string>): boolean {
  return !isKnown(item) && !skipped.has(itemKey(item))
}

/** The items ticked to start with: the new ones, but those skipped before. */
export function initialSelection(items: readonly ListedItem[], skipped: ReadonlySet<string> = NONE): Set<string> {
  return new Set(items.filter((item) => startsTicked(item, skipped)).map(itemKey))
}

const NONE: ReadonlySet<string> = new Set()

/** The selection with every item of `items` that can be ticked (downloaded ones included) ticked, or none of them;
 * the ticks of items not among them, such as those another type hides, stay. */
export function selectEvery(
  selection: ReadonlySet<string>,
  items: readonly ListedItem[],
  ticked: boolean,
): Set<string> {
  const next = new Set(selection)
  for (const item of items) {
    if (ticked && !cannotTick(item)) next.add(itemKey(item))
    else next.delete(itemKey(item))
  }
  return next
}

/** The selection with one item ticked or unticked. */
export function withItem(selection: ReadonlySet<string>, item: ListedItem, ticked: boolean): Set<string> {
  const next = new Set(selection)
  if (ticked && !cannotTick(item)) next.add(itemKey(item))
  else next.delete(itemKey(item))
  return next
}

/** The ticked items, in the listing's order. One in the queue since it was ticked is left out. */
export function selectedItems(items: readonly ListedItem[], selection: ReadonlySet<string>): ListedItem[] {
  return items.filter((item) => !cannotTick(item) && selection.has(itemKey(item)))
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

/** How many new items Load New lists on to: as many as Load More's. */
export const LOAD_NEW_COUNT = 50

/** The first of `items` that's new (neither the history nor the queue has it) and wasn't among `before`'s keys: where
 * the picker scrolls once Load New has listed on past what's known. */
export function firstNewSince(items: readonly ListedItem[], before: ReadonlySet<string>): ListedItem | undefined {
  return items.find((item) => !isKnown(item) && !before.has(itemKey(item)))
}

/** A picker's items and which of them are ticked, kept together so items loaded later can be ticked as they arrive,
 * with the tick changes made by hand, the latest last, for Undo. */
export interface PickerItems {
  items: readonly ListedItem[]
  selection: ReadonlySet<string>
  undo: readonly TickChange[]
}

/** One change of ticks made by hand (a box, the box at the top), by the items' keys. */
export interface TickChange {
  ticked: readonly string[]
  unticked: readonly string[]
}

/** The items to start with, the new ones ticked but those skipped before. */
export function pickerItems(items: readonly ListedItem[], skipped: ReadonlySet<string> = NONE): PickerItems {
  return { items, selection: initialSelection(items, skipped), undo: [] }
}

/** The picker with the ticks changed by hand to `selection`, the change kept for Undo; the same picker when nothing
 * changed. */
export function withTicks(current: PickerItems, selection: ReadonlySet<string>): PickerItems {
  const ticked = [...selection].filter((key) => !current.selection.has(key))
  const unticked = [...current.selection].filter((key) => !selection.has(key))
  if (ticked.length === 0 && unticked.length === 0) return current
  return { ...current, selection, undo: [...current.undo, { ticked, unticked }] }
}

/** The picker with its latest change of ticks undone, or the same picker when there's none. */
export function undone(current: PickerItems): PickerItems {
  const last = current.undo.at(-1)
  if (!last) return current
  const selection = new Set(current.selection)
  for (const key of last.ticked) selection.delete(key)
  for (const key of last.unticked) selection.add(key)
  return { ...current, selection, undo: current.undo.slice(0, -1) }
}

/** What a picker had ticked, and its changes for Undo, kept to open again with: the keys it listed, and those of them
 * ticked. */
export interface KeptTicks {
  listed: ReadonlySet<string>
  selection: ReadonlySet<string>
  undo: readonly TickChange[]
}

export function keptTicks({ items, selection, undo }: PickerItems): KeptTicks {
  return { listed: new Set(items.map(itemKey)), selection, undo }
}

/** A picker opened again with the ticks it had: an item it listed is ticked as it was, one listed since as a new
 * picker's would be, and its changes can still be undone. */
export function withKeptTicks(current: PickerItems, kept: KeptTicks): PickerItems {
  const selection = new Set<string>()
  for (const item of current.items) {
    const key = itemKey(item)
    if (kept.listed.has(key) ? kept.selection.has(key) : current.selection.has(key)) selection.add(key)
  }
  return { ...current, selection, undo: kept.undo }
}

/**
 * The picker with items loaded later (Load More, Load All) added: each kept once, the first time it's met, since a read
 * may start with items listed before. An account's posts are put in date order with the others, newest first, since a
 * pinned post among the first ones may be older than those loaded. The ticks made stay; an added item that's new is
 * ticked, as the first ones were (but one skipped before), since Selected shows only what's ticked (`splitByTicks`):
 * one left unticked would go straight to Discarded.
 */
export function withMoreItems(
  current: PickerItems,
  more: readonly ListedItem[],
  kind: ListingKind,
  skipped: ReadonlySet<string> = NONE,
): PickerItems {
  const known = new Set(current.items.map(itemKey))
  const added = more.filter((item) => {
    const key = itemKey(item)
    if (known.has(key)) return false
    known.add(key)
    return true
  })
  if (added.length === 0) return current
  const merged = [...current.items, ...added]
  const ticked = added.filter((item) => startsTicked(item, skipped)).map(itemKey)
  return {
    ...current,
    items: kind === "account" ? newestFirst(merged) : merged,
    selection: ticked.length > 0 ? new Set([...current.selection, ...ticked]) : current.selection,
  }
}

/** The picker once these items, by their keys, joined the queue (Download and Load More): In Downloads, and no longer
 * ticked. */
export function withQueued(current: PickerItems, keys: ReadonlySet<string>): PickerItems {
  const selection = new Set(current.selection)
  for (const key of keys) selection.delete(key)
  return {
    ...current,
    items: current.items.map((item) => (keys.has(itemKey(item)) ? { ...item, inQueue: true } : item)),
    selection,
  }
}

/**
 * The items the picker lists, in their order, split in two: what will download (ticked, its Selected tab), and what
 * won't (unticked, downloaded before or in Downloads: Discarded). An item moves the moment its box changes.
 */
export function splitByTicks<Item extends ListedItem>(
  items: readonly Item[],
  selection: ReadonlySet<string>,
): { ticked: Item[]; leftOut: Item[] } {
  const ticked: Item[] = []
  const leftOut: Item[] = []
  for (const item of items) {
    if (selection.has(itemKey(item)) && !cannotTick(item)) ticked.push(item)
    else leftOut.push(item)
  }
  return { ticked, leftOut }
}

/** What Download records as skipped for the listing (`skipListingItems`): the items discarded that are new, those the
 * history and the queue don't have, as a watched source skips its new items only. */
export function discardedKeys(discarded: readonly ListedItem[]): ItemKey[] {
  return discarded
    .filter((item) => !isKnown(item) && !cannotTick(item))
    .map(({ video }) => ({ site: video.site, videoId: video.videoId }))
}

/** What's skipped once Download took `taken` and discarded `discarded`: those taken leave, those discarded join, each
 * once. */
export function withSkipped(
  current: readonly ItemKey[],
  discarded: readonly ItemKey[],
  taken: readonly ItemKey[],
): ItemKey[] {
  const leaving = new Set([...taken, ...discarded].map(keyOf))
  return [...discarded, ...current.filter((key) => !leaving.has(keyOf(key)))]
}

/** Items by their day, newest first, those without one last; items of one day keep their order. */
function newestFirst(items: readonly ListedItem[]): ListedItem[] {
  return items
    .map((item, index) => ({ item, index }))
    .sort((first, second) => {
      const [a, b] = [first.item.video.uploadDate, second.item.video.uploadDate]
      if (a !== b) {
        if (a === null) return 1
        if (b === null) return -1
        return a < b ? 1 : -1
      }
      return first.index - second.index
    })
    .map(({ item }) => item)
}

/** The items Hide Downloaded leaves: all of them, or with it, those neither the history nor the queue has. */
export function shownItems(items: readonly ListedItem[], hideDownloaded: boolean): readonly ListedItem[] {
  return hideDownloaded ? items.filter((item) => !isKnown(item)) : items
}

/**
 * What an item holds, for the pickers' type filter: a post of images only, videos (a video, or a post of videos only),
 * a post of both, or a post of other files (a .zip). A post of other files and images or videos counts as mixed, and a
 * post whose files aren't known as images, as its picture shows it.
 */
export type ItemType = "images" | "videos" | "mixed" | "files"

/** A type, or every one. */
export type TypeFilter = ItemType | "all"

/** The types in the order the filter offers them. */
export const ITEM_TYPES: readonly ItemType[] = ["images", "videos", "mixed", "files"]

/** What the type filter reads of an item. */
type Typed = { video: Pick<VideoInfo, "post" | "files"> }

export function itemType({ video }: Typed): ItemType {
  const { post } = video
  if (!post) return video.files ? "files" : "videos"
  const held: ItemType[] = []
  if (post.images > 0) held.push("images")
  if (post.videos > 0) held.push("videos")
  if (post.others > 0) held.push("files")
  if (held.length > 1) return "mixed"
  return held[0] ?? "images"
}

/** How many items are of one type. */
export interface TypeCount {
  type: ItemType
  count: number
}

/** How many of the items are of each type among them, in the filter's order. */
export function typeCounts(items: readonly Typed[]): TypeCount[] {
  const counts = new Map<ItemType, number>()
  for (const item of items) {
    const type = itemType(item)
    counts.set(type, (counts.get(type) ?? 0) + 1)
  }
  return ITEM_TYPES.flatMap((type) => {
    const count = counts.get(type)
    return count === undefined ? [] : [{ type, count }]
  })
}

/** The filter in effect: the type chosen while the items are of several types and some are of it, else every type. */
export function typeInEffect(chosen: TypeFilter, counts: readonly TypeCount[]): TypeFilter {
  return counts.length > 1 && counts.some(({ type }) => type === chosen) ? chosen : "all"
}

/** The items of the type, or all of them. */
export function ofType<Item extends Typed>(items: readonly Item[], filter: TypeFilter): readonly Item[] {
  return filter === "all" ? items : items.filter((item) => itemType(item) === filter)
}

/** How many files the items hold: a post's images, videos and other files; one for a video, or for a post whose files
 * aren't known. */
export function filesIn(items: readonly Typed[]): number {
  return items.reduce((files, { video: { post } }) => {
    return files + (post ? Math.max(post.images + post.videos + post.others, 1) : 1)
  }, 0)
}

/** An X account's photos and videos: how many the site counts (`Listing.mediaTotal`), and how many the posts listed
 * so far hold. */
export interface MediaCount {
  total: number
  listed: number
}

/**
 * What Load All says of an account that counts its photos and videos rather than its posts (X): the site's count
 * against the files in the posts listed so far. Null for a listing without such a count, or once how many posts it
 * holds is known (`postTotal`), which says more.
 */
export function mediaCount(
  listing: Pick<Listing, "mediaTotal">,
  items: readonly Typed[],
  postTotal: number | null,
): MediaCount | null {
  if (listing.mediaTotal === null || postTotal !== null) return null
  return { total: listing.mediaTotal, listed: filesIn(items) }
}

/**
 * How many minutes listing the rest of an account takes at most when its site lists only so far before making the read
 * wait (X: `Listing.pace`), from the site's count of its photos and videos: a wait for each stretch the rest fills, the
 * reading itself being seconds. At most, since the site counts files, of which a post may hold several, and counts
 * every one the account ever posted, while its lists may end sooner (NASA's photos after 470 posts of some 28,000
 * files). Null when the rest fits in a stretch, or the site gives no such count.
 */
export function restEstimate(pace: ListingPace | null, media: MediaCount | null): number | null {
  if (pace === null || media === null || pace.items <= 0) return null
  const waits = Math.floor(Math.max(media.total - media.listed, 0) / pace.items)
  return waits === 0 ? null : Math.round((waits * pace.pauseSeconds) / 60)
}

/** What a listing's items are called: an account's posts, YouTube's videos, anything else's items. */
/** Whether a listing's items name who posted them: a playlist's, and a list of several artists' works (Pixiv's
 * following feed or a tag's), whose posters differ. An account's or a channel's are all its own. */
export function showsUploaders(listing: Pick<Listing, "kind" | "items">): boolean {
  if (listing.kind === "playlist") return true
  const uploaders = new Set(listing.items.map(({ video }) => video.uploader ?? ""))
  return uploaders.size > 1
}

export function nounOf(listing: Pick<Listing, "kind" | "siteName">): keyof typeof strings.picker.nouns {
  if (listing.kind === "account") return "post"
  return listing.kind === "channel" || listing.siteName === "youtube.com" ? "video" : "item"
}

/** What Download Selected's picks come from, which the Queue shows them together under. */
export function sourceOf(listing: Listing): QueueSource {
  return {
    url: listing.url,
    kind: listing.kind,
    name: listing.title,
    siteName: listing.siteName,
    thumbnailUrl: listing.thumbnailUrl,
  }
}

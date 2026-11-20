import { useEffect, useRef, useState } from "react"
import { Film, Images, Play, X } from "lucide-react"
import { addManyToQueue, cancelLoadingMore, confirmWithUser, countListing, loadMoreOfListing } from "../api"
import type { ListedItem } from "../bindings/ListedItem"
import type { Listing } from "../bindings/Listing"
import type { ListingMore } from "../bindings/ListingMore"
import type { MoreRead } from "../bindings/MoreRead"
import type { PickerLayout } from "../bindings/PickerLayout"
import { describeDownloadError, describePreviewError, isCancelled, type ErrorDescription } from "../errors"
import {
  formatCalendarDate,
  formatContents,
  formatDay,
  formatDuration,
  formatRoughAge,
  joinDetails,
} from "../format"
import {
  firstNewSince,
  isKnown,
  isQueued,
  itemKey,
  LOAD_NEW_COUNT,
  mediaCount,
  nounOf,
  pickerItems,
  picksFor,
  selectedItems,
  selectEvery,
  shownItems,
  sourceOf,
  takesPreset,
  withItem,
  withMoreItems,
  type MediaCount,
  type PickedPreset,
} from "../listingPicker"
import { usePickerLayout, usePresets, useRulePresets } from "../queries"
import { scrollWithin } from "../scrollWithin"
import { strings } from "../strings"
import { useTypeFilter } from "../useTypeFilter"
import { Button } from "./Button"
import { SelectInput } from "./Field"
import { IconButton } from "./IconButton"
import { ItemTypeFilter } from "./ItemTypeFilter"
import { MediaTiles, TileLabel, type MediaTile } from "./MediaTiles"
import { PickerViewControls } from "./PickerViewControls"
import { ToggleButton } from "./SegmentedControl"
import { StatusMessage } from "./StatusMessage"
import { classes } from "./styles"
import { Thumbnail } from "./Thumbnail"
import { WatchForm } from "./WatchForm"

interface ListingPickerProps {
  listing: Listing
  onClose: () => void
  /** The picked items joined the queue. */
  onAdded: () => void
  /** It's watched now: the card closes, and the Watched view opens. */
  onWatched: () => void
}

/** What the picker is loading: the next items (Load More), on until enough new ones (Load New) or all the rest (Load
 * All). */
type Loading = MoreRead["kind"]

/** Where Load All's count is, when the first read didn't say: another read counting it (a YouTube channel's videos), or
 * that read failed. */
type Counted = "counting" | "failed" | null

/**
 * The preview card for a channel, playlist or account: what it is, its items with a box each (the new ones ticked,
 * those downloaded before or in the queue greyed, or hidden with Hide Downloaded), a type filter when they're of
 * several types (only the images of an account, say), Load More, Load New and Load All when only the newest were
 * listed, and Download Selected, which queues each ticked item shown as its own download. A video starts with the
 * preset a rule picks for it, else the one picked here.
 */
export function ListingPicker({ listing, onClose, onAdded, onWatched }: ListingPickerProps) {
  const [{ items, selection }, setPicker] = useState(() => pickerItems(listing.items))
  const [more, setMore] = useState<ListingMore | null>(listing.more)
  const [total, setTotal] = useState(listing.total)
  const [counted, setCounted] = useState<Counted>(() =>
    listing.total === null && listing.more?.channelId ? "counting" : null,
  )
  const [loading, setLoading] = useState<Loading | null>(null)
  const [stopping, setStopping] = useState(false)
  const [loadError, setLoadError] = useState<ErrorDescription | null>(null)
  const { data: presets } = usePresets()
  const rulePresets = useRulePresets(items.map((item) => item.video))
  const [chosenId, setChosenId] = useState<string | null>(null)
  const [adding, setAdding] = useState(false)
  const [error, setError] = useState<ErrorDescription | null>(null)
  const [watching, setWatching] = useState(false)
  const { layout, change: changeLayout } = usePickerLayout("items")
  const presetId = chosenId ?? presets?.defaultId ?? null
  // Batches that arrive after the card closed are dropped; closing it stops the load.
  const open = useRef(true)
  // Once Load New ends, the items listed before it, so the list can scroll to the first new one after them.
  const listedBeforeNew = useRef<ReadonlySet<string> | null>(null)
  const list = useRef<HTMLDivElement>(null)
  useEffect(() => {
    open.current = true
    return () => {
      open.current = false
      void cancelLoadingMore()
    }
  }, [])

  // A YouTube channel's Videos tab doesn't say how many videos it holds: another read counts them, for Load All to say.
  // Closing the card stops it.
  useEffect(() => {
    const from = listing.more
    if (listing.total !== null || !from?.channelId) return
    let current = true
    countListing(from).then(
      (count) => {
        if (!current) return
        // Unless Load All reached the end meanwhile, which says.
        if (count !== null) setTotal((total) => total ?? count)
        setCounted(null)
      },
      (caught: unknown) => {
        if (current && !isCancelled(caught)) setCounted("failed")
      },
    )
    return () => {
      current = false
    }
  }, [listing])

  const setSelection = (update: (selection: ReadonlySet<string>) => ReadonlySet<string>) =>
    setPicker((current) => ({ ...current, selection: update(current.selection) }))
  const types = useTypeFilter(shownItems(items, layout.hideDownloaded))
  const shown = types.shown
  const hidden = items.length - shown.length
  const picked = selectedItems(shown, selection)
  const selectable = shown.filter((item) => !isQueued(item)).length
  const allSelected = selectable > 0 && picked.length === selectable
  const hasVideos = shown.some(takesPreset)
  const placeOf = new Map(items.map((item, index) => [itemKey(item), index]))
  const presetFor = (item: ListedItem): PickedPreset | null => {
    const byRule = rulePresets[placeOf.get(itemKey(item)) ?? -1]
    // Not known yet.
    if (byRule === undefined) return null
    const preset = presets?.presets.find((candidate) => candidate.id === (byRule?.presetId ?? presetId))
    return preset ? { id: preset.id, name: preset.name } : null
  }
  const picks = presets ? picksFor(shown, selection, presetFor) : null

  // After Load New, the list scrolls to the first new item it listed, past the known ones before it. Its last batch may
  // come just after it ends, so until one is found it keeps looking.
  useEffect(() => {
    const before = listedBeforeNew.current
    if (loading !== null || !before) return
    const first = firstNewSince(shown, before)
    const tile = first && list.current?.querySelector<HTMLElement>(`[data-item="${CSS.escape(itemKey(first))}"]`)
    if (!tile) return
    listedBeforeNew.current = null
    scrollWithin(tile, "start", "smooth")
  }, [loading, shown])

  /** Lists the next items, on until enough new ones, or all the rest, adding each batch as it comes; new ones are
   * ticked if anything is. */
  async function load(read: MoreRead) {
    if (!more || loading) return
    setLoading(read.kind)
    setLoadError(null)
    const tickNew = selection.size > 0
    listedBeforeNew.current = read.kind === "new" ? new Set(items.map(itemKey)) : null
    try {
      await loadMoreOfListing(more, read, (batch) => {
        if (!open.current) return
        setPicker((current) => withMoreItems(current, batch.items, listing.kind, tickNew))
        setMore(batch.more)
        if (batch.total !== null) setTotal(batch.total)
      })
    } catch (caught) {
      if (open.current && !isCancelled(caught)) setLoadError(describePreviewError(caught))
    } finally {
      if (open.current) {
        setLoading(null)
        setStopping(false)
      }
    }
  }

  function stop() {
    setStopping(true)
    void cancelLoadingMore()
  }

  async function download() {
    if (!picks || picks.length === 0) return
    const replacing = picks.filter((pick) => pick.overwrite).length
    if (
      replacing > 0 &&
      !(await confirmWithUser({
        title: strings.pending.overwriteTitle,
        message: strings.pending.overwriteMessage(replacing),
        confirmLabel: strings.pending.overwriteConfirm,
        cancelLabel: strings.dialogs.cancel,
      }))
    ) {
      return
    }
    setAdding(true)
    setError(null)
    try {
      await addManyToQueue(picks, sourceOf(listing))
      onAdded()
    } catch (caught) {
      setError(describeDownloadError(caught))
      setAdding(false)
    }
  }

  const noun = strings.picker.nouns[nounOf(listing)]
  const loadRow = (more !== null || loading !== null || loadError !== null) && (
    <LoadRow
      loading={loading}
      stopping={stopping}
      canLoad={more !== null}
      count={items.length}
      fresh={items.filter((item) => !isKnown(item)).length}
      total={total}
      media={mediaCount(listing, items, total)}
      counted={total === null ? counted : null}
      noun={noun}
      slowly={listing.kind === "account" && listing.siteName === "instagram.com"}
      error={loadError}
      disabled={adding}
      onLoad={(read) => void load(read)}
      onStop={stop}
    />
  )
  return (
    <>
      <div className="flex gap-4">
        <ListingPicture listing={listing} />
        <div className="flex min-w-0 flex-1 flex-col gap-1">
          <p className="line-clamp-2 font-semibold select-text" title={listing.title}>
            {listing.title}
          </p>
          <p className="truncate text-fg-secondary">{joinDetails([listing.uploader, listing.siteName])}</p>
          <p className="text-xs text-fg-secondary tabular-nums">
            {joinDetails([
              strings.picker.kinds[listing.kind],
              strings.picker.listed(items.length, noun, listing.newestOnly && more !== null, total),
            ])}
          </p>
        </div>
        <IconButton label={strings.preview.close} icon={X} onClick={onClose} />
      </div>
      {items.length === 0 ? (
        <p className="text-fg-secondary">{strings.picker.empty}</p>
      ) : (
        <>
          <div className="flex flex-wrap items-center justify-between gap-x-3 gap-y-2">
            <label className="flex items-center gap-2 text-fg-secondary">
              <input
                type="checkbox"
                className="size-4 accent-accent"
                aria-label={strings.picker.selectAll}
                title={strings.picker.selectAll}
                checked={allSelected}
                ref={(box) => {
                  if (box) box.indeterminate = picked.length > 0 && !allSelected
                }}
                disabled={adding || selectable === 0}
                onChange={(event) => {
                  const ticked = event.target.checked
                  setSelection((current) => selectEvery(current, shown, ticked))
                }}
              />
              <span className="whitespace-nowrap tabular-nums">
                {joinDetails([
                  strings.picker.selected(picked.length, shown.length),
                  hidden > 0 && strings.picker.hidden(hidden),
                ])}
              </span>
            </label>
            <div className="flex flex-wrap items-center justify-end gap-2">
              <ItemTypeFilter counts={types.counts} value={types.type} onChange={types.choose} disabled={adding} />
              <ToggleButton
                label={strings.picker.hideDownloaded}
                hint={strings.picker.hideDownloadedHint}
                pressed={layout.hideDownloaded}
                onChange={(hideDownloaded) => changeLayout((latest) => ({ ...latest, hideDownloaded }))}
              />
              <PickerViewControls layout={layout} onChange={changeLayout} />
            </div>
          </div>
          <div ref={list}>
            {shown.length === 0 ? (
              <p className="text-fg-secondary">{strings.picker.allHidden}</p>
            ) : (
              <ListedItems
                items={shown}
                layout={layout}
                showUploader={listing.kind === "playlist"}
                isSelected={(item) => selection.has(itemKey(item))}
                disabled={adding}
                onSelect={(item, ticked) => setSelection((current) => withItem(current, item, ticked))}
              />
            )}
          </div>
          {loadRow}
          {watching ? (
            <WatchForm listing={{ ...listing, items: [...items] }} onCancel={() => setWatching(false)} onWatched={onWatched} />
          ) : (
            <div className="flex flex-col items-end gap-2">
              {error && <StatusMessage tone="danger" {...error} />}
              <div className="flex w-full items-center justify-end gap-2">
                <Button
                  className="mr-auto"
                  disabled={adding}
                  title={strings.watched.watchHint}
                  onClick={() => setWatching(true)}
                >
                  {strings.watched.watch}
                </Button>
                {hasVideos && (
                  <label
                    className="flex items-center gap-2 text-fg-secondary"
                    title={strings.picker.videosStartAsHint}
                  >
                    {strings.picker.videosStartAs}
                    <SelectInput
                      value={presetId ?? ""}
                      disabled={!presets || adding}
                      onChange={(event) => setChosenId(event.target.value)}
                    >
                      {presets?.presets.map((preset) => (
                        <option key={preset.id} value={preset.id}>
                          {preset.name}
                        </option>
                      ))}
                    </SelectInput>
                  </label>
                )}
                <Button
                  variant="primary"
                  loading={adding}
                  disabled={!picks || picks.length === 0}
                  onClick={() => void download()}
                >
                  {strings.picker.downloadSelected(picked.length)}
                </Button>
              </div>
            </div>
          )}
        </>
      )}
    </>
  )
}

interface LoadRowProps {
  loading: Loading | null
  stopping: boolean
  /** There's more to list. */
  canLoad: boolean
  /** How many are listed so far, how many of them the history and the queue don't have, and how many there are in all
   * when the site says. */
  count: number
  fresh: number
  total: number | null
  /** Without a total, on an account whose site counts its photos and videos instead (X): that count, and how many of
   * them the posts listed so far hold. */
  media: MediaCount | null
  /** Without a total, whether another read is counting it, or couldn't. */
  counted: Counted
  noun: readonly [string, string]
  /** Instagram: Load All goes a page at a time, slowly, which a line under it says. */
  slowly: boolean
  error: ErrorDescription | null
  disabled: boolean
  onLoad: (read: MoreRead) => void
  onStop: () => void
}

/**
 * Under the items: Load More, Load New and Load All (with how many there are, when the site says or another read
 * counted them, or on X how many photos and videos the site counts), or while Load New or Load All lists them, how many
 * are listed so far and how many are new, and Stop. What went wrong shows over them, and they stay to try again.
 */
function LoadRow({
  loading,
  stopping,
  canLoad,
  count,
  fresh,
  total,
  media,
  counted,
  noun,
  slowly,
  error,
  disabled,
  onLoad,
  onStop,
}: LoadRowProps) {
  const text = strings.picker
  return (
    <div className="flex flex-col items-center gap-2">
      {error && <StatusMessage tone="danger" {...error} />}
      {loading === "all" || loading === "new" ? (
        <>
          <div className="flex items-center gap-3">
            <p className="text-fg-secondary tabular-nums" role="status">
              {text.loadingAll(count, total, noun, fresh, media)}
            </p>
            <Button size="sm" disabled={stopping} onClick={onStop}>
              {text.stopLoading}
            </Button>
          </div>
          {slowly && <p className="text-center text-xs text-fg-secondary">{text.loadingSlowly}</p>}
        </>
      ) : (
        canLoad && (
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              loading={loading === "next"}
              disabled={disabled}
              title={text.loadMoreHint(noun)}
              onClick={() => onLoad({ kind: "next" })}
            >
              {text.loadMore}
            </Button>
            <Button
              size="sm"
              disabled={disabled || loading !== null}
              title={text.loadNewHint(LOAD_NEW_COUNT, noun)}
              onClick={() => onLoad({ kind: "new", count: LOAD_NEW_COUNT })}
            >
              {text.loadNew(LOAD_NEW_COUNT)}
            </Button>
            <Button
              size="sm"
              disabled={disabled || loading !== null}
              title={text.loadAllHint(count, total, noun, counted, media)}
              onClick={() => onLoad({ kind: "all" })}
            >
              {text.loadAll(total, media?.total ?? null)}
            </Button>
          </div>
        )
      )}
    </div>
  )
}

/** A channel's or an account's picture, round, or a playlist's first thumbnail; `small` for a watched source's card. */
export function ListingPicture({
  listing,
  small = false,
}: {
  listing: Pick<Listing, "kind" | "thumbnailUrl">
  small?: boolean
}) {
  const [failed, setFailed] = useState(false)
  if (listing.kind === "playlist") {
    return <Thumbnail key={listing.thumbnailUrl} url={listing.thumbnailUrl} className={small ? "w-24" : "w-44"} />
  }
  return (
    <div className={classes("shrink-0 overflow-hidden rounded-full bg-input", small ? "size-14" : "size-24")}>
      {listing.thumbnailUrl && !failed && (
        <img
          src={listing.thumbnailUrl}
          alt=""
          referrerPolicy="no-referrer"
          className="size-full object-cover"
          onError={() => setFailed(true)}
        />
      )}
    </div>
  )
}

interface ListedRowProps {
  item: ListedItem
  /** A playlist's items may come from several uploaders; a channel's or an account's all come from it. */
  showUploader: boolean
  /** The watched source that found it, in All New Items, first in its details. */
  source?: string | null
  selected: boolean
  disabled: boolean
  onSelect: (ticked: boolean) => void
}

/**
 * One item of the picker: its box, thumbnail, title, and "date · length" or what a post holds. Greyed when the history
 * or the queue has it, with Downloaded or In Queue. Rows off screen aren't laid out (`content-visibility`), so a
 * playlist of hundreds scrolls smoothly.
 */
export function ListedRow({ item, showUploader, source = null, selected, disabled, onSelect }: ListedRowProps) {
  const { video, lastDownload } = item
  const known = isKnown(item)
  const queued = isQueued(item)
  return (
    <li data-item={itemKey(item)} className="[contain-intrinsic-size:auto_4.25rem] [content-visibility:auto]">
      <label
        className={classes(
          "flex items-center gap-3 rounded-md px-2 py-1.5 transition-colors",
          queued ? "cursor-default" : "cursor-pointer hover:bg-input",
        )}
      >
        <input
          type="checkbox"
          className="size-4 shrink-0 accent-accent"
          aria-label={strings.picker.select}
          checked={selected && !queued}
          disabled={disabled || queued}
          onChange={(event) => onSelect(event.target.checked)}
        />
        <Thumbnail key={video.thumbnailUrl} url={video.thumbnailUrl} className={classes("w-24", known && "opacity-50")} />
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <p className={classes("truncate", known && "text-fg-muted")} title={video.title}>
            {video.title}
          </p>
          <p className="truncate text-xs text-fg-secondary tabular-nums">{listedDetails(item, showUploader, source)}</p>
        </div>
        {queued ? (
          <span
            className="shrink-0 rounded-full border border-line px-1.5 text-xs text-fg-secondary"
            title={strings.picker.inQueueHint}
          >
            {strings.picker.inQueue}
          </span>
        ) : (
          lastDownload && (
            <span
              className="shrink-0 rounded-full border border-line px-1.5 text-xs text-fg-secondary"
              title={strings.preview.alreadyDownloaded(formatDay(lastDownload.downloadedAt), lastDownload.preset)}
            >
              {strings.picker.downloaded}
            </span>
          )
        )}
      </label>
    </li>
  )
}

/** "source · uploader · date · length" or what a post holds, under an item's title in List and in its hover text
 * elsewhere. */
function listedDetails({ video, approximateDate }: ListedItem, showUploader: boolean, source: string | null): string {
  const date = video.uploadDate
    ? formatCalendarDate(video.uploadDate)
    : approximateDate && formatRoughAge(approximateDate)
  return joinDetails([
    source,
    showUploader && video.uploader,
    date,
    video.durationSeconds !== null && formatDuration(video.durationSeconds),
    formatContents(video),
  ])
}

interface ListedItemsProps<Item extends ListedItem> {
  items: readonly Item[]
  layout: PickerLayout
  /** A playlist's items may come from several uploaders; a channel's or an account's all come from it. */
  showUploader: boolean
  /** In All New Items, the watched source that found each item: named under its picture and first in its details. */
  sourceOf?: (item: Item) => string
  isSelected: (item: Item) => boolean
  disabled: boolean
  onSelect: (item: Item, ticked: boolean) => void
  /** In All New Items: the items take what the view's height leaves them (in a column that shrinks to the view), rather
   * than at most 28 rem or half the window. */
  fill?: boolean
}

/**
 * A listing's items as the picker's view shows them: a row each (List), or their pictures (Masonry, Gallery), each with
 * its box. Gallery cuts a video's picture to 16:9 and a post's to a square. They scroll on their own.
 */
export function ListedItems<Item extends ListedItem>({
  items,
  layout,
  showUploader,
  sourceOf,
  isSelected,
  disabled,
  onSelect,
  fill = false,
}: ListedItemsProps<Item>) {
  const height = fill ? "min-h-0" : "max-h-[min(28rem,50vh)]"
  if (layout.view === "list") {
    return (
      <ul className={classes("-mx-2 flex flex-col overflow-y-auto", height)}>
        {items.map((item) => (
          <ListedRow
            key={itemKey(item)}
            item={item}
            showUploader={showUploader}
            source={sourceOf?.(item) ?? null}
            selected={isSelected(item)}
            disabled={disabled}
            onSelect={(ticked) => onSelect(item, ticked)}
          />
        ))}
      </ul>
    )
  }
  const posts = items.some((item) => item.video.post !== null)
  return (
    <MediaTiles
      tiles={items.map((item) =>
        listedTile(item, showUploader, sourceOf?.(item) ?? null, isSelected(item), (ticked) => onSelect(item, ticked)),
      )}
      view={layout.view}
      size={layout.size}
      galleryRatio={posts ? 1 : 16 / 9}
      disabled={disabled}
      className={classes("-mx-1", height)}
    />
  )
}

/** An item as a picture: its title and details as its hover text, its length or what a post holds in its corner, and
 * Downloaded or In Queue under it, after the source that found it in All New Items. */
function listedTile(
  item: ListedItem,
  showUploader: boolean,
  source: string | null,
  selected: boolean,
  onToggle: (ticked: boolean) => void,
): MediaTile {
  const { video, lastDownload } = item
  const queued = isQueued(item)
  const post = video.post
  const fileCount = post ? post.images + post.videos + post.others : 0
  const onlyVideo = post !== null && fileCount === 1 && post.videos === 1
  const known = queued
    ? strings.picker.inQueueHint
    : lastDownload && strings.preview.alreadyDownloaded(formatDay(lastDownload.downloadedAt), lastDownload.preset)
  return {
    key: itemKey(item),
    thumbnailUrl: video.thumbnailUrl,
    fallbackIcon: post && !onlyVideo ? Images : Film,
    label: video.title,
    hint: [video.title, listedDetails(item, showUploader, source), known].filter(Boolean).join("\n"),
    picked: selected,
    locked: queued,
    ratio: post ? 4 / 5 : 16 / 9,
    corner:
      video.durationSeconds !== null ? (
        <TileLabel>{formatDuration(video.durationSeconds)}</TileLabel>
      ) : fileCount > 1 ? (
        <TileLabel>
          <span className="flex items-center gap-1">
            <Images size={12} />
            {fileCount}
          </span>
        </TileLabel>
      ) : onlyVideo ? (
        <span className="block rounded bg-accent-fill p-1 text-on-accent">
          <Play size={10} fill="currentColor" />
        </span>
      ) : null,
    mark: queued
      ? { text: joinDetails([strings.picker.inQueue, source]), tone: "plain" }
      : lastDownload
        ? { text: joinDetails([strings.picker.downloaded, source]), tone: "done" }
        : source
          ? { text: source, tone: "plain" }
          : null,
    onToggle,
  }
}

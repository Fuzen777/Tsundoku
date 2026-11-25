import { useEffect, useMemo, useRef, useState, type ReactNode } from "react"
import { createPortal } from "react-dom"
import { ChevronDown, ChevronUp, Film, Images, Maximize2, Minimize2, Play, Undo2, X } from "lucide-react"
import {
  addManyToQueue,
  cancelLoadingMore,
  confirmWithUser,
  countListing,
  loadMoreOfListing,
  skipListingItems,
  type ListingPause,
} from "../api"
import type { ItemKey } from "../bindings/ItemKey"
import type { ListedItem } from "../bindings/ListedItem"
import type { Listing } from "../bindings/Listing"
import type { ListingMore } from "../bindings/ListingMore"
import type { ListingPace } from "../bindings/ListingPace"
import type { MoreRead } from "../bindings/MoreRead"
import type { PickerLayout } from "../bindings/PickerLayout"
import { describeDownloadError, describePreviewError, isCancelled, type ErrorDescription } from "../errors"
import {
  formatCalendarDate,
  formatClockTime,
  formatContents,
  formatDay,
  formatDuration,
  formatRoughAge,
  joinDetails,
} from "../format"
import { useTimeLeft } from "../holds"
import {
  firstNewSince,
  cannotTick,
  discardedKeys,
  isKnown,
  isLocked,
  isQueued,
  itemKey,
  keptTicks,
  keyOf,
  LOAD_NEW_COUNT,
  mediaCount,
  nounOf,
  pickerItems,
  picksFor,
  restEstimate,
  selectedItems,
  selectEvery,
  shownItems,
  showsUploaders,
  sourceOf,
  splitByTicks,
  takesPreset,
  undone,
  withItem,
  withKeptTicks,
  withMoreItems,
  withQueued,
  withSkipped,
  withTicks,
  type MediaCount,
  type TickChange,
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
import { SegmentedControl, ToggleButton } from "./SegmentedControl"
import { SiteIcon } from "./SiteIcon"
import { StatusMessage } from "./StatusMessage"
import { classes } from "./styles"
import { Thumbnail } from "./Thumbnail"
import { WatchForm } from "./WatchForm"
import { keepListing, keepPicker, keptPicker } from "../keptPickers"
import { sitePictureUrl } from "../api"

interface ListingPickerProps {
  listing: Listing
  /** The items skipped the last time the listing was downloaded from, or by the source watching it: they start
   * unticked, under Discarded, marked Skipped. */
  skipped: readonly ItemKey[]
  /** The pending link it was opened from (Pick What to Download): its ticks and their changes are kept for it while it
   * waits in Pending, and the picker opens with them again. */
  pendingId: number | null
  /** Opens in full page (Settings → Adding Links' Pick in Full Page). */
  startFullPage: boolean
  /** Pending isn't the open view: the picker stays as it is, out of sight, even in full page. */
  hidden: boolean
  onClose: () => void
  /** The picked items joined the queue. */
  onAdded: () => void
  /** Some picked items joined the queue, and the card stays open to pick more (Download and Load More). */
  onSomeAdded: () => void
  /** It's watched now: the card closes, and the Watched view opens. */
  onWatched: () => void
}

/** The picker's two tabs: what will download, and what won't. */
type PickerTab = "selected" | "discarded"

/** What the picker is loading: the next items (Load More), on until enough new ones (Load New) or all the rest (Load
 * All). */
type Loading = MoreRead["kind"]

/** Where Load All's count is, when the first read didn't say: another read counting it (a YouTube channel's videos), or
 * that read failed. */
type Counted = "counting" | "failed" | null

/**
 * The preview card for a channel, playlist or account: what it is, its items with a box each (the new ones ticked), a
 * type filter when they're of several types (only the images of an account, say), Load More, Load New and Load All
 * when only the newest were listed, and Download, which queues each ticked item shown as its own download, with
 * Download and Load More beside it to go on page by page. Two tabs beside Hide Downloaded split the items: Selected,
 * what will download, and Discarded, what won't (unticked, skipped before, downloaded before or in the queue, greyed or
 * hidden with Hide Downloaded); an item moves from one to the other the moment its box changes, and Download records
 * the discarded new ones as skipped for the listing. Undo, after the count, puts back the ticks' changes one at a
 * time. View in Full Page lays it over the whole window, as it opens with Pick in Full Page;
 * otherwise its card takes at most 60% of the window's height, its items scrolling inside, and Collapse folds it to one
 * line (`FoldedPicker`). A video starts with the preset a rule picks for it, else the one picked here. What it loads is
 * kept by the app, so the listing opens with it the next time; while a site makes a load wait (X), the items listed stay
 * to pick from.
 */
export function ListingPicker({
  listing,
  skipped: skippedBefore,
  pendingId,
  startFullPage,
  hidden: outOfSight,
  onClose,
  onAdded,
  onSomeAdded,
  onWatched,
}: ListingPickerProps) {
  // What's skipped, as the core keeps it, and by the picker's keys (also for loads, which a ref reads as it is then).
  const [skipped, setSkipped] = useState<readonly ItemKey[]>(skippedBefore)
  const skippedKeys = useMemo(() => new Set(skipped.map(keyOf)), [skipped])
  const skippedNow = useRef<ReadonlySet<string>>(skippedKeys)
  const [picker, setPicker] = useState(() => {
    const kept = pendingId === null ? null : (keptPicker(pendingId)?.ticks ?? null)
    const fresh = pickerItems(listing.items, new Set(skippedBefore.map(keyOf)))
    return kept ? withKeptTicks(fresh, kept) : fresh
  })
  const { items, selection, undo } = picker
  const [more, setMore] = useState<ListingMore | null>(listing.more)
  const [total, setTotal] = useState(listing.total)
  const [counted, setCounted] = useState<Counted>(() =>
    listing.total === null && listing.more?.channelId ? "counting" : null,
  )
  const [loading, setLoading] = useState<Loading | null>(null)
  const [pause, setPause] = useState<ListingPause | null>(null)
  const [stopping, setStopping] = useState(false)
  const [loadError, setLoadError] = useState<ErrorDescription | null>(null)
  const { data: presets } = usePresets()
  const rulePresets = useRulePresets(items.map((item) => item.video))
  const [chosenId, setChosenId] = useState<string | null>(null)
  // What Download is queueing: all that's ticked, the card then closing, or a page of it (Download and Load More).
  const [adding, setAdding] = useState<"all" | "page" | null>(null)
  const isAdding = adding !== null
  const [error, setError] = useState<ErrorDescription | null>(null)
  const [watching, setWatching] = useState(false)
  const [tab, setTab] = useState<PickerTab>("selected")
  const [fullPage, setFullPage] = useState(startFullPage)
  const [folded, setFolded] = useState(false)
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

  // While the link waits in Pending, what's ticked is kept for it as it changes, and the listing as loaded so far, so
  // picking from it again carries on at once, with nothing read again.
  useEffect(() => {
    if (pendingId !== null) keepPicker(pendingId, keptTicks(picker))
  }, [pendingId, picker])
  useEffect(() => {
    if (pendingId !== null) keepListing(pendingId, { ...listing, items: [...items], more, total }, skipped)
  }, [pendingId, listing, items, more, total, skipped])
  useEffect(() => {
    skippedNow.current = skippedKeys
  }, [skippedKeys])

  // Escape leaves the full page.
  useEffect(() => {
    if (!fullPage || outOfSight) return
    const leave = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !event.defaultPrevented) setFullPage(false)
    }
    window.addEventListener("keydown", leave)
    return () => window.removeEventListener("keydown", leave)
  }, [fullPage, outOfSight])

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

  /** Changes the ticks by hand, the change kept for Undo. */
  const setSelection = (update: (selection: ReadonlySet<string>) => ReadonlySet<string>) =>
    setPicker((current) => withTicks(current, update(current.selection)))
  const types = useTypeFilter(shownItems(items, layout.hideDownloaded))
  const shown = types.shown
  const hidden = items.length - shown.length
  const picked = selectedItems(shown, selection)
  const selectable = shown.filter((item) => !cannotTick(item)).length
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
  const nothingPicked = !picks || picks.length === 0
  const { ticked: toDownload, leftOut } = splitByTicks(shown, selection)
  const tabItems = tab === "selected" ? toDownload : leftOut
  const titleOf = new Map(items.map((item) => [itemKey(item), item.video.title]))

  /** Ticks or unticks an item: it moves to Selected, or to Discarded, at once. */
  function select(item: ListedItem, ticked: boolean) {
    setSelection((current) => withItem(current, item, ticked))
  }

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

  /** Lists the next items, on until enough new ones, or all the rest, adding each batch as it comes, the new ones
   * ticked. */
  async function load(read: MoreRead) {
    if (!more || loading) return
    setLoading(read.kind)
    setLoadError(null)
    listedBeforeNew.current = read.kind === "new" ? new Set(items.map(itemKey)) : null
    try {
      await loadMoreOfListing(
        more,
        read,
        (batch) => {
          if (!open.current) return
          setPicker((current) => withMoreItems(current, batch.items, listing.kind, skippedNow.current))
          setMore(batch.more)
          if (batch.total !== null) setTotal(batch.total)
        },
        (waits) => {
          if (open.current) setPause(waits)
        },
      )
    } catch (caught) {
      if (open.current && !isCancelled(caught)) setLoadError(describePreviewError(caught))
    } finally {
      if (open.current) {
        setLoading(null)
        setPause(null)
        setStopping(false)
      }
    }
  }

  function stop() {
    setStopping(true)
    void cancelLoadingMore()
  }

  /** Queues the ticked items shown, then closes the card, or with `thenLoadMore` marks them In Downloads (under
   * Discarded) and lists the next ones, ticked, to go on picking. The new items discarded are kept as skipped for the
   * listing, and in the source watching it, so they start unticked the next time. */
  async function download(thenLoadMore = false) {
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
    setAdding(thenLoadMore ? "page" : "all")
    setError(null)
    try {
      await addManyToQueue(picks, sourceOf(listing))
    } catch (caught) {
      setError(describeDownloadError(caught))
      setAdding(null)
      return
    }
    const discarded = discardedKeys(leftOut)
    const taken = picks.map(({ video }) => ({ site: video.site, videoId: video.videoId }))
    // Not kept if it fails: the downloads are queued, and the next time those items start ticked, as before.
    skipListingItems(listing.url, discarded, taken).catch(() => {})
    setSkipped((current) => withSkipped(current, discarded, taken))
    if (!thenLoadMore) {
      onAdded()
      return
    }
    const queued = new Set(picked.map(itemKey))
    setPicker((current) => withQueued(current, queued))
    setAdding(null)
    onSomeAdded()
    void load({ kind: "next" })
  }

  const noun = strings.picker.nouns[nounOf(listing)]
  const media = mediaCount(listing, items, total)
  const loadRow = (more !== null || loading !== null || loadError !== null) && (
    <LoadRow
      loading={loading}
      pause={loading === null ? null : pause}
      stopping={stopping}
      canLoad={more !== null}
      count={items.length}
      fresh={items.filter((item) => !isKnown(item)).length}
      total={total}
      media={media}
      pace={more === null ? null : listing.pace}
      restMinutes={more === null ? null : restEstimate(listing.pace, media)}
      counted={total === null ? counted : null}
      noun={noun}
      slowly={listing.kind === "account" && listing.siteName === "instagram.com"}
      error={loadError}
      disabled={isAdding}
      onLoad={(read) => void load(read)}
      onStop={stop}
    />
  )
  const lastChange = undo.at(-1)
  const listed = joinDetails([
    strings.picker.kinds[listing.kind],
    strings.picker.listed(items.length, noun, listing.newestOnly && more !== null, total),
  ])
  if (folded && !fullPage) {
    return (
      <FoldedPicker
        listing={listing}
        details={joinDetails([listed, listing.uploader, listing.siteName])}
        picked={picked.length}
        onExpand={() => setFolded(false)}
        onClose={onClose}
      />
    )
  }
  const content = (
    <>
      <div className="flex gap-4">
        <ListingPicture listing={listing} />
        <div className="flex min-w-0 flex-1 flex-col gap-1">
          <p className="line-clamp-2 font-semibold select-text" title={listing.title}>
            {listing.title}
          </p>
          <p className="flex min-w-0 items-center gap-1.5 text-fg-secondary">
            <SiteIcon page={listing.url} site={listing.siteName} />
            <span className="truncate">{joinDetails([listing.uploader, listing.siteName])}</span>
          </p>
          <p className="text-xs text-fg-secondary tabular-nums">{listed}</p>
        </div>
        {!fullPage && (
          <IconButton label={strings.picker.collapse} icon={ChevronUp} onClick={() => setFolded(true)} />
        )}
        <IconButton label={strings.preview.close} icon={X} onClick={onClose} />
      </div>
      {items.length === 0 ? (
        <p className="text-fg-secondary">{strings.picker.empty}</p>
      ) : (
        <>
          <div className="flex flex-wrap items-center justify-between gap-x-3 gap-y-2">
            <div className="flex items-center gap-2">
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
                  disabled={isAdding || selectable === 0}
                  onChange={(event) => {
                    const ticked = event.target.checked
                    setSelection((current) => selectEvery(current, shown, ticked))
                  }}
                />
                <span className="whitespace-nowrap tabular-nums">
                  {joinDetails([
                    strings.picker.shownCount(shown.length, noun),
                    hidden > 0 && strings.picker.hidden(hidden),
                  ])}
                </span>
              </label>
              <UndoButton change={lastChange} titleOf={titleOf} disabled={isAdding} onUndo={() => setPicker(undone)} />
            </div>
            <div className="flex flex-wrap items-center justify-end gap-2">
              <ItemTypeFilter counts={types.counts} value={types.type} onChange={types.choose} disabled={isAdding} />
              <SegmentedControl<PickerTab>
                label={strings.picker.tab}
                options={[
                  {
                    value: "selected",
                    label: strings.picker.typeCount(strings.picker.tabs.selected, toDownload.length),
                    hint: strings.picker.tabHints.selected,
                  },
                  {
                    value: "discarded",
                    label: strings.picker.typeCount(strings.picker.tabs.discarded, leftOut.length),
                    hint: strings.picker.tabHints.discarded,
                  },
                ]}
                value={tab}
                onChange={setTab}
              />
              <ToggleButton
                label={strings.picker.hideDownloaded}
                hint={strings.picker.hideDownloadedHint}
                pressed={layout.hideDownloaded}
                onChange={(hideDownloaded) => changeLayout((latest) => ({ ...latest, hideDownloaded }))}
              />
              <PickerViewControls layout={layout} onChange={changeLayout} />
              <IconButton
                label={fullPage ? strings.picker.exitFullPage : strings.picker.fullPage}
                icon={fullPage ? Minimize2 : Maximize2}
                onClick={() => setFullPage((wasFull) => !wasFull)}
              />
            </div>
          </div>
          {/* The items scroll here, taking what the rest leaves them: of the whole window in full page, else of the
              card's 60%. */}
          <div
            ref={list}
            className={classes("-mx-2 flex min-h-0 flex-col gap-3 overflow-y-auto px-2", fullPage && "flex-1")}
          >
            {shown.length === 0 ? (
              <p className="text-fg-secondary">{strings.picker.allHidden}</p>
            ) : tabItems.length === 0 ? (
              <p className="text-fg-secondary">{strings.picker.emptyTabs[tab]}</p>
            ) : (
              <ListedItems
                // Each tab's list of its own, so its scroll starts at the top.
                key={tab}
                items={tabItems}
                layout={layout}
                showUploader={showsUploaders(listing)}
                isSelected={(item) => tab === "selected" && selection.has(itemKey(item))}
                isSkipped={(item) => skippedKeys.has(itemKey(item))}
                disabled={isAdding}
                onSelect={select}
                fill
              />
            )}
          </div>
          {loadRow}
          {watching ? (
            <WatchForm
              listing={{ ...listing, items: [...items] }}
              selected={picked}
              onCancel={() => setWatching(false)}
              onWatched={onWatched}
            />
          ) : (
            <div className="flex flex-col items-end gap-2">
              {error && <StatusMessage tone="danger" {...error} />}
              <div className="flex w-full flex-wrap items-center justify-end gap-2">
                <Button
                  className="mr-auto"
                  disabled={isAdding}
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
                      disabled={!presets || isAdding}
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
                {more !== null && (
                  <Button
                    title={strings.picker.downloadAndLoadMoreHint(noun)}
                    loading={adding === "page"}
                    disabled={nothingPicked || adding === "all" || loading !== null}
                    onClick={() => void download(true)}
                  >
                    {strings.picker.downloadAndLoadMore(picked.length)}
                  </Button>
                )}
                <Button
                  variant="primary"
                  loading={adding === "all"}
                  disabled={nothingPicked || adding === "page"}
                  onClick={() => void download()}
                >
                  {strings.picker.download(picked.length)}
                </Button>
              </div>
            </div>
          )}
        </>
      )}
    </>
  )
  if (!fullPage || outOfSight) return content
  // Over the whole window, the list taking what the rest leaves it; the card it came from stays where it was.
  return createPortal(
    <div
      role="dialog"
      aria-modal="true"
      aria-label={listing.title}
      className="fixed inset-0 z-40 flex flex-col gap-4 bg-bg p-6"
    >
      {content}
    </div>,
    document.body,
  )
}

interface FoldedPickerProps {
  listing: Listing
  /** What it is, how many it lists, its uploader and site, for the hover text. */
  details: string
  /** How many items are ticked. */
  picked: number
  onExpand: () => void
  onClose: () => void
}

/** The picker folded to one line: the channel's or account's picture, the site's icon (else its name), the listing's
 * name and how many items are ticked, which opens it again; its page and details in the hover text. What's ticked and
 * what's loaded stay, and loading goes on. */
function FoldedPicker({ listing, details, picked, onExpand, onClose }: FoldedPickerProps) {
  return (
    <div className="flex items-center gap-2">
      <button
        type="button"
        aria-expanded={false}
        title={strings.picker.foldedHint(listing.url, details)}
        onClick={onExpand}
        className="flex min-w-0 flex-1 items-center gap-2 rounded-md text-left focus-visible:outline-2 focus-visible:outline-accent"
      >
        <ListingPicture listing={listing} size="tiny" />
        <SiteIcon
          page={listing.url}
          site={listing.siteName}
          fallback={
            <span className="shrink-0 text-fg-secondary">{`${listing.siteName}${strings.separator.trimEnd()}`}</span>
          }
        />
        <span className="truncate font-medium">{listing.title}</span>
        <span className="shrink-0 text-fg-secondary tabular-nums">
          {`${strings.separator.trimStart()}${strings.picker.picked(picked)}`}
        </span>
      </button>
      <IconButton label={strings.picker.expand} icon={ChevronDown} aria-expanded={false} onClick={onExpand} />
      <IconButton label={strings.preview.close} icon={X} onClick={onClose} />
    </div>
  )
}

interface UndoButtonProps {
  /** The latest change of ticks, none before the first. */
  change: TickChange | undefined
  titleOf: ReadonlyMap<string, string>
  disabled: boolean
  onUndo: () => void
}

/** Undo, after the count of what's ticked: puts back the latest change of ticks, its hint naming it. */
function UndoButton({ change, titleOf, disabled, onUndo }: UndoButtonProps): ReactNode {
  const named = (keys: readonly string[]) => (keys.length === 1 ? (titleOf.get(keys[0]) ?? keys.length) : keys.length)
  const hint = change
    ? strings.picker.undoHint(named(change.ticked), named(change.unticked))
    : strings.picker.nothingToUndo
  return (
    <Button size="sm" variant="ghost" title={hint} aria-label={hint} disabled={disabled || !change} onClick={onUndo}>
      <Undo2 size={14} aria-hidden />
      {strings.picker.undo}
    </Button>
  )
}

interface LoadRowProps {
  loading: Loading | null
  /** The site makes the load wait (X's limit on how far an account is listed at a time): it goes on by itself. */
  pause: ListingPause | null
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
  /** On a site that lists only so far at a time (X): how far, and how many minutes listing the rest takes when it
   * takes a wait or more. */
  pace: ListingPace | null
  restMinutes: number | null
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
 * are listed so far and how many are new, and Stop. While the site makes a load wait (X), "Paused by X, resuming at…"
 * with the time left takes the count's place, Load More's too. On an X account too big to list in one go, a line under
 * them says how X paces it and how long the rest takes. What went wrong shows over them, and they stay to try again.
 */
function LoadRow({
  loading,
  pause,
  stopping,
  canLoad,
  count,
  fresh,
  total,
  media,
  pace,
  restMinutes,
  counted,
  noun,
  slowly,
  error,
  disabled,
  onLoad,
  onStop,
}: LoadRowProps) {
  const text = strings.picker
  const rest = restMinutes === null ? null : text.roughTime(restMinutes)
  const waitMinutes = pace === null ? 0 : Math.round(pace.pauseSeconds / 60)
  const note = pause
    ? text.pausedNote(noun, rest)
    : pace !== null && rest !== null
      ? loading === null
        ? text.paceBefore(pace.items, waitMinutes, noun, rest)
        : loading === "all"
          ? text.paceWhile(pace.items, waitMinutes, noun, rest)
          : null
      : null
  return (
    <div className="flex flex-col items-center gap-2">
      {error && <StatusMessage tone="danger" {...error} />}
      {loading === "all" || loading === "new" || pause ? (
        <>
          <div className="flex items-center gap-3">
            {pause ? (
              <PausedStatus key={pause.until} pause={pause} />
            ) : (
              <p className="text-fg-secondary tabular-nums" role="status">
                {text.loadingAll(count, total, noun, fresh, media)}
              </p>
            )}
            <Button size="sm" disabled={stopping} onClick={onStop}>
              {text.stopLoading}
            </Button>
          </div>
          {note ? (
            <p className="max-w-xl text-center text-xs text-fg-secondary">{note}</p>
          ) : (
            slowly && <p className="text-center text-xs text-fg-secondary">{text.loadingSlowly}</p>
          )}
        </>
      ) : (
        canLoad && (
          <>
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
            {note && <p className="max-w-xl text-center text-xs text-fg-secondary">{note}</p>}
          </>
        )
      )}
    </div>
  )
}

/** "Paused by X, resuming at 1:42 AM (12:31 left)", counting down. Shown only while the site makes a load wait, so
 * its clock starts with the wait. */
function PausedStatus({ pause }: { pause: ListingPause }) {
  const timeLeft = useTimeLeft(pause.until)
  return (
    <p className="text-fg-secondary tabular-nums" role="status">
      {strings.picker.pausedBySite(pause.site, formatClockTime(pause.until), timeLeft)}
    </p>
  )
}

/** How large a listing's picture shows: on the picker, on a watched source's card, on a pending link's row, or on the
 * picker's folded line, where
 * a playlist shows none (its first video's thumbnail isn't its own picture) and a channel or an account none while it
 * has none. */
const PICTURE_SIZES = {
  large: { round: "size-24", thumbnail: "w-44" },
  small: { round: "size-14", thumbnail: "w-24" },
  /** A link's row in Pending, once its picker listed it. */
  row: { round: "size-12", thumbnail: "w-24" },
  tiny: { round: "size-6", thumbnail: null },
} as const

/** A channel's or an account's picture, round, or a playlist's first thumbnail. */
export function ListingPicture({
  listing,
  size = "large",
}: {
  listing: Pick<Listing, "kind" | "thumbnailUrl">
  size?: keyof typeof PICTURE_SIZES
}) {
  const [failed, setFailed] = useState(false)
  const sizes = PICTURE_SIZES[size]
  if (listing.kind === "playlist") {
    if (!sizes.thumbnail) return null
    return <Thumbnail key={listing.thumbnailUrl} url={listing.thumbnailUrl} className={sizes.thumbnail} />
  }
  if (size === "tiny" && (!listing.thumbnailUrl || failed)) return null
  return (
    <div className={classes("shrink-0 overflow-hidden rounded-full bg-input", sizes.round)}>
      {listing.thumbnailUrl && !failed && (
        <img
          src={sitePictureUrl(listing.thumbnailUrl)}
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
  /** Left unticked the last time its listing was downloaded from: marked Skipped. */
  skipped?: boolean
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
export function ListedRow({
  item,
  skipped = false,
  showUploader,
  source = null,
  selected,
  disabled,
  onSelect,
}: ListedRowProps) {
  const { video, lastDownload } = item
  const locked = isLocked(item)
  const known = isKnown(item) || locked
  const queued = isQueued(item)
  const untickable = cannotTick(item)
  return (
    <li data-item={itemKey(item)} className="[contain-intrinsic-size:auto_4.25rem] [content-visibility:auto]">
      <label
        className={classes(
          "flex items-center gap-3 rounded-md px-2 py-1.5 transition-colors",
          untickable ? "cursor-default" : "cursor-pointer hover:bg-input",
        )}
      >
        <input
          type="checkbox"
          className="size-4 shrink-0 accent-accent"
          aria-label={strings.picker.select}
          checked={selected && !untickable}
          disabled={disabled || untickable}
          onChange={(event) => onSelect(event.target.checked)}
        />
        <Thumbnail key={video.thumbnailUrl} url={video.thumbnailUrl} className={classes("w-24", known && "opacity-50")} />
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <p className={classes("truncate", known && "text-fg-muted")} title={video.title}>
            {video.title}
          </p>
          <p className="truncate text-xs text-fg-secondary tabular-nums">{listedDetails(item, showUploader, source)}</p>
        </div>
        {locked ? (
          <span
            className="shrink-0 rounded-full border border-line px-1.5 text-xs text-fg-secondary"
            title={strings.picker.lockedHint(video.fanbox?.fee ?? 0)}
          >
            {strings.picker.locked(video.fanbox?.fee ?? 0)}
          </span>
        ) : queued ? (
          <span
            className="shrink-0 rounded-full border border-line px-1.5 text-xs text-fg-secondary"
            title={strings.picker.inQueueHint}
          >
            {strings.picker.inQueue}
          </span>
        ) : lastDownload ? (
          <span
            className="shrink-0 rounded-full border border-line px-1.5 text-xs text-fg-secondary"
            title={strings.preview.alreadyDownloaded(formatDay(lastDownload.downloadedAt), lastDownload.preset)}
          >
            {strings.picker.downloaded}
          </span>
        ) : (
          skipped && (
            <span
              className="shrink-0 rounded-full border border-line px-1.5 text-xs text-fg-secondary"
              title={strings.picker.skippedHint}
            >
              {strings.picker.skipped}
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
  /** Left unticked the last time the listing was downloaded from: marked Skipped. */
  isSkipped?: (item: Item) => boolean
  disabled: boolean
  onSelect: (item: Item, ticked: boolean) => void
  /** In All New Items and the listing picker: the items take what the height around them leaves (a column that shrinks
   * to the view, or the picker's card or full page), rather than at most 28 rem or half the window. */
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
  isSkipped,
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
            skipped={isSkipped?.(item) ?? false}
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
        listedTile(item, showUploader, sourceOf?.(item) ?? null, isSelected(item), isSkipped?.(item) ?? false, (ticked) =>
          onSelect(item, ticked),
        ),
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
  skipped: boolean,
  onToggle: (ticked: boolean) => void,
): MediaTile {
  const { video, lastDownload } = item
  const queued = isQueued(item)
  const locked = isLocked(item)
  const fee = video.fanbox?.fee ?? 0
  const post = video.post
  const fileCount = post ? post.images + post.videos + post.others : 0
  const onlyVideo = post !== null && fileCount === 1 && post.videos === 1
  const known = locked
    ? strings.picker.lockedHint(fee)
    : queued
      ? strings.picker.inQueueHint
      : lastDownload
        ? strings.preview.alreadyDownloaded(formatDay(lastDownload.downloadedAt), lastDownload.preset)
        : skipped && strings.picker.skippedHint
  return {
    key: itemKey(item),
    thumbnailUrl: video.thumbnailUrl,
    fallbackIcon: post && !onlyVideo ? Images : Film,
    label: video.title,
    hint: [video.title, listedDetails(item, showUploader, source), known].filter(Boolean).join("\n"),
    picked: selected,
    locked: queued || locked,
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
    mark: locked
      ? { text: joinDetails([strings.picker.locked(fee), source]), tone: "plain" }
      : queued
        ? { text: joinDetails([strings.picker.inQueue, source]), tone: "plain" }
      : lastDownload
        ? { text: joinDetails([strings.picker.downloaded, source]), tone: "done" }
        : skipped
          ? { text: joinDetails([strings.picker.skipped, source]), tone: "plain" }
          : source
          ? { text: source, tone: "plain" }
          : null,
    onToggle,
  }
}

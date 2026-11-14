import { useState } from "react"
import { Film, Images, Play, X } from "lucide-react"
import { addManyToQueue, confirmWithUser } from "../api"
import type { ListedItem } from "../bindings/ListedItem"
import type { Listing } from "../bindings/Listing"
import type { PickerLayout } from "../bindings/PickerLayout"
import { describeDownloadError, type ErrorDescription } from "../errors"
import {
  formatCalendarDate,
  formatContents,
  formatDay,
  formatDuration,
  formatRoughAge,
  joinDetails,
} from "../format"
import {
  initialSelection,
  isKnown,
  isQueued,
  itemKey,
  nounOf,
  picksFor,
  selectedItems,
  selectEvery,
  takesPreset,
  withItem,
  type PickedPreset,
} from "../listingPicker"
import { usePickerLayout, usePresets, useRulePresets } from "../queries"
import { strings } from "../strings"
import { Button } from "./Button"
import { SelectInput } from "./Field"
import { IconButton } from "./IconButton"
import { MediaTiles, TileLabel, type MediaTile } from "./MediaTiles"
import { PickerViewControls } from "./PickerViewControls"
import { StatusMessage } from "./StatusMessage"
import { classes } from "./styles"
import { Thumbnail } from "./Thumbnail"
import { WatchForm } from "./WatchForm"

interface ListingPickerProps {
  listing: Listing
  onClose: () => void
  /** The picked items joined the queue. */
  onAdded: () => void
}

/**
 * The preview card for a channel, playlist or account: what it is, its items with a box each (the new ones ticked,
 * those downloaded before or in the queue greyed), and Download Selected, which queues each ticked item as its own
 * download. A video starts with the preset a rule picks for it, else the one picked here.
 */
export function ListingPicker({ listing, onClose, onAdded }: ListingPickerProps) {
  const { items } = listing
  const [selection, setSelection] = useState(() => initialSelection(items))
  const { data: presets } = usePresets()
  const rulePresets = useRulePresets(items.map((item) => item.video))
  const [chosenId, setChosenId] = useState<string | null>(null)
  const [adding, setAdding] = useState(false)
  const [error, setError] = useState<ErrorDescription | null>(null)
  const [watching, setWatching] = useState(false)
  const { layout, change: changeLayout } = usePickerLayout("items")
  const presetId = chosenId ?? presets?.defaultId ?? null

  const picked = selectedItems(items, selection)
  const selectable = items.filter((item) => !isQueued(item)).length
  const allSelected = selectable > 0 && picked.length === selectable
  const hasVideos = items.some(takesPreset)
  const placeOf = new Map(items.map((item, index) => [itemKey(item), index]))
  const presetFor = (item: ListedItem): PickedPreset | null => {
    const byRule = rulePresets[placeOf.get(itemKey(item)) ?? -1]
    // Not known yet.
    if (byRule === undefined) return null
    const preset = presets?.presets.find((candidate) => candidate.id === (byRule?.presetId ?? presetId))
    return preset ? { id: preset.id, name: preset.name } : null
  }
  const picks = presets ? picksFor(items, selection, presetFor) : null

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
      await addManyToQueue(picks)
      onAdded()
    } catch (caught) {
      setError(describeDownloadError(caught))
      setAdding(false)
    }
  }

  const noun = strings.picker.nouns[nounOf(listing)]
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
              strings.picker.listed(items.length, noun, listing.newestOnly, listing.total),
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
                onChange={(event) => setSelection(selectEvery(items, event.target.checked))}
              />
              <span className="whitespace-nowrap tabular-nums">{strings.picker.selected(picked.length, items.length)}</span>
            </label>
            <PickerViewControls layout={layout} onChange={changeLayout} />
          </div>
          <ListedItems
            items={items}
            layout={layout}
            showUploader={listing.kind === "playlist"}
            isSelected={(item) => selection.has(itemKey(item))}
            disabled={adding}
            onSelect={(item, ticked) => setSelection((current) => withItem(current, item, ticked))}
          />
          {watching ? (
            <WatchForm listing={listing} onCancel={() => setWatching(false)} onWatched={onClose} />
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
  selected: boolean
  disabled: boolean
  onSelect: (ticked: boolean) => void
}

/**
 * One item of the picker: its box, thumbnail, title, and "date · length" or what a post holds. Greyed when the history
 * or the queue has it, with Downloaded or In Queue. Rows off screen aren't laid out (`content-visibility`), so a
 * playlist of hundreds scrolls smoothly.
 */
export function ListedRow({ item, showUploader, selected, disabled, onSelect }: ListedRowProps) {
  const { video, lastDownload } = item
  const known = isKnown(item)
  const queued = isQueued(item)
  return (
    <li className="[contain-intrinsic-size:auto_4.25rem] [content-visibility:auto]">
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
          <p className="truncate text-xs text-fg-secondary tabular-nums">{listedDetails(item, showUploader)}</p>
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

/** "uploader · date · length" or what a post holds, under an item's title in List and in its hover text elsewhere. */
function listedDetails({ video, approximateDate }: ListedItem, showUploader: boolean): string {
  const date = video.uploadDate
    ? formatCalendarDate(video.uploadDate)
    : approximateDate && formatRoughAge(approximateDate)
  return joinDetails([
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
  isSelected: (item: Item) => boolean
  disabled: boolean
  onSelect: (item: Item, ticked: boolean) => void
}

/**
 * A listing's items as the picker's view shows them: a row each (List), or their pictures (Masonry, Gallery), each with
 * its box. Gallery cuts a video's picture to 16:9 and a post's to a square.
 */
export function ListedItems<Item extends ListedItem>({
  items,
  layout,
  showUploader,
  isSelected,
  disabled,
  onSelect,
}: ListedItemsProps<Item>) {
  if (layout.view === "list") {
    return (
      <ul className="-mx-2 flex max-h-[min(28rem,50vh)] flex-col overflow-y-auto">
        {items.map((item) => (
          <ListedRow
            key={itemKey(item)}
            item={item}
            showUploader={showUploader}
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
      tiles={items.map((item) => listedTile(item, showUploader, isSelected(item), (ticked) => onSelect(item, ticked)))}
      view={layout.view}
      size={layout.size}
      galleryRatio={posts ? 1 : 16 / 9}
      disabled={disabled}
      className="-mx-1 max-h-[min(28rem,50vh)]"
    />
  )
}

/** An item as a picture: its title and details as its hover text, its length or what a post holds in its corner, and
 * Downloaded or In Queue under it. */
function listedTile(item: ListedItem, showUploader: boolean, selected: boolean, onToggle: (ticked: boolean) => void): MediaTile {
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
    hint: [video.title, listedDetails(item, showUploader), known].filter(Boolean).join("\n"),
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
      ? { text: strings.picker.inQueue, tone: "plain" }
      : lastDownload
        ? { text: strings.picker.downloaded, tone: "done" }
        : null,
    onToggle,
  }
}

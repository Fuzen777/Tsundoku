import { useLayoutEffect, useRef, useState, type ReactNode } from "react"
import { ChevronDown, ChevronRight, CircleStop, Play, RefreshCw, X } from "lucide-react"
import {
  cancelTheseDownloads,
  clearTheseDownloads,
  confirmWithUser,
  restoreClearedDownloads,
  resumeTheseDownloads,
  retryTheseDownloads,
  stopTheseDownloads,
} from "../api"
import type { QueueItem } from "../bindings/QueueItem"
import type { QueueSource } from "../bindings/QueueSource"
import { describeDownloadError, type ErrorDescription } from "../errors"
import { joinDetails } from "../format"
import { usePickerLayout } from "../queries"
import { useCombinedProgress } from "../queueEvents"
import { sourceSummary, stateCounts } from "../queueSources"
import { strings } from "../strings"
import { useOfferUndo } from "../undo"
import { Card } from "./Card"
import { IconButton } from "./IconButton"
import { ListingPicture } from "./ListingPicker"
import { PickerViewControls } from "./PickerViewControls"
import { ProgressBar } from "./ProgressBar"
import { QueueRow, type RowOpening } from "./QueueRow"
import { QueueTiles } from "./QueueTiles"
import { StatusMessage } from "./StatusMessage"

const text = strings.queue.source

interface QueueSourceGroupProps {
  source: QueueSource
  /** Its downloads in this part of the list. */
  items: readonly QueueItem[]
  folded: boolean
  onFold: () => void
  /** Its downloads' rows. */
  rows: ReactNode
}

/** A download's row shown under its line of pictures, opened as asked; `opened` counts each ask, so asking again opens
 * it afresh (on its log, say, when it's already shown). */
interface ShownRow {
  id: number
  opens: RowOpening
  opened: number
}

/**
 * The downloads picked together from one channel, playlist or account, under a heading of their own: a card with the
 * source's picture, name, kind and site, how far its downloads have come ("3 of 12 done · 2 downloading") with their
 * progress together while any downloads or waits, and the actions a row has, for all of them at once. Its downloads
 * hang under it on a line, as rows (List) or pictures (Masonry, Gallery), chosen with the picker's view controls on the
 * heading and remembered for every heading. The chevron, or a click on the picture or the name, folds them away.
 */
export function QueueSourceGroup({ source, items, folded, onFold, rows }: QueueSourceGroupProps) {
  const [error, setError] = useState<ErrorDescription | null>(null)
  const { layout, change: changeLayout } = usePickerLayout("queueSources")
  const [shownRow, setShownRow] = useState<ShownRow | null>(null)
  const shownItem = shownRow && items.find((item) => item.id === shownRow.id)
  const shownBox = useRef<HTMLDivElement>(null)
  const section = useRef<HTMLElement>(null)
  // Where the picture asked about was, so it stays there as the row moves from under another line to under its own.
  const held = useRef<{ id: number; top: number } | null>(null)
  const offerUndo = useOfferUndo()
  const fraction = useCombinedProgress(items)
  const counts = stateCounts(items)
  const ids = items.map((item) => item.id)
  const unfinished = items.length - counts.done
  const active = counts.running + counts.waiting > 0
  // The bar shows once there's something on it: none of them waiting with nothing done yet.
  const showsBar = counts.running > 0 || (active && counts.done > 0)

  const act = (action: () => Promise<unknown>) => {
    setError(null)
    action().catch((caught: unknown) => setError(describeDownloadError(caught)))
  }
  /** Cancel for each one not done, once confirmed, since what they downloaded is deleted. */
  const cancelAll = async () => {
    const confirmed = await confirmWithUser({
      title: text.cancelTitle(unfinished, source.name),
      message: text.cancelMessage(unfinished, counts.done),
      confirmLabel: text.cancelConfirm(unfinished),
      cancelLabel: text.keep(unfinished),
    })
    if (confirmed) await cancelTheseDownloads(ids)
  }
  /** Clear Finished for them, then an offer for a few seconds to put them back. */
  const clearFinished = async () => {
    const cleared = await clearTheseDownloads(ids)
    if (cleared > 0) {
      offerUndo({
        message: strings.queue.cleared(cleared),
        undo: restoreClearedDownloads,
        describeError: describeDownloadError,
      })
    }
  }

  const pictureOf = (id: number) => section.current?.querySelector(`[data-download="${id}"]`)
  const showRow = (id: number, opens: RowOpening) => {
    const top = pictureOf(id)?.getBoundingClientRect().top
    held.current = top === undefined ? null : { id, top }
    setShownRow((shown) => ({ id, opens, opened: (shown?.opened ?? 0) + 1 }))
  }
  // Before it's painted: the picture asked about back where it was (a row shown higher up has closed), then the row
  // shown under it brought into sight, scrolling no more than that takes.
  useLayoutEffect(() => {
    const before = held.current
    held.current = null
    const picture = before && pictureOf(before.id)
    if (before && picture) {
      const moved = picture.getBoundingClientRect().top - before.top
      const scroller = scrollerOf(picture)
      if (moved !== 0 && scroller) scroller.scrollTop += moved
    }
    if (shownRow) shownBox.current?.scrollIntoView({ block: "nearest" })
  }, [shownRow])

  return (
    <section ref={section} className="flex flex-col gap-2" aria-label={source.name}>
      <Card className="flex flex-col gap-2 p-3">
        {/* The controls move under the name when both don't fit on one line, so the name stays readable. */}
        <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
          <div className="flex min-w-64 flex-1 items-center gap-3">
            <IconButton
              label={folded ? strings.lists.expand(source.name) : strings.lists.collapse(source.name)}
              icon={folded ? ChevronRight : ChevronDown}
              aria-expanded={!folded}
              onClick={onFold}
            />
            {/* The chevron is the control; the picture and the name only make it easier to hit. */}
            <div className="flex min-w-0 flex-1 cursor-pointer items-center gap-3" onClick={onFold}>
              <ListingPicture listing={source} small />
              <div className="flex min-w-0 flex-col gap-1">
                <p className="truncate font-semibold" title={source.name}>
                  {source.name}
                </p>
                <p className="truncate text-xs text-fg-secondary">
                  {joinDetails([strings.picker.kinds[source.kind], source.siteName])}
                </p>
                <p className="truncate text-xs text-fg-secondary tabular-nums">{sourceSummary(items)}</p>
              </div>
            </div>
          </div>
          <div className="ml-auto flex shrink-0 items-center gap-3">
            {!folded && <PickerViewControls layout={layout} onChange={changeLayout} />}
            <div className="flex items-center gap-1">
              {active && (
                <IconButton
                  label={text.pauseAll(source.name)}
                  icon={CircleStop}
                  variant="secondary"
                  onClick={() => act(() => stopTheseDownloads(ids))}
                />
              )}
              {counts.stopped > 0 && (
                <IconButton
                  label={text.resumeAll(source.name)}
                  icon={Play}
                  variant="secondary"
                  onClick={() => act(() => resumeTheseDownloads(ids))}
                />
              )}
              {counts.failed > 0 && (
                <IconButton
                  label={text.retryFailed(source.name)}
                  icon={RefreshCw}
                  variant="secondary"
                  onClick={() => act(() => retryTheseDownloads(ids))}
                />
              )}
              {unfinished > 0 ? (
                <IconButton label={text.cancelAll(source.name)} icon={X} onClick={() => act(cancelAll)} />
              ) : (
                <IconButton label={text.clearFinished(source.name)} icon={X} onClick={() => act(clearFinished)} />
              )}
            </div>
          </div>
        </div>
        {showsBar && <ProgressBar value={fraction} label={text.progress(source.name)} />}
        {error && <StatusMessage tone="danger" {...error} />}
      </Card>
      {!folded && (
        <div className="ml-6 flex flex-col gap-2 border-l-2 border-line pl-2">
          {layout.view === "list" ? (
            rows
          ) : (
            <QueueTiles
              items={items}
              view={layout.view}
              size={layout.size}
              // An account's posts are cut square, a channel's or a playlist's videos to 16:9, as in the picker.
              galleryRatio={source.kind === "account" ? 1 : 16 / 9}
              shownId={shownItem ? shownItem.id : null}
              details={
                shownItem && (
                  <div ref={shownBox} className="scroll-my-2">
                    <QueueRow
                      key={`${shownItem.id}-${shownRow.opened}`}
                      item={shownItem}
                      opens={shownRow.opens}
                      onHide={() => setShownRow(null)}
                    />
                  </div>
                )
              }
              onShow={showRow}
              onHide={() => setShownRow(null)}
              onError={setError}
            />
          )}
        </div>
      )}
    </section>
  )
}

/** The box an element scrolls in: the nearest one above it that scrolls, or the page. */
function scrollerOf(element: Element): Element | null {
  for (let box = element.parentElement; box; box = box.parentElement) {
    const { overflowY } = getComputedStyle(box)
    if ((overflowY === "auto" || overflowY === "scroll") && box.scrollHeight > box.clientHeight) return box
  }
  return document.scrollingElement
}

import { useState, type ReactNode } from "react"
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
import { useCombinedProgress } from "../queueEvents"
import { useQueueLook } from "../queueItems"
import { sourceSummary, stateCounts } from "../queueSources"
import { strings } from "../strings"
import { useOfferUndo } from "../undo"
import { Card } from "./Card"
import { Collapsible } from "./Collapsible"
import { IconButton } from "./IconButton"
import { QueueSourcePicture } from "./SourcePicture"
import { ProgressBar } from "./ProgressBar"
import { StatusMessage } from "./StatusMessage"

const text = strings.queue.source

interface QueueSourceGroupProps {
  source: QueueSource
  /** Its downloads in this part of the list. */
  items: readonly QueueItem[]
  folded: boolean
  onFold: () => void
  /** Its downloads, as rows or pictures (the Downloads view's choice). */
  rows: ReactNode
}

/**
 * The downloads picked together from one channel, playlist or account, under a heading of their own: a card with the
 * source's picture, name, kind and site, how far its downloads have come ("3 of 12 done · 2 downloading") with their
 * progress together while any downloads or waits, and the actions a row has, for all of them at once. Grouped by
 * status, it only says how many there are, since the status's heading says where they stand. Its downloads hang under it
 * on a line, as rows or pictures, as the whole view shows them. The chevron, or a click on the picture or the name,
 * folds them away.
 */
export function QueueSourceGroup({ source, items, folded, onFold, rows }: QueueSourceGroupProps) {
  const [error, setError] = useState<ErrorDescription | null>(null)
  const look = useQueueLook()
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

  return (
    <section className="flex flex-col gap-2" aria-label={source.name}>
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
              <QueueSourcePicture listing={source} />
              <div className="flex min-w-0 flex-col gap-1">
                <p className="truncate font-semibold" title={source.name}>
                  {source.name}
                </p>
                <p className="truncate text-xs text-fg-secondary">
                  {joinDetails([strings.picker.kinds[source.kind], source.siteName])}
                </p>
                <p className="truncate text-xs text-fg-secondary tabular-nums">{sourceSummary(items, look.byStatus)}</p>
              </div>
            </div>
          </div>
          <div className="ml-auto flex shrink-0 items-center gap-1">
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
        {showsBar && <ProgressBar value={fraction} label={text.progress(source.name)} />}
        {error && <StatusMessage tone="danger" {...error} />}
      </Card>
      <Collapsible open={!folded} gap={2} className="ml-6 flex flex-col gap-2 border-l-2 border-line pl-2">
        {rows}
      </Collapsible>
    </section>
  )
}

import type { QueryClient } from "@tanstack/react-query"
import {
  confirmWithUser,
  getBackgroundOptions,
  onCloseRequested,
  onDownloadProgress,
  onPendingChanged,
  onQueueChanged,
  queryKeys,
} from "./api"
import type { DownloadProgress } from "./bindings/DownloadProgress"
import type { QueueItem } from "./bindings/QueueItem"
import { createLiveStore } from "./liveStore"
import { strings } from "./strings"

/*
  Keeps the UI's copy of the download queue and the pending links current from the core's events: queue-changed and
  pending-changed replace them in React Query, and download-progress updates each running item's progress several
  times a second, outside it.
*/

const progress = createLiveStore<number, DownloadProgress>()

/** Starts listening to the queue's events, and decides what closing the window does: with the tray on, it hides;
 * otherwise, it asks first while downloads run. Called once, before the first render (main.tsx). */
export function trackQueue(queryClient: QueryClient): void {
  let doneIds = new Set<number>()
  void onQueueChanged((items) => {
    queryClient.setQueryData(queryKeys.queue, items)
    // A pending link may now be downloaded already or queued.
    void queryClient.invalidateQueries({ queryKey: queryKeys.pending })
    // A download that finished is in the history now. Only then is the history read again, since an open History
    // reads every page it has shown.
    const done = new Set(items.filter((item) => item.state === "done").map((item) => item.id))
    const finished = [...done].some((id) => !doneIds.has(id))
    doneIds = done
    if (finished) {
      void queryClient.invalidateQueries({ queryKey: queryKeys.history })
      void queryClient.invalidateQueries({ queryKey: queryKeys.historyFilters })
      void queryClient.invalidateQueries({ queryKey: queryKeys.videoDownloads })
    }
    for (const item of items) {
      if (item.progress) {
        progress.set(item.id, item.progress)
      } else {
        progress.delete(item.id)
      }
    }
  })
  void onPendingChanged((links) => queryClient.setQueryData(queryKeys.pending, links))
  void onDownloadProgress(({ id, progress: value }) => progress.set(id, value))
  void onCloseRequested(async () => {
    // Read again each time: the tray's setting may have changed since the window opened.
    const { keepInTray } = await getBackgroundOptions()
    if (keepInTray) return "hide"
    const items = queryClient.getQueryData<QueueItem[]>(queryKeys.queue) ?? []
    const running = items.filter((item) => item.state === "running").length
    if (running === 0) return "close"
    const confirmed = await confirmWithUser({
      title: strings.queue.closeTitle,
      message: strings.queue.closeMessage(running),
      confirmLabel: strings.queue.closeConfirm,
      cancelLabel: strings.dialogs.cancel,
    })
    return confirmed ? "close" : "stay"
  })
}

/** A running item's latest progress. */
export function useDownloadProgress(item: QueueItem): DownloadProgress | null {
  const live = progress.useValue(item.id)
  return item.state === "running" ? (live ?? item.progress) : null
}

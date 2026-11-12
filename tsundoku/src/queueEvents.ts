import type { QueryClient } from "@tanstack/react-query"
import {
  confirmWithUser,
  getBackgroundOptions,
  onCloseRequested,
  onDownloadProgress,
  onPendingChanged,
  onQueueChanged,
  onWatchedChanged,
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
    // A pending link or a watched item may now be downloaded already or queued.
    void queryClient.invalidateQueries({ queryKey: queryKeys.pending })
    void queryClient.invalidateQueries({ queryKey: queryKeys.watched })
    // A download that finished is in the history now. Only then is the history read again, since an open History
    // reads every page it has shown.
    const done = new Set(items.filter((item) => item.state === "done").map((item) => item.id))
    const finished = [...done].some((id) => !doneIds.has(id))
    doneIds = done
    if (finished) {
      void queryClient.invalidateQueries({ queryKey: queryKeys.history })
      void queryClient.invalidateQueries({ queryKey: queryKeys.historyFilters })
      void queryClient.invalidateQueries({ queryKey: queryKeys.videoDownloads })
      void queryClient.invalidateQueries({ queryKey: queryKeys.leftOutFiles })
      void queryClient.invalidateQueries({ queryKey: queryKeys.leftOutSharedFiles })
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
  void onWatchedChanged((sources) => queryClient.setQueryData(queryKeys.watched, sources))
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

/** A running item's latest progress, or that of the file the check window saves for a waiting one. */
export function useDownloadProgress(item: QueueItem): DownloadProgress | null {
  const live = progress.useValue(item.id)
  const saving = item.state === "waiting" && item.hold?.kind === "windowDownload"
  return item.state === "running" || saving ? (live ?? item.progress) : null
}

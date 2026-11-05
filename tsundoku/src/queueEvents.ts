import type { QueryClient } from "@tanstack/react-query"
import { confirmWithUser, onCloseRequested, onDownloadProgress, onQueueChanged, queryKeys } from "./api"
import type { DownloadProgress } from "./bindings/DownloadProgress"
import type { QueueItem } from "./bindings/QueueItem"
import { createLiveStore } from "./liveStore"
import { strings } from "./strings"

/*
  Keeps the UI's copy of the download queue current from the core's events: queue-changed replaces the items in
  React Query, and download-progress updates each running item's progress several times a second, outside it.
*/

const progress = createLiveStore<number, DownloadProgress>()

/** Starts listening to the queue's events, and asks before closing the window while downloads run. Called once,
 * before the first render (main.tsx). */
export function trackQueue(queryClient: QueryClient): void {
  void onQueueChanged((items) => {
    queryClient.setQueryData(queryKeys.queue, items)
    // A download that finished is in the history now.
    void queryClient.invalidateQueries({ queryKey: queryKeys.history })
    void queryClient.invalidateQueries({ queryKey: queryKeys.historyFilters })
    void queryClient.invalidateQueries({ queryKey: queryKeys.videoDownloads })
    for (const item of items) {
      if (item.progress) {
        progress.set(item.id, item.progress)
      } else {
        progress.delete(item.id)
      }
    }
  })
  void onDownloadProgress(({ id, progress: value }) => progress.set(id, value))
  void onCloseRequested(async () => {
    const items = queryClient.getQueryData<QueueItem[]>(queryKeys.queue) ?? []
    const running = items.filter((item) => item.state === "running").length
    if (running === 0) return true
    return confirmWithUser({
      title: strings.queue.closeTitle,
      message: strings.queue.closeMessage(running),
      confirmLabel: strings.queue.closeConfirm,
      cancelLabel: strings.dialogs.cancel,
    })
  })
}

/** A running item's latest progress. */
export function useDownloadProgress(item: QueueItem): DownloadProgress | null {
  const live = progress.useValue(item.id)
  return item.state === "running" ? (live ?? item.progress) : null
}

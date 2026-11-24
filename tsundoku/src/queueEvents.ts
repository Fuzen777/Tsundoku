import { hashKey, type QueryClient } from "@tanstack/react-query"
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
import type { ListArrangements } from "./bindings/ListArrangements"
import type { QueueItem } from "./bindings/QueueItem"
import type { WatchedSource } from "./bindings/WatchedSource"
import { forgetPickersBut } from "./keptPickers"
import { createLiveStore } from "./liveStore"
import { changeListArrangements } from "./queries"
import { combinedFraction } from "./queueSources"
import { strings } from "./strings"
import { checkFoundItems } from "./watchedSources"

/*
  Keeps the UI's copy of the download queue, the pending links and the watched sources current from the core's events:
  queue-changed, pending-changed and watched-changed replace them in React Query (a check that finds something also
  turns Watched to All New Items), and download-progress updates each running item's progress several times a second,
  outside it.
*/

const progress = createLiveStore<number, DownloadProgress>()

/** How long a download that just finished counts as just finished: its row's flash, and a little more. */
const JUST_FINISHED_MS = 1500
/** The downloads that finished in the last moment, while the app watched: their rows flash (`justFinished`). */
const justFinishedIds = new Set<number>()

/** Whether a download finished a moment ago, for its row or picture to flash once as it's drawn (DESIGN.md → Motion).
 * A row drawn again in another group (Done) flashes there. */
export function justFinished(id: number): boolean {
  return justFinishedIds.has(id)
}

/** Starts listening to the queue's events, and decides what closing the window does: with the tray on, it hides;
 * otherwise, it asks first while downloads run. Called once, before the first render (main.tsx). */
export function trackQueue(queryClient: QueryClient): void {
  let doneIds = new Set<number>()
  void onQueueChanged((items) => {
    // Those done now that weren't in the copy the UI had: none when it had none yet.
    const before = queryClient.getQueryData<QueueItem[]>(queryKeys.queue)
    if (before) {
      const doneBefore = new Set(before.filter((item) => item.state === "done").map((item) => item.id))
      for (const item of items) {
        if (item.state !== "done" || doneBefore.has(item.id)) continue
        justFinishedIds.add(item.id)
        setTimeout(() => justFinishedIds.delete(item.id), JUST_FINISHED_MS)
      }
    }
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
  void onPendingChanged((links) => {
    queryClient.setQueryData(queryKeys.pending, links)
    // A link that left Pending takes what its picker had ticked with it.
    forgetPickersBut(links.map((link) => link.id))
  })
  void onWatchedChanged((sources) => queryClient.setQueryData(queryKeys.watched, sources))
  showAllNewWhenFound(queryClient)
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

/**
 * Turns the Watched view to All New Items once a check finds something, whichever view shows, so what it found is in
 * one list to go through (Fuzen's choice). The choice is saved as the Show toggle's own is; Sources shows again when
 * picked. Every copy of the sources React Query gets (an event, a refetch) is compared with the one before it, so a
 * refetch can't hide a find; the first copy, read when the app starts, is compared with nothing.
 */
function showAllNewWhenFound(queryClient: QueryClient): void {
  const watchedHash = hashKey(queryKeys.watched)
  let before: WatchedSource[] | undefined
  queryClient.getQueryCache().subscribe((event) => {
    if (event.type !== "updated" || event.query.queryHash !== watchedHash) return
    const after = event.query.state.data as WatchedSource[] | undefined
    if (after === undefined || after === before) return
    const found = before !== undefined && checkFoundItems(before, after)
    before = after
    const shown = queryClient.getQueryData<ListArrangements>(queryKeys.listArrangements)?.watched.show
    if (!found || shown === "allNew") return
    void changeListArrangements(queryClient, (current) => ({
      ...current,
      watched: { ...current.watched, show: "allNew" },
    })).catch(() => {
      // Saving failed and the toggle stayed on Sources, where the new items show on their cards anyway.
    })
  })
}

/** A running item's latest progress, or that of the file the check window saves for a waiting one. */
export function useDownloadProgress(item: QueueItem): DownloadProgress | null {
  const live = progress.useValue(item.id)
  const saving = item.state === "waiting" && item.hold?.kind === "windowDownload"
  return item.state === "running" || saving ? (live ?? item.progress) : null
}

/** How far these downloads have come together (`combinedFraction`), following the running ones' progress. */
export function useCombinedProgress(items: readonly QueueItem[]): number {
  return progress.useComputed((latest) =>
    combinedFraction(items, (item) => (item.state === "running" ? (latest.get(item.id) ?? item.progress) : null)),
  )
}

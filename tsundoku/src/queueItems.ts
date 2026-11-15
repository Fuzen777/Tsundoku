import {
  confirmWithUser,
  copyText,
  getDownloadDetails,
  pickFolder,
  removeFinishedDownload,
  restoreClearedDownloads,
  setQueueFolder,
} from "./api"
import type { DownloadProgress } from "./bindings/DownloadProgress"
import type { QueueItem } from "./bindings/QueueItem"
import { describeDownloadError, describeHistoryError, type ErrorDescription } from "./errors"
import { formatBytes, formatDuration, formatSpeed } from "./format"
import { strings } from "./strings"
import { useOfferUndo } from "./undo"

/*
  What a download in the Queue says and does, shared by its row (components/QueueRow.tsx) and its picture in Masonry
  and Gallery (components/QueueTiles.tsx).
*/

/** What a download's actions run through: each one's error is shown on it, and removing it offers to undo. */
export function useItemActions(item: QueueItem, setError: (error: ErrorDescription | null) => void) {
  const offerUndo = useOfferUndo()
  const act = (action: (id: number) => Promise<void>, describe = describeDownloadError) => {
    setError(null)
    action(item.id).catch((error: unknown) => setError(describe(error)))
  }
  /** Remove from Queue, then an offer for a few seconds to put it back where it was. */
  const removeWithUndo = async (id: number) => {
    await removeFinishedDownload(id)
    offerUndo({
      message: strings.queue.removed(item.video.title),
      undo: restoreClearedDownloads,
      describeError: describeDownloadError,
    })
  }
  const actOnFile = (action: (downloadId: number) => Promise<void>) => {
    if (!item.result) return
    setError(null)
    action(item.result.downloadId).catch((error: unknown) => setError(describeHistoryError(error)))
  }
  return { act, removeWithUndo, actOnFile }
}

/** Puts a failed download's details on the clipboard, as Copy Details does. Rejects with a DownloadError. */
export async function copyDownloadDetails(id: number, error: ErrorDescription): Promise<void> {
  await copyText(await getDownloadDetails(id, { message: error.message, detail: error.detail ?? null }))
}

/** Change Folder…: asks for a folder, then saves the download there. */
export async function changeFolder(id: number, item: QueueItem): Promise<void> {
  const folder = await pickFolder(strings.queue.changeFolderTitle)
  if (folder) await moveToFolder(id, item, folder)
}

/** Saves the download in `folder`, or the usual one, asking first when a stopped one would lose what it downloaded. */
export async function moveToFolder(id: number, item: QueueItem, folder: string | null): Promise<void> {
  const startsOver = item.state === "stopped"
  if (
    startsOver &&
    !(await confirmWithUser({
      title: strings.queue.changeFolderStoppedTitle,
      message: strings.queue.changeFolderStoppedMessage,
      confirmLabel: strings.queue.changeFolderConfirm,
      cancelLabel: strings.dialogs.cancel,
    }))
  ) {
    return
  }
  await setQueueFolder(id, folder)
}

/** What the item is doing: its state, or while it runs, the stage yt-dlp is at. */
export function stageLabel(item: QueueItem, progress: DownloadProgress | null): string {
  const stages = strings.queue.stages
  return item.state === "running" ? stages[progress?.stage ?? "starting"] : stages[item.state]
}

/** Size, speed and time left while downloading; the file's size once done. */
export function sizeDetails(item: QueueItem, progress: DownloadProgress | null): (string | null)[] {
  if (item.state === "done") {
    return [item.result?.fileSize ? formatBytes(item.result.fileSize) : null]
  }
  if (progress?.stage === "countdown") {
    return [progress.eta === null ? null : strings.queue.timeLeft(formatDuration(progress.eta))]
  }
  if (!progress || progress.totalBytes === null) return []
  return [
    strings.queue.progress(formatBytes(progress.downloadedBytes ?? 0), formatBytes(progress.totalBytes)),
    progress.speed ? formatSpeed(progress.speed) : null,
    progress.eta !== null && progress.fraction !== 1 ? strings.queue.timeLeft(formatDuration(progress.eta)) : null,
  ]
}

/** Whether an event happened on a button, field, link, menu or log, which keep their own clicks: a double-click there
 * doesn't open the download's file. */
export function onAControl(target: EventTarget): boolean {
  return target instanceof Element && target.closest("button, a, input, textarea, select, pre, [role='menu']") !== null
}

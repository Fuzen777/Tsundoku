import type { DownloadProgress } from "./bindings/DownloadProgress"
import type { DownloadStage } from "./bindings/DownloadStage"
import type { QueueItem } from "./bindings/QueueItem"
import type { QueueState } from "./bindings/QueueState"
import { joinDetails } from "./format"
import { canTryNow } from "./holds"
import { strings } from "./strings"

/*
  The downloads picked together from one channel, playlist or account (QueueItem.source), which the Queue keeps
  together under the source's own heading (components/QueueSourceGroup.tsx).
*/

/** The source a download was picked from, by its page, or null for one added on its own. */
export function sourceKey(item: QueueItem): string | null {
  return item.source?.url ?? null
}

/** How many downloads are in each state. */
export function stateCounts(items: readonly QueueItem[]): Record<QueueState, number> {
  const counts: Record<QueueState, number> = { running: 0, waiting: 0, stopped: 0, failed: 0, done: 0 }
  for (const item of items) counts[item.state] += 1
  return counts
}

/** The states a source's heading counts after how many are done, in this order, each left out at zero. */
const COUNTED: readonly Exclude<QueueState, "done">[] = ["running", "waiting", "stopped", "failed"]

/**
 * Where a source's downloads stand: "3 of 12 done · 2 downloading · 7 waiting", or only "12 done" or "3 waiting" when
 * they all stand in one place (grouped by status, say).
 */
export function sourceSummary(items: readonly QueueItem[]): string {
  const counts = stateCounts(items)
  const text = strings.queue.source
  const counted = COUNTED.filter((state) => counts[state] > 0)
  if (counts.done === items.length) return text.allDone(items.length)
  if (counts.done === 0 && counted.length === 1) return text.states[counted[0]](items.length)
  return joinDetails([text.done(counts.done, items.length), ...counted.map((state) => text.states[state](counts[state]))])
}

/** The stages after a download's files are in, while the tools put them together: they give no progress of their own. */
const FINISHING_STAGES: ReadonlySet<DownloadStage> = new Set([
  "merging",
  "converting",
  "embedding",
  "processing",
  "finishing",
  "unpacking",
])

/**
 * How far the downloads have come together, from 0 to 1: a finished one counts whole, and so does one past downloading
 * (merging, embedding…), so the bar doesn't slip back; a running one by its progress (`progressOf`; none known yet
 * counts as nothing); the rest not at all.
 */
export function combinedFraction(
  items: readonly QueueItem[],
  progressOf: (item: QueueItem) => DownloadProgress | null | undefined,
): number {
  if (items.length === 0) return 0
  let sum = 0
  for (const item of items) {
    if (item.state === "done") {
      sum += 1
    } else if (item.state === "running") {
      const progress = progressOf(item)
      sum += progress && FINISHING_STAGES.has(progress.stage) ? 1 : Math.min(Math.max(progress?.fraction ?? 0, 0), 1)
    }
  }
  return sum / items.length
}

/** What a download's picture offers in its right-click menu, in the Queue's Masonry and Gallery: the buttons its row
 * shows, then its details, then what the row's More Actions holds, and Cancel or Remove from Queue last. */
export type TileAction =
  | "stop"
  | "tryNow"
  | "openCheck"
  | "enterPassword"
  | "resume"
  | "retry"
  | "openFile"
  | "showInFolder"
  | "showLog"
  | "copyDetails"
  | "details"
  | "downloadNext"
  | "changeFolder"
  | "usualFolder"
  | "remove"
  | "cancel"

/** The actions a download's picture offers, in the order its menu lists them: those its row would show. */
export function tileActions(item: QueueItem): TileAction[] {
  const { state } = item
  const hold = state === "waiting" ? item.hold : null
  const movable = state === "waiting" || state === "stopped" || state === "failed"
  const actions: (TileAction | false)[] = [
    (state === "running" || state === "waiting") && "stop",
    canTryNow(hold) && "tryNow",
    hold?.kind === "browserCheck" && "openCheck",
    hold?.kind === "needsPassword" && "enterPassword",
    state === "stopped" && "resume",
    state === "failed" && "retry",
    state === "done" && "openFile",
    state === "done" && "showInFolder",
    state !== "waiting" && "showLog",
    state === "failed" && item.error !== null && "copyDetails",
    "details",
    state === "waiting" && "downloadNext",
    movable && "changeFolder",
    movable && item.folderChosen && "usualFolder",
    state === "done" ? "remove" : "cancel",
  ]
  return actions.filter((action): action is TileAction => action !== false)
}

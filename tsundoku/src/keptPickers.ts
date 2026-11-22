import type { KeptTicks } from "./listingPicker"

/**
 * What the listing picker had ticked, with its changes for Undo, for each pending link it was opened from (Pick What to
 * Download), so picking from the link again carries on from there. Kept in memory while the app runs and the link waits
 * in Pending: `forgetPickersBut` drops the others whenever Pending changes.
 */
const kept = new Map<number, KeptTicks>()

export function keepPicker(pendingId: number, ticks: KeptTicks): void {
  kept.set(pendingId, ticks)
}

export function keptPicker(pendingId: number): KeptTicks | undefined {
  return kept.get(pendingId)
}

/** Forgets what was kept for the links no longer in Pending. */
export function forgetPickersBut(pendingIds: Iterable<number>): void {
  const waiting = new Set(pendingIds)
  for (const id of [...kept.keys()]) {
    if (!waiting.has(id)) kept.delete(id)
  }
}

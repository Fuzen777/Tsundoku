import { useSyncExternalStore } from "react"
import type { ItemKey } from "./bindings/ItemKey"
import type { Listing } from "./bindings/Listing"
import type { KeptTicks } from "./listingPicker"

/**
 * What the listing picker had for each pending link it was opened from (Pick What to Download), so picking from the link
 * again carries on from there, at once: what's ticked, with its changes for Undo, and the listing as it stood, every item
 * loaded included (`listing.items`, with `more` and `total` as they were). Several links can each have theirs, while one
 * picker shows at a time; Pending's Needs Attention shows the others with their picture and name (`useKeptPicker`).
 * Kept in memory while the app runs and the link waits in Pending: `forgetPickersBut` drops the others whenever Pending
 * changes.
 */
export interface KeptPicker {
  ticks: KeptTicks | null
  listing: Listing | null
  /** The listing's items skipped before (`LinkPreview`'s), with those its picker's downloads skipped since. */
  skipped: readonly ItemKey[]
}

const kept = new Map<number, KeptPicker>()
const listeners = new Set<() => void>()

function change(pendingId: number, update: Partial<KeptPicker>): void {
  kept.set(pendingId, { ticks: null, listing: null, skipped: [], ...kept.get(pendingId), ...update })
  for (const listener of listeners) listener()
}

export function keepPicker(pendingId: number, ticks: KeptTicks): void {
  change(pendingId, { ticks })
}

/** Keeps the listing as the picker has it now: its items so far, where Load More carries on, and what's skipped. */
export function keepListing(pendingId: number, listing: Listing, skipped: readonly ItemKey[]): void {
  change(pendingId, { listing, skipped })
}

export function keptPicker(pendingId: number): KeptPicker | undefined {
  return kept.get(pendingId)
}

/** What's kept for a pending link, kept current as its picker changes; nothing for `null`. */
export function useKeptPicker(pendingId: number | null): KeptPicker | undefined {
  return useSyncExternalStore(subscribe, () => (pendingId === null ? undefined : kept.get(pendingId)))
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

/** Forgets what was kept for the links no longer in Pending. */
export function forgetPickersBut(pendingIds: Iterable<number>): void {
  const waiting = new Set(pendingIds)
  let forgot = false
  for (const id of [...kept.keys()]) {
    if (!waiting.has(id)) {
      kept.delete(id)
      forgot = true
    }
  }
  if (forgot) for (const listener of listeners) listener()
}

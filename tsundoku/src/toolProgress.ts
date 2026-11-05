import { useSyncExternalStore } from "react"
import { onToolProgress } from "./api"
import type { ToolId } from "./bindings/ToolId"
import type { ToolStage } from "./bindings/ToolStage"

/*
  The current stage of each tool being installed or updated, from the core's progress events. Kept outside React
  Query since it streams, and shared by the setup screen and Settings, so an update started in the background shows
  wherever the tool is.
*/

const stages = new Map<ToolId, ToolStage>()
const listeners = new Set<() => void>()

/** Starts listening to the core's progress events. Called once, before the first render (main.tsx). */
export function trackToolProgress(): void {
  void onToolProgress(({ id, stage }) => {
    if (stage) {
      stages.set(id, stage)
    } else {
      stages.delete(id)
    }
    for (const listener of listeners) listener()
  })
}

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

/** The tool's current stage, or undefined when nothing is running for it. */
export function useToolStage(id: ToolId): ToolStage | undefined {
  return useSyncExternalStore(subscribe, () => stages.get(id))
}

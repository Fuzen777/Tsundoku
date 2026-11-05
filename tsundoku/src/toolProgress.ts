import { onToolProgress } from "./api"
import type { ToolId } from "./bindings/ToolId"
import type { ToolStage } from "./bindings/ToolStage"
import { createLiveStore } from "./liveStore"

/*
  The current stage of each tool being installed or updated, from the core's progress events. Shared by the setup
  screen and Settings, so an update started in the background shows wherever the tool is.
*/

const stages = createLiveStore<ToolId, ToolStage>()

/** Starts listening to the core's progress events. Called once, before the first render (main.tsx). */
export function trackToolProgress(): void {
  void onToolProgress(({ id, stage }) => {
    if (stage) {
      stages.set(id, stage)
    } else {
      stages.delete(id)
    }
  })
}

/** The tool's current stage, or undefined when nothing is running for it. */
export function useToolStage(id: ToolId): ToolStage | undefined {
  return stages.useValue(id)
}

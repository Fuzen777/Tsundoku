import { describe, expect, it } from "vitest"
import type { QueueItem } from "./bindings/QueueItem"
import type { QueueState } from "./bindings/QueueState"
import { shownStage } from "./queueItems"
import { strings } from "./strings"

const stages = strings.queue.stages
const item = (state: QueueState) => ({ state }) as QueueItem

describe("shownStage", () => {
  const byStatus = { byStatus: true }

  it("leaves out what the status's heading says", () => {
    expect(shownStage(stages.waiting, item("waiting"), byStatus)).toBeNull()
    expect(shownStage(stages.done, item("done"), byStatus)).toBeNull()
    expect(shownStage(stages.downloading, item("running"), byStatus)).toBeNull()
    expect(shownStage(stages.downloadingVideo, item("running"), byStatus)).toBeNull()
    expect(shownStage(stages.downloadingAudio, item("running"), byStatus)).toBeNull()
  })

  it("keeps a stage that says more than the heading", () => {
    expect(shownStage(stages.merging, item("running"), byStatus)).toBe(stages.merging)
    expect(shownStage(stages.starting, item("running"), byStatus)).toBe(stages.starting)
    // The check window saving the file, while the download waits.
    expect(shownStage(stages.downloading, item("waiting"), byStatus)).toBe(stages.downloading)
  })

  it("keeps every stage when the list isn't grouped by status", () => {
    expect(shownStage(stages.downloadingVideo, item("running"), { byStatus: false })).toBe(stages.downloadingVideo)
    expect(shownStage(stages.waiting, item("waiting"), { byStatus: false })).toBe(stages.waiting)
  })
})

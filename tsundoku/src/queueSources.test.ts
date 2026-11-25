import { describe, expect, it } from "vitest"
import type { QueueItem } from "./bindings/QueueItem"
import type { QueueState } from "./bindings/QueueState"
import type { Hold } from "./bindings/Hold"
import { combinedFraction, sourceKey, sourceSummary, tileActions } from "./queueSources"

function item(id: number, state: QueueState, fraction: number | null = null): QueueItem {
  return {
    id,
    state,
    source: null,
    progress:
      fraction === null
        ? null
        : { stage: "downloadingVideo", fraction, downloadedBytes: null, totalBytes: null, speed: null, eta: null },
  } as QueueItem
}

describe("sourceKey", () => {
  it("tells sources apart by their page, and a download added on its own has none", () => {
    const nasa = {
      url: "https://www.youtube.com/@NASA/videos",
      kind: "channel",
      name: "NASA",
      siteName: "youtube.com",
      thumbnailUrl: null,
    } as const
    expect(sourceKey({ ...item(1, "waiting"), source: nasa })).toBe("https://www.youtube.com/@NASA/videos")
    expect(sourceKey(item(2, "waiting"))).toBeNull()
  })
})

describe("sourceSummary", () => {
  it("says how many are done, then the others standing elsewhere, leaving out the states none is in", () => {
    const items = [item(1, "done"), item(2, "running"), item(3, "waiting"), item(4, "waiting"), item(5, "failed")]
    expect(sourceSummary(items)).toBe("1 of 5 done · 1 downloading · 2 waiting · 1 failed")
    expect(sourceSummary([item(1, "stopped"), item(2, "waiting")])).toBe("0 of 2 done · 1 waiting · 1 stopped")
  })

  it("only counts them when they all stand in one place", () => {
    expect(sourceSummary([item(1, "done"), item(2, "done")])).toBe("2 done")
    expect(sourceSummary([item(1, "waiting"), item(2, "waiting"), item(3, "waiting")])).toBe("3 waiting")
    expect(sourceSummary([item(1, "failed")])).toBe("1 failed")
  })

  it("only says how many there are under their status's heading, which says where they stand", () => {
    expect(sourceSummary([item(1, "done"), item(2, "done")], true)).toBe("2 downloads")
    expect(sourceSummary([item(1, "waiting")], true)).toBe("1 download")
    expect(sourceSummary([item(1, "done"), item(2, "waiting")], true)).toBe("1 of 2 done · 1 waiting")
  })
})

describe("combinedFraction", () => {
  const progressOf = (entry: QueueItem) => entry.progress

  it("counts a finished download whole and a running one by its progress", () => {
    const items = [item(1, "done"), item(2, "running", 0.5), item(3, "waiting"), item(4, "failed")]
    expect(combinedFraction(items, progressOf)).toBe(0.375)
  })

  it("counts a running one past downloading whole, since merging or embedding gives no progress", () => {
    const embedding = {
      ...item(2, "running"),
      progress: { stage: "embedding", fraction: null, downloadedBytes: null, totalBytes: null, speed: null, eta: null },
    } as QueueItem
    expect(combinedFraction([item(1, "done"), embedding], progressOf)).toBe(1)
  })

  it("counts a running one without progress yet as nothing, and keeps progress within bounds", () => {
    expect(combinedFraction([item(1, "running"), item(2, "running", 1.4)], progressOf)).toBe(0.5)
    expect(combinedFraction([], progressOf)).toBe(0)
  })

  it("ignores the progress a download that isn't running still carries", () => {
    expect(combinedFraction([item(1, "stopped", 0.9), item(2, "done")], progressOf)).toBe(0.5)
  })
})

describe("tileActions", () => {
  const withHold = (hold: Hold): QueueItem => ({ ...item(1, "waiting"), hold, folderChosen: false }) as QueueItem
  const plain = (state: QueueState, more: Partial<QueueItem> = {}): QueueItem =>
    ({ ...item(1, state), hold: null, error: null, folderChosen: false, ...more }) as QueueItem

  it("offers a picture the buttons its row shows, its details, More Actions, then Cancel or Remove", () => {
    expect(tileActions(plain("running"))).toEqual(["stop", "showLog", "copyLink", "details", "cancel"])
    expect(tileActions(plain("waiting"))).toEqual([
      "stop",
      "copyLink",
      "details",
      "downloadNext",
      "changeFolder",
      "cancel",
    ])
    expect(tileActions(plain("stopped", { folderChosen: true }))).toEqual([
      "resume",
      "showLog",
      "copyLink",
      "details",
      "changeFolder",
      "usualFolder",
      "cancel",
    ])
    expect(tileActions(plain("done"))).toEqual([
      "openFile",
      "showInFolder",
      "showLog",
      "copyLink",
      "details",
      "remove",
    ])
  })

  it("offers Copy Details only for a failure it can describe", () => {
    const error = { kind: "cancelled" } as unknown as QueueItem["error"]
    expect(tileActions(plain("failed", { error }))).toEqual([
      "retry",
      "showLog",
      "copyDetails",
      "copyLink",
      "details",
      "changeFolder",
      "cancel",
    ])
    expect(tileActions(plain("failed"))).not.toContain("copyDetails")
  })

  it("offers what a hold waits for: Try Now, the check, or the password", () => {
    const tryNow = tileActions(withHold({ kind: "notEnoughRoom", drive: "D:", free: 0, needed: 1, others: 0, until: 0 }))
    expect(tryNow.slice(0, 2)).toEqual(["stop", "tryNow"])
    expect(tileActions(withHold({ kind: "browserCheck", site: "example.com", files: 1 }))).toContain("openCheck")
    const password = tileActions(withHold({ kind: "needsPassword", wrong: false, tried: 0 }))
    expect(password).toContain("enterPassword")
    expect(password).not.toContain("tryNow")
  })
})

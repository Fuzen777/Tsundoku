import { describe, expect, it } from "vitest"
import { describeHistoryError, describePreviewError, describeToolError, isCancelled } from "./errors"
import { strings } from "./strings"

describe("describeToolError", () => {
  it("names the tool and keeps the cause as the detail", () => {
    expect(describeToolError({ kind: "wontRun", detail: "The system cannot execute it." }, "ffmpeg")).toEqual({
      message: "ffmpeg won't run.",
      detail: "The system cannot execute it.",
    })
  })

  it("drops an empty detail", () => {
    expect(describeToolError({ kind: "notThisTool", output: "" }, "deno").detail).toBeUndefined()
  })

  it("treats anything that isn't a core error as unexpected", () => {
    const description = describeToolError(new Error("invoke failed"), "ytDlp")
    expect(description.message).toBe(strings.errors.unexpected)
    expect(description.detail).toBe("Error: invoke failed")
  })
})

describe("describePreviewError", () => {
  it("keeps yt-dlp's error as the detail", () => {
    const description = describePreviewError({ kind: "unavailable", detail: "[youtube] x: This video is unavailable" })
    expect(description).toEqual({
      message: strings.preview.errors.unavailable,
      detail: "[youtube] x: This video is unavailable",
    })
  })

  it("describes yt-dlp failing to run as a tool error", () => {
    expect(describePreviewError({ kind: "tool", error: { kind: "blocked" } }).message).toBe(
      strings.tools.errors.blocked("yt-dlp"),
    )
  })
})

describe("describeHistoryError", () => {
  it("shows a missing file's path", () => {
    expect(describeHistoryError({ kind: "fileMissing", path: String.raw`D:\Videos\a.mkv` })).toEqual({
      message: strings.history.errors.fileMissing,
      detail: String.raw`D:\Videos\a.mkv`,
    })
  })
})

describe("isCancelled", () => {
  it("only matches the core's cancellation", () => {
    expect(isCancelled({ kind: "cancelled" })).toBe(true)
    expect(isCancelled({ kind: "busy" })).toBe(false)
    expect(isCancelled(new Error("cancelled"))).toBe(false)
  })
})

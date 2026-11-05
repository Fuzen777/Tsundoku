import { describe, expect, it } from "vitest"
import { strings } from "./strings"
import { describeToolError, isCancelled } from "./toolErrors"

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

  it("treats anything that isn't a tool error as unexpected", () => {
    const description = describeToolError(new Error("invoke failed"), "ytDlp")
    expect(description.message).toBe(strings.tools.errors.unexpected)
    expect(description.detail).toBe("Error: invoke failed")
  })
})

describe("isCancelled", () => {
  it("only matches the core's cancellation", () => {
    expect(isCancelled({ kind: "cancelled" })).toBe(true)
    expect(isCancelled({ kind: "busy" })).toBe(false)
    expect(isCancelled(new Error("cancelled"))).toBe(false)
  })
})

import { describe, expect, it } from "vitest"
import type { Hold } from "./bindings/Hold"
import { describeHold, holdEnd } from "./holds"

const GB = 1024 ** 3

describe("a download without room on its drive", () => {
  const hold: Hold = { kind: "notEnoughRoom", drive: "D:", free: 3.1 * GB, needed: 14 * GB, others: 0, until: 1 }

  it("says what the drive has and what the download needs", () => {
    expect(describeHold(hold)).toBe("D: has 3.1 GB free, and this needs 14.0 GB. It starts by itself once there's room.")
    expect(describeHold({ ...hold, free: 8 * GB, needed: 4 * GB, others: 5 * GB })).toBe(
      "D: has 8.0 GB free, and this needs 4.0 GB besides the 5.0 GB the downloads running there still need. It starts by itself once there's room.",
    )
  })

  it("has no countdown, since it starts once there's room", () => {
    expect(holdEnd(hold)).toBeNull()
    expect(holdEnd({ kind: "slowDown", site: "YouTube", until: 5 })).toBe(5)
  })
})

describe("a download behind a check", () => {
  it("says it waits for you, without a countdown", () => {
    const check: Hold = { kind: "browserCheck", site: "MediaFire", files: 1 }
    expect(describeHold(check)).toBe(
      "MediaFire wants to check you're a person before this download. It waits here until you open the check and start the download on MediaFire's page.",
    )
    expect(describeHold({ ...check, files: 3 })).toContain("before 3 of these files")
    expect(holdEnd(check)).toBeNull()
  })
})

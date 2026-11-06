import { describe, expect, it } from "vitest"
import { checkingTime } from "./linkFiles"

describe("checking time", () => {
  it("is in minutes under an hour, else in half hours", () => {
    expect(checkingTime(1)).toBe("about a minute")
    expect(checkingTime(301)).toBe("about 13 minutes")
    expect(checkingTime(1440)).toBe("about an hour")
    expect(checkingTime(2134)).toBe("about 1.5 hours")
  })
})

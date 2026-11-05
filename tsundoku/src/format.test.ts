import { describe, expect, it } from "vitest"
import { formatBytes, formatCalendarDate, formatDay, formatDuration, joinDetails } from "./format"

describe("formatBytes", () => {
  it("keeps bytes whole", () => {
    expect(formatBytes(0)).toBe("0 B")
    expect(formatBytes(1023)).toBe("1023 B")
  })

  it("uses 1024-based units with one decimal", () => {
    expect(formatBytes(1024)).toBe("1.0 KB")
    expect(formatBytes(23_575_744)).toBe("22.5 MB")
    expect(formatBytes(90_993_957)).toBe("86.8 MB")
    expect(formatBytes(5 * 1024 ** 3)).toBe("5.0 GB")
  })
})

describe("formatDuration", () => {
  it("shows minutes and seconds under an hour", () => {
    expect(formatDuration(0)).toBe("0:00")
    expect(formatDuration(19)).toBe("0:19")
    expect(formatDuration(143.206)).toBe("2:23")
    expect(formatDuration(3599)).toBe("59:59")
  })

  it("adds hours from an hour up", () => {
    expect(formatDuration(3600)).toBe("1:00:00")
    expect(formatDuration(3723)).toBe("1:02:03")
  })
})

describe("dates", () => {
  it("shows a calendar date on the same day in every time zone", () => {
    expect(formatCalendarDate("2005-04-24", "en-US")).toBe("Apr 24, 2005")
  })

  it("shows the day of a moment in the given time zone", () => {
    // 2026-11-05 01:00 UTC is still November 4 in New York.
    expect(formatDay(1793840400, "en-US", "UTC")).toBe("Nov 5, 2026")
    expect(formatDay(1793840400, "en-US", "America/New_York")).toBe("Nov 4, 2026")
  })
})

describe("joinDetails", () => {
  it("skips what's missing", () => {
    expect(joinDetails(["jawed", null, "youtube.com", undefined, false])).toBe("jawed · youtube.com")
    expect(joinDetails([])).toBe("")
  })
})

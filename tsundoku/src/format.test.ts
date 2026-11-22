import { describe, expect, it } from "vitest"
import {
  formatBytes,
  formatCalendarDate,
  formatClockTime,
  formatContents,
  formatDay,
  formatDuration,
  formatFileDate,
  formatMonth,
  formatRoughAge,
  formatSpeed,
  joinDetails,
} from "./format"

describe("formatFileDate", () => {
  it("writes the local day with leading zeros", () => {
    expect(formatFileDate(new Date(2026, 10, 6, 23, 59))).toBe("2026-11-06")
    expect(formatFileDate(new Date(2027, 0, 1))).toBe("2027-01-01")
  })
})

describe("formatMonth", () => {
  it("writes a month in words", () => {
    expect(formatMonth("2026-11", "en-US")).toBe("November 2026")
  })

  it("leaves text that isn't a month as it is, rather than failing", () => {
    expect(formatMonth("", "en-US")).toBe("")
  })
})

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

describe("formatSpeed", () => {
  it("shows bytes per second", () => {
    expect(formatSpeed(5_452_595.4)).toBe("5.2 MB/s")
    expect(formatSpeed(512)).toBe("512 B/s")
  })
})

describe("formatRoughAge", () => {
  const now = Date.parse("2026-11-06T15:00:00Z")
  const age = (isoDate: string) => formatRoughAge(isoDate, "en-US", now)

  it("says how long ago, as roughly as the date is", () => {
    expect(age("2026-11-06")).toBe("today")
    expect(age("2026-11-04")).toBe("2 days ago")
    expect(age("2026-10-16")).toBe("3 weeks ago")
    expect(age("2026-09-06")).toBe("2 months ago")
    expect(age("2025-11-01")).toBe("last year")
    expect(age("2013-11-07")).toBe("13 years ago")
  })

  it("takes a day ahead as today", () => {
    expect(age("2026-11-08")).toBe("today")
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

describe("formatContents", () => {
  it("counts a post's images and videos", () => {
    expect(formatContents({ post: { images: 2, videos: 1, others: 0 }, files: null })).toBe("2 images, 1 video")
    expect(formatContents({ post: { images: 0, videos: 0, others: 1 }, files: null })).toBe("1 file")
  })

  it("shows one file's size, or a folder's count and size", () => {
    expect(formatContents({ post: null, files: { count: 1, bytes: 23_592_960 } })).toBe("22.5 MB")
    expect(formatContents({ post: null, files: { count: 14, bytes: 1_288_490_189 } })).toBe("14 files, 1.2 GB")
  })

  it("has nothing to say about a video", () => {
    expect(formatContents({ post: null, files: null })).toBeNull()
  })
})

describe("formatClockTime", () => {
  // 2026-11-06 14:32 UTC.
  const moment = Date.UTC(2026, 10, 6, 14, 32)

  it("gives the time of a moment later today", () => {
    expect(formatClockTime(moment, "en-GB", "UTC", Date.UTC(2026, 10, 6, 14, 0))).toBe("14:32")
    expect(formatClockTime(moment, "en-US", "UTC", Date.UTC(2026, 10, 6, 9, 0))).toBe("2:32 PM")
  })

  it("names the day of a moment on another day", () => {
    expect(formatClockTime(moment, "en-GB", "UTC", Date.UTC(2026, 10, 5, 23, 0))).toBe("Fri 14:32")
  })
})

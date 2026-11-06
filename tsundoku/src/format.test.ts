import { describe, expect, it } from "vitest"
import {
  formatBytes,
  formatCalendarDate,
  formatContents,
  formatDay,
  formatDuration,
  formatFileDate,
  formatSpeed,
  joinDetails,
} from "./format"

describe("formatFileDate", () => {
  it("writes the local day with leading zeros", () => {
    expect(formatFileDate(new Date(2026, 10, 6, 23, 59))).toBe("2026-11-06")
    expect(formatFileDate(new Date(2027, 0, 1))).toBe("2027-01-01")
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
    expect(formatContents({ post: { images: 2, videos: 1 }, files: null })).toBe("2 images, 1 video")
  })

  it("shows one file's size, or a folder's count and size", () => {
    expect(formatContents({ post: null, files: { count: 1, bytes: 23_592_960 } })).toBe("22.5 MB")
    expect(formatContents({ post: null, files: { count: 14, bytes: 1_288_490_189 } })).toBe("14 files, 1.2 GB")
  })

  it("has nothing to say about a video", () => {
    expect(formatContents({ post: null, files: null })).toBeNull()
  })
})

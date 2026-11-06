import { describe, expect, it } from "vitest"
import { dateBounds } from "./historyDates"

/** Seconds since 1970 at a local date and time. */
const at = (year: number, month: number, day: number, hours = 0) =>
  Math.floor(new Date(year, month - 1, day, hours).getTime() / 1000)

describe("dateBounds", () => {
  const now = new Date(2026, 10, 6, 15, 30)

  it("starts each range at a midnight, counting today", () => {
    expect(dateBounds("any", now)).toEqual({ from: null, until: null })
    expect(dateBounds("today", now)).toEqual({ from: at(2026, 11, 6), until: null })
    expect(dateBounds("week", now)).toEqual({ from: at(2026, 10, 31), until: null })
    expect(dateBounds("month", now)).toEqual({ from: at(2026, 10, 8), until: null })
    expect(dateBounds("year", now)).toEqual({ from: at(2025, 11, 6), until: null })
  })

  it("takes both days of a custom range", () => {
    expect(dateBounds("custom", now, "2026-01-01", "2026-01-31")).toEqual({
      from: at(2026, 1, 1),
      until: at(2026, 2, 1),
    })
  })

  it("leaves an empty custom date unbounded", () => {
    expect(dateBounds("custom", now, "", "2026-01-31")).toEqual({ from: null, until: at(2026, 2, 1) })
    expect(dateBounds("custom", now, "2026-01-01", "")).toEqual({ from: at(2026, 1, 1), until: null })
    expect(dateBounds("custom", now, "not a date", "")).toEqual({ from: null, until: null })
  })
})

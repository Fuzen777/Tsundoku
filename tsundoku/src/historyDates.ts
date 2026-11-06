/** How far back the history view looks, by its last download's date. */
export type DateRange = "any" | "today" | "week" | "month" | "year" | "custom"

export const DATE_RANGES: readonly DateRange[] = ["any", "today", "week", "month", "year", "custom"]

/** From and until, in seconds since 1970, null for no bound. */
export interface DateBounds {
  from: number | null
  until: number | null
}

/**
 * The bounds of a range, in the reader's own time zone, worked out when the range is picked (the history's query
 * keeps them, so they don't move while the view is open). Each starts at a midnight: Today at today's, Past 7 Days at
 * the one 6 days before, so it holds 7 days with today. Custom takes two calendar dates ("2026-11-06"), both days
 * included, either left empty for no bound.
 */
export function dateBounds(range: DateRange, now: Date, customFrom = "", customTo = ""): DateBounds {
  const midnight = (date: Date) => new Date(date.getFullYear(), date.getMonth(), date.getDate())
  const seconds = (date: Date) => Math.floor(date.getTime() / 1000)
  const daysBefore = (days: number) => {
    const day = midnight(now)
    day.setDate(day.getDate() - days)
    return { from: seconds(day), until: null }
  }
  switch (range) {
    case "any":
      return { from: null, until: null }
    case "today":
      return daysBefore(0)
    case "week":
      return daysBefore(6)
    case "month":
      return daysBefore(29)
    case "year": {
      const day = midnight(now)
      day.setFullYear(day.getFullYear() - 1)
      return { from: seconds(day), until: null }
    }
    case "custom": {
      const from = calendarDay(customFrom)
      const to = calendarDay(customTo)
      if (to) to.setDate(to.getDate() + 1)
      return { from: from && seconds(from), until: to && seconds(to) }
    }
  }
}

/** Midnight at the start of a "YYYY-MM-DD" date, in the reader's time zone, or null for anything else. */
function calendarDay(text: string): Date | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(text)
  if (!match) return null
  return new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3]))
}

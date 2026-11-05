import { strings } from "./strings"

/** A size in bytes as Windows shows it: in 1024-based units, with one decimal from KB up ("22.5 MB"). */
export function formatBytes(bytes: number): string {
  const units = strings.units.bytes
  let value = bytes
  let unitIndex = 0
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024
    unitIndex += 1
  }
  return unitIndex === 0 ? `${value} ${units[0]}` : `${value.toFixed(1)} ${units[unitIndex]}`
}

/** A download speed: "5.2 MB/s". */
export function formatSpeed(bytesPerSecond: number): string {
  return strings.units.perSecond(formatBytes(Math.round(bytesPerSecond)))
}

/** A length as a video player shows it: "0:19", "2:23", "1:02:03". */
export function formatDuration(seconds: number): string {
  const total = Math.round(seconds)
  const hours = Math.floor(total / 3600)
  const minutes = Math.floor((total % 3600) / 60)
  const remainingSeconds = String(total % 60).padStart(2, "0")
  return hours > 0
    ? `${hours}:${String(minutes).padStart(2, "0")}:${remainingSeconds}`
    : `${minutes}:${remainingSeconds}`
}

/** A calendar date given as YYYY-MM-DD, in the reader's format ("Apr 24, 2005" in US English). */
export function formatCalendarDate(isoDate: string, locale?: string): string {
  // As UTC: a date without a time would otherwise shift a day in time zones west of UTC.
  return new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeZone: "UTC" }).format(
    new Date(`${isoDate}T00:00:00Z`),
  )
}

/** The day of a moment given in seconds since 1970, in the reader's time zone and format. */
export function formatDay(secondsSinceEpoch: number, locale?: string, timeZone?: string): string {
  return new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeZone }).format(new Date(secondsSinceEpoch * 1000))
}

/** Joins the parts of a line of details that are known, skipping the rest: "jawed · youtube.com". */
export function joinDetails(parts: readonly (string | null | undefined | false)[]): string {
  return parts.filter(Boolean).join(strings.separator)
}

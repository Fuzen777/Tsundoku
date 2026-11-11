import type { FileContents } from "./bindings/FileContents"
import type { PostContents } from "./bindings/PostContents"
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

/** A day a site only gives roughly ("3 weeks ago" in YouTube's listings), given as YYYY-MM-DD, as how long ago it was in
 * the reader's language, as roughly: "3 weeks ago", "last month". */
export function formatRoughAge(isoDate: string, locale?: string, now = Date.now()): string {
  const days = Math.max(0, Math.floor((now - Date.parse(`${isoDate}T00:00:00Z`)) / 86_400_000))
  const relative = new Intl.RelativeTimeFormat(locale, { numeric: "auto" })
  if (days < 7) return relative.format(-days, "day")
  if (days < 30) return relative.format(-Math.round(days / 7), "week")
  if (days < 365) return relative.format(-Math.round(days / 30), "month")
  return relative.format(-Math.round(days / 365), "year")
}

/** How long ago a moment given in seconds since 1970 was, in the reader's language: "5 minutes ago", "yesterday". */
export function formatTimeAgo(secondsSinceEpoch: number, locale?: string, now = Date.now()): string {
  const seconds = Math.max(0, Math.round(now / 1000 - secondsSinceEpoch))
  const relative = new Intl.RelativeTimeFormat(locale, { numeric: "auto" })
  if (seconds < 60) return relative.format(0, "second")
  if (seconds < 3600) return relative.format(-Math.floor(seconds / 60), "minute")
  if (seconds < 86_400) return relative.format(-Math.floor(seconds / 3600), "hour")
  return relative.format(-Math.floor(seconds / 86_400), "day")
}

/** A moment's day in the reader's time zone as a file name has it, so files sort by it: "2026-11-06". */
export function formatFileDate(moment: Date): string {
  return [moment.getFullYear(), moment.getMonth() + 1, moment.getDate()]
    .map((part) => String(part).padStart(2, "0"))
    .join("-")
}

/** Joins the parts of a line of details that are known, skipping the rest: "jawed · youtube.com". */
export function joinDetails(parts: readonly (string | null | undefined | false)[]): string {
  return parts.filter(Boolean).join(strings.separator)
}

/** What a post or a file host's link holds: "2 images, 1 video", "1 file" (a .zip on Pixeldrain), "3 files, 1.2 GB".
 * Null for a video. */
export function formatContents({ post, files }: { post: PostContents | null; files: FileContents | null }): string | null {
  if (post) return strings.preview.postContents(post.images, post.videos, post.others)
  // A host read through the check window gives its size only once the check is passed.
  if (files) return strings.preview.fileContents(files.count, files.bytes > 0 ? formatBytes(files.bytes) : null)
  return null
}

/** When a wait ends, given in milliseconds since 1970, in the reader's format: its time ("2:32 PM", "14:32"), with the
 * day's name before it when it isn't today ("Sat 9:05 AM"). */
export function formatClockTime(millisecondsSinceEpoch: number, locale?: string, timeZone?: string, now = Date.now()): string {
  const moment = new Date(millisecondsSinceEpoch)
  const day = new Intl.DateTimeFormat(locale, { dateStyle: "short", timeZone })
  const today = day.format(moment) === day.format(new Date(now))
  return new Intl.DateTimeFormat(locale, {
    weekday: today ? undefined : "short",
    hour: "numeric",
    minute: "2-digit",
    timeZone,
  }).format(moment)
}

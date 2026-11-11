import { useEffect, useState } from "react"
import type { Hold } from "./bindings/Hold"
import type { SiteWait } from "./bindings/SiteWait"
import { formatBytes, formatClockTime, formatDuration } from "./format"
import { strings } from "./strings"

/** Why a waiting download doesn't start yet, as a sentence, and when it continues: its site asked to slow down, the
 * hourly limit on Instagram's files is reached, its connection dropped, its site's download limit is used up, its
 * drive lacks room, or its site wants to check you're a person. */
export function describeHold(hold: Hold): string {
  switch (hold.kind) {
    case "slowDown":
      return strings.queue.holds.slowDown(hold.site, formatClockTime(hold.until))
    case "hourlyLimit":
      return strings.queue.holds.hourlyLimit(hold.site, hold.limit, formatClockTime(hold.until))
    case "retry":
      return strings.queue.holds.retry(hold.attempt, hold.attempts, formatClockTime(hold.until))
    case "limitReached":
      return strings.queue.holds.limitReached(hold.site, formatClockTime(hold.until))
    case "notEnoughRoom":
      return hold.others > 0
        ? strings.queue.holds.notEnoughRoomBeside(
            hold.drive,
            formatBytes(hold.free),
            formatBytes(hold.needed),
            formatBytes(hold.others),
          )
        : strings.queue.holds.notEnoughRoom(hold.drive, formatBytes(hold.free), formatBytes(hold.needed))
    case "browserCheck":
      return strings.queue.holds.browserCheck(hold.site, hold.files)
  }
}

/** When a hold ends, for its countdown: none for a drive without room, which is only looked at again then, nor for a
 * check, which waits for you. */
export function holdEnd(hold: Hold): number | null {
  switch (hold.kind) {
    case "notEnoughRoom":
    case "browserCheck":
      return null
    default:
      return hold.until
  }
}

/** Why a link in Pending isn't read yet: its site asked to slow down. */
export function describeSiteWait(wait: SiteWait): string {
  return strings.queue.holds.slowDown(wait.site, formatClockTime(wait.until))
}

/** The time left until a wait ends (`until`, in milliseconds since 1970), as "4:59 left", counting down every second.
 * Null without a wait. */
export function useTimeLeft(until: number | null): string | null {
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    if (until === null) return
    const timer = window.setInterval(() => setNow(Date.now()), 1000)
    return () => window.clearInterval(timer)
  }, [until])
  if (until === null) return null
  return strings.queue.timeLeft(formatDuration(Math.max(0, until - now) / 1000))
}

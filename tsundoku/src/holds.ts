import { useEffect, useState } from "react"
import type { Hold } from "./bindings/Hold"
import type { SiteWait } from "./bindings/SiteWait"
import { formatClockTime, formatDuration } from "./format"
import { strings } from "./strings"

/** Why a waiting download doesn't start yet, as a sentence, and when it continues: its site asked to slow down, the
 * hourly limit on Instagram's files is reached, its connection dropped, or its site's download limit is used up. */
export function describeHold(hold: Hold): string {
  const time = formatClockTime(hold.until)
  switch (hold.kind) {
    case "slowDown":
      return strings.queue.holds.slowDown(hold.site, time)
    case "hourlyLimit":
      return strings.queue.holds.hourlyLimit(hold.site, hold.limit, time)
    case "retry":
      return strings.queue.holds.retry(hold.attempt, hold.attempts, time)
    case "limitReached":
      return strings.queue.holds.limitReached(hold.site, time)
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

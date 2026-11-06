import type { PendingLink } from "./bindings/PendingLink"

/**
 * The pending links the Pending view shows and the sidebar counts: all but copied links and links from dropped files
 * still being read, which may turn out to be nothing to download and leave. Those are only counted, as being checked.
 */
export function shownPendingLinks(links: readonly PendingLink[]): PendingLink[] {
  return links.filter((link) => !isBeingChecked(link))
}

/** A link caught from the clipboard or read from a dropped file that isn't read yet. */
export function isBeingChecked(link: PendingLink): boolean {
  return (link.caught || link.fromFile) && (link.state === "waiting" || link.state === "reading")
}

/** How many links are being checked, by where they came from. */
export function checkingCounts(links: readonly PendingLink[]): { copied: number; fromFiles: number } {
  const checking = links.filter(isBeingChecked)
  return {
    copied: checking.filter((link) => link.caught).length,
    fromFiles: checking.filter((link) => link.fromFile).length,
  }
}

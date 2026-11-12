import type { PendingLink } from "./bindings/PendingLink"

/**
 * The pending links the Pending view shows and the sidebar counts: all but copied links, links from dropped files and
 * links a page's Click'n'Load button sent still being read, which may turn out to be nothing to download and leave.
 * Those are only counted, as being checked.
 */
export function shownPendingLinks(links: readonly PendingLink[]): PendingLink[] {
  return links.filter((link) => !isBeingChecked(link))
}

/** A link caught from the clipboard, read from a dropped file or sent by a page that isn't read yet. */
export function isBeingChecked(link: PendingLink): boolean {
  return (link.caught || link.fromFile || link.fromPage) && (link.state === "waiting" || link.state === "reading")
}

/** How many links are being checked, by where they came from. */
export function checkingCounts(links: readonly PendingLink[]): { copied: number; fromFiles: number; fromPages: number } {
  const checking = links.filter(isBeingChecked)
  return {
    copied: checking.filter((link) => link.caught).length,
    fromFiles: checking.filter((link) => link.fromFile).length,
    fromPages: checking.filter((link) => link.fromPage).length,
  }
}

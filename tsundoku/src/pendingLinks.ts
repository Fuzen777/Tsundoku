import type { PendingLink } from "./bindings/PendingLink"

/**
 * The pending links the Pending view shows and the sidebar counts: all but copied links still being read, which may
 * turn out to be nothing to download and leave. Those are only counted, as being checked.
 */
export function shownPendingLinks(links: readonly PendingLink[]): PendingLink[] {
  return links.filter((link) => !isBeingChecked(link))
}

/** A link caught from the clipboard that isn't read yet. */
export function isBeingChecked(link: PendingLink): boolean {
  return link.caught && (link.state === "waiting" || link.state === "reading")
}

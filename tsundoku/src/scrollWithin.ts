/**
 * Scrolling an element into view inside the view that holds it, and nothing else. `scrollIntoView` also scrolls every
 * ancestor that can scroll, the window's own page included, which moved the whole window up (the sidebar and add bar
 * with it) when a section near the end, such as About, was asked to come to the top.
 */

/** The nearest ancestor that scrolls vertically, such as a view's `Page`. */
function scrollingAncestor(element: HTMLElement): HTMLElement | null {
  for (let parent = element.parentElement; parent; parent = parent.parentElement) {
    const { overflowY } = getComputedStyle(parent)
    if (overflowY === "auto" || overflowY === "scroll") return parent
  }
  return null
}

/**
 * Scrolls the element's own scrolling ancestor so the element sits at its top (less the element's `scroll-margin-top`)
 * or its center. The browser stops at the end of what can scroll, so the last section only comes as far up as it can.
 */
export function scrollWithin(element: HTMLElement, block: "start" | "center", behavior: ScrollBehavior = "auto") {
  const container = scrollingAncestor(element)
  if (!container) return
  const offset = element.getBoundingClientRect().top - container.getBoundingClientRect().top
  const margin = block === "start" ? parseFloat(getComputedStyle(element).scrollMarginTop) || 0 : 0
  const centering = block === "center" ? (container.clientHeight - element.offsetHeight) / 2 : 0
  container.scrollTo({
    top: container.scrollTop + offset - margin - centering,
    behavior,
  })
}

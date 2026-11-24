import { createContext, useContext, useLayoutEffect, type RefObject } from "react"
import { prefersReducedMotion } from "./motion"

/*
  Items coming into a list and leaving it (DESIGN.md → Motion): in the Pending and Downloads views' lists (and Watched's),
  a new item fades in where it's drawn, and one that left collapses away from where it was, the items under it sliding
  up into its place. A list follows its items' keys (`useListKeys`), and each item's element follows its own
  (`useEnterAndLeave`); an item that moves to another group, or a list drawn again in another view, does neither.
*/

/** How long an item takes to fade in, or to collapse away. */
const DURATION_MS = 200
/** Fluent's decelerate curve (index.css's `ease-decelerate`), for coming in. */
const DECELERATE = "cubic-bezier(0.1, 0.9, 0.2, 1)"
/** Fluent's easy ease, for leaving: gentle at both ends. */
const EASY_EASE = "cubic-bezier(0.33, 0, 0.67, 1)"
/** More items than this leaving at once (Clear Finished on a long list) just go: so many collapsing together would
 * only make the window slow. */
const AT_MOST_LEAVING = 30

/** What a list knows of its items' keys. */
interface ListKeys {
  /** Every key it has drawn, since it was first drawn. */
  seen: Set<number>
  /** The keys it draws now. */
  listed: Set<number>
  /** Drawn once already: what's drawn with it the first time isn't new. */
  drawn: boolean
}

/** The keys of the list an item is in, for its element to tell whether it's new, or gone. */
export const ListKeysContext = createContext<ListKeys | null>(null)

/** Each view's list's keys, kept while the app runs, so a list drawn anew in its view (Pending's first link after it
 * was empty, or the view opened again) knows what it drew before. */
const keysByList = new Map<string, ListKeys>()

/** Follows the keys of a view's list (`list` names it), as it's drawn each time; for `ListKeysContext`. */
export function useListKeys(list: string, keys: readonly number[]): ListKeys {
  let known = keysByList.get(list)
  if (!known) {
    known = { seen: new Set(), listed: new Set(), drawn: false }
    keysByList.set(list, known)
  }
  // After its items' elements are drawn, so a new one finds its key unseen.
  useLayoutEffect(() => {
    const following = keysByList.get(list)
    if (!following) return
    following.listed = new Set(keys)
    for (const key of keys) following.seen.add(key)
    following.drawn = true
  })
  return known
}

/** An element leaving, with where it was. */
interface Leaving {
  element: HTMLElement
  parent: HTMLElement | null
  previous: Element | null
  next: Element | null
  key: number
  list: ListKeys
}

/** The elements of the last change that left, before they're put back to collapse away. */
let leaving: Leaving[] = []

/**
 * An item's element in a list that follows its keys (`ListKeysContext`): faded in when its key is new to the list, and,
 * when its key leaves the list, put back where it was once React has taken it out, to collapse away (or, in a grid of
 * pictures, to fade and shrink there), then removed. Nothing moves with reduced motion.
 */
export function useEnterAndLeave(element: RefObject<HTMLElement | null>, key: number): void {
  const known = useContext(ListKeysContext)
  useLayoutEffect(() => {
    const shown = element.current
    if (!shown || !known) return
    if (known.drawn && !known.seen.has(key) && !prefersReducedMotion()) {
      shown.animate([{ opacity: 0 }, { opacity: 1 }], { duration: DURATION_MS, easing: DECELERATE })
    }
    // Still in the page here: React takes it out after.
    return () => {
      if (prefersReducedMotion()) return
      leaving.push({
        element: shown,
        parent: shown.parentElement,
        previous: shown.previousElementSibling,
        next: shown.nextElementSibling,
        key,
        list: known,
      })
      if (leaving.length === 1) queueMicrotask(showLeaving)
    }
  }, [element, key, known])
}

/** Once React's change is done: the elements whose keys are no longer listed, in lists still in the page, go back to
 * collapse away. From the last, so each goes back before one that came after it. */
function showLeaving() {
  const left = leaving.filter(({ parent, key, list }) => parent?.isConnected && !list.listed.has(key))
  leaving = []
  if (left.length > AT_MOST_LEAVING) return
  for (const { element, parent, previous, next } of left.reverse()) {
    if (!parent) continue
    if (next?.parentElement === parent) parent.insertBefore(element, next)
    else if (previous?.parentElement === parent) previous.after(element)
    else parent.append(element)
    collapse(element, parent)
  }
}

/** An element put back to leave: no longer something to click or read, it collapses with the gap beside it, or fades
 * and shrinks in place in a grid, then goes. */
function collapse(element: HTMLElement, parent: HTMLElement) {
  element.inert = true
  element.setAttribute("aria-hidden", "true")
  const parentStyle = getComputedStyle(parent)
  const timing = { duration: DURATION_MS, easing: EASY_EASE, fill: "forwards" as const }
  let animation: Animation
  if (parentStyle.display.includes("grid")) {
    animation = element.animate([{ opacity: 1 }, { opacity: 0, transform: "scale(0.9)" }], timing)
  } else {
    // The gap after it, or before it when it's the last.
    const gap = parseFloat(parentStyle.rowGap) || 0
    const margin = element.nextElementSibling ? "marginBottom" : "marginTop"
    element.style.overflow = "hidden"
    animation = element.animate(
      [
        { opacity: 1, height: `${element.offsetHeight}px`, [margin]: "0px" },
        { opacity: 0, height: "0px", [margin]: `${element.previousElementSibling || element.nextElementSibling ? -gap : 0}px` },
      ],
      timing,
    )
  }
  const remove = () => element.remove()
  animation.finished.then(remove, remove)
}

import { useCallback, useEffect, useRef, useState } from "react"
import { cancelPreview, fetchPreview, listingKind } from "./api"
import type { LinkPreview } from "./bindings/LinkPreview"
import type { ListingKind } from "./bindings/ListingKind"
import { isCancelled, needsPassword } from "./errors"

export type PreviewState =
  | { stage: "idle" }
  /** `listing` is what the link is when its address says so (a channel, playlist or account), for the waiting text. */
  | { stage: "loading"; link: string; listing: ListingKind | null }
  /** `password` is the one typed that opened the link, which its download takes too. */
  | { stage: "ready"; preview: LinkPreview; password: string | null }
  /** For a link that needs a password: `unlocking` while a typed one is tried, and `tries` counts the wrong ones. */
  | { stage: "failed"; link: string; error: unknown; unlocking: boolean; tries: number }

/**
 * The preview card's state. Showing a link replaces whatever was shown, and only the latest link's answer is kept:
 * the core cancels an earlier fetch, and an answer that arrives late is ignored.
 */
export function usePreview() {
  const [state, setState] = useState<PreviewState>({ stage: "idle" })
  const latestRequest = useRef(0)

  const show = useCallback(async (link: string, password: string | null = null) => {
    latestRequest.current += 1
    const request = latestRequest.current
    setState({ stage: "loading", link, listing: null })
    listingKind(link)
      .then((listing) => {
        if (listing && request === latestRequest.current) {
          setState((current) => (current.stage === "loading" ? { ...current, listing } : current))
        }
      })
      .catch(() => {
        // Only the waiting text would have said more.
      })
    try {
      const preview = await fetchPreview(link, password)
      if (request === latestRequest.current) setState({ stage: "ready", preview, password })
    } catch (error) {
      if (request !== latestRequest.current) return
      setState(isCancelled(error) ? { stage: "idle" } : { stage: "failed", link, error, unlocking: false, tries: 0 })
    }
  }, [])

  /** Unlock, for a link that needs a password: reads it again with `password`, keeping the card's field while it's
   * tried, and again after a wrong one. */
  const unlock = useCallback(
    async (password: string) => {
      if (state.stage !== "failed" || !needsPassword(state.error)) return
      const { link, tries } = state
      latestRequest.current += 1
      const request = latestRequest.current
      setState({ ...state, unlocking: true })
      try {
        const preview = await fetchPreview(link, password)
        if (request === latestRequest.current) setState({ stage: "ready", preview, password })
      } catch (error) {
        if (request !== latestRequest.current) return
        setState(isCancelled(error) ? { stage: "idle" } : { stage: "failed", link, error, unlocking: false, tries: tries + 1 })
      }
    },
    [state],
  )

  /** Closes the card, cancelling a fetch in progress. */
  const close = useCallback(() => {
    latestRequest.current += 1
    void cancelPreview()
    setState({ stage: "idle" })
  }, [])

  return { state, show, unlock, close }
}

/** What was dropped on the window: its text (a link, several, or text holding them) and its files, either of which
 * may be missing. A browser often gives both, such as a linked image's file with the link. */
export interface Drop {
  text: string | null
  files: File[]
}

/**
 * Calls `onDrop` with what was dropped anywhere on the window: a link dragged from a browser's address bar, several
 * links, text holding them, or files from File Explorer. Needs `dragDropEnabled: false` in tauri.conf.json, or Tauri
 * takes the drop before the page sees it: WebView2 then gives the page text and files alike, files as `File`s.
 */
export function useLinkDrop(onDrop: (drop: Drop) => void) {
  useEffect(() => {
    function onDragOver(event: DragEvent) {
      const types = event.dataTransfer?.types ?? []
      if (types.includes("text/uri-list") || types.includes("text/plain") || types.includes("Files")) {
        event.preventDefault()
        if (event.dataTransfer) event.dataTransfer.dropEffect = "copy"
      }
    }
    function onDropped(event: DragEvent) {
      const text = droppedText(event.dataTransfer)
      const files = Array.from(event.dataTransfer?.files ?? [])
      if (!text && files.length === 0) return
      // Otherwise a dropped file would open in the window, as in a browser.
      event.preventDefault()
      onDrop({ text, files })
    }
    window.addEventListener("dragover", onDragOver)
    window.addEventListener("drop", onDropped)
    return () => {
      window.removeEventListener("dragover", onDragOver)
      window.removeEventListener("drop", onDropped)
    }
  }, [onDrop])
}

/** The links of a uri-list, one per line (lines starting with # are comments), or else the dropped text. */
function droppedText(data: DataTransfer | null): string | null {
  if (!data) return null
  const fromList = data
    .getData("text/uri-list")
    .split(/\r?\n/)
    .filter((line) => line.trim() && !line.startsWith("#"))
  const text = (fromList.length > 0 ? fromList.join("\n") : data.getData("text/plain")).trim()
  return text || null
}

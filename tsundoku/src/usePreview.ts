import { useCallback, useEffect, useRef, useState } from "react"
import { cancelPreview, fetchPreview } from "./api"
import type { VideoPreview } from "./bindings/VideoPreview"
import { isCancelled } from "./errors"

export type PreviewState =
  | { stage: "idle" }
  | { stage: "loading"; link: string }
  | { stage: "ready"; preview: VideoPreview }
  | { stage: "failed"; error: unknown }

/**
 * The preview card's state. Showing a link replaces whatever was shown, and only the latest link's answer is kept:
 * the core cancels an earlier fetch, and an answer that arrives late is ignored.
 */
export function usePreview() {
  const [state, setState] = useState<PreviewState>({ stage: "idle" })
  const latestRequest = useRef(0)

  const show = useCallback(async (link: string) => {
    latestRequest.current += 1
    const request = latestRequest.current
    setState({ stage: "loading", link })
    try {
      const preview = await fetchPreview(link)
      if (request === latestRequest.current) setState({ stage: "ready", preview })
    } catch (error) {
      if (request !== latestRequest.current) return
      setState(isCancelled(error) ? { stage: "idle" } : { stage: "failed", error })
    }
  }, [])

  /** Closes the card, cancelling a fetch in progress. */
  const close = useCallback(() => {
    latestRequest.current += 1
    void cancelPreview()
    setState({ stage: "idle" })
  }, [])

  return { state, show, close }
}

/**
 * Calls `onLink` with a link dropped anywhere on the window, such as one dragged from a browser's address bar.
 * Needs `dragDropEnabled: false` in tauri.conf.json, or Tauri takes the drop before the page sees it.
 */
export function useLinkDrop(onLink: (link: string) => void) {
  useEffect(() => {
    function onDragOver(event: DragEvent) {
      const types = event.dataTransfer?.types ?? []
      if (types.includes("text/uri-list") || types.includes("text/plain")) {
        event.preventDefault()
        if (event.dataTransfer) event.dataTransfer.dropEffect = "copy"
      }
    }
    function onDrop(event: DragEvent) {
      const link = droppedLink(event.dataTransfer)
      if (!link) return
      event.preventDefault()
      onLink(link)
    }
    window.addEventListener("dragover", onDragOver)
    window.addEventListener("drop", onDrop)
    return () => {
      window.removeEventListener("dragover", onDragOver)
      window.removeEventListener("drop", onDrop)
    }
  }, [onLink])
}

/** The first link of a uri-list (lines starting with # are comments), or else the dropped text. */
function droppedLink(data: DataTransfer | null): string | null {
  if (!data) return null
  const fromList = data
    .getData("text/uri-list")
    .split(/\r?\n/)
    .find((line) => line.trim() && !line.startsWith("#"))
  const link = (fromList ?? data.getData("text/plain")).trim()
  return link || null
}

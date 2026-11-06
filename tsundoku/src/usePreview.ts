import { useCallback, useEffect, useRef, useState } from "react"
import { cancelPreview, fetchPreview } from "./api"
import type { VideoPreview } from "./bindings/VideoPreview"
import { isCancelled } from "./errors"

export type PreviewState =
  | { stage: "idle" }
  | { stage: "loading"; link: string }
  | { stage: "ready"; preview: VideoPreview }
  | { stage: "failed"; link: string; error: unknown }

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
      setState(isCancelled(error) ? { stage: "idle" } : { stage: "failed", link, error })
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
 * Calls `onDrop` with what was dropped anywhere on the window: a link dragged from a browser's address bar, several
 * links, or text holding them. Needs `dragDropEnabled: false` in tauri.conf.json, or Tauri takes the drop before the
 * page sees it.
 */
export function useLinkDrop(onDrop: (text: string) => void) {
  useEffect(() => {
    function onDragOver(event: DragEvent) {
      const types = event.dataTransfer?.types ?? []
      if (types.includes("text/uri-list") || types.includes("text/plain")) {
        event.preventDefault()
        if (event.dataTransfer) event.dataTransfer.dropEffect = "copy"
      }
    }
    function onDropped(event: DragEvent) {
      const text = droppedText(event.dataTransfer)
      if (!text) return
      event.preventDefault()
      onDrop(text)
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

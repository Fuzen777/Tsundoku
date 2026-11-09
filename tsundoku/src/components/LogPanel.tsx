import { useLayoutEffect, useRef, useState } from "react"
import { Copy, ExternalLink } from "lucide-react"
import { copyText, openDownloadLog } from "../api"
import { describeDownloadError, type ErrorDescription } from "../errors"
import { useDownloadLog } from "../queries"
import { strings } from "../strings"
import { IconButton } from "./IconButton"
import { StatusMessage } from "./StatusMessage"

const text = strings.queue.log

/** How close to the bottom, in pixels, still counts as following the log as it grows. */
const FOLLOW_MARGIN = 24

/**
 * A download's log, under its row in the Queue: its end in monospace, read again every second while `live`, kept
 * scrolled to the bottom unless scrolled up. Copy and Open in Editor beside it.
 */
export function LogPanel({ downloadId, live }: { downloadId: number; live: boolean }) {
  const { data: log, error } = useDownloadLog(downloadId, live)
  const [actionError, setActionError] = useState<ErrorDescription | null>(null)
  const box = useRef<HTMLPreElement>(null)
  const following = useRef(true)

  useLayoutEffect(() => {
    const element = box.current
    if (element && following.current) element.scrollTop = element.scrollHeight
  }, [log?.text])

  const act = (action: () => Promise<void>) => {
    setActionError(null)
    action().catch((caught: unknown) => setActionError(describeDownloadError(caught)))
  }

  if (error) return <StatusMessage tone="danger" {...describeDownloadError(error)} />
  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-center gap-1">
        <span className="flex-1 text-xs text-fg-muted">
          {log ? (log.cut ? text.cut : log.text.trim() === "" ? text.empty : null) : text.loading}
        </span>
        <IconButton label={text.copy} icon={Copy} disabled={!log} onClick={() => act(() => copyText(log?.text ?? ""))} />
        <IconButton label={text.openInEditor} icon={ExternalLink} onClick={() => act(() => openDownloadLog(downloadId))} />
      </div>
      <pre
        ref={box}
        onScroll={(event) => {
          const element = event.currentTarget
          following.current = element.scrollHeight - element.scrollTop - element.clientHeight < FOLLOW_MARGIN
        }}
        className="max-h-80 overflow-auto rounded-md border border-line bg-input p-2 font-mono text-xs whitespace-pre-wrap break-all text-fg-secondary select-text"
      >
        {log?.text}
      </pre>
      {actionError && <StatusMessage tone="danger" {...actionError} />}
    </div>
  )
}

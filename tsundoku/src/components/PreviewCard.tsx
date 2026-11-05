import { useState } from "react"
import { Loader2, X } from "lucide-react"
import { openDownload, showDownloadInFolder } from "../api"
import type { PastDownload } from "../bindings/PastDownload"
import type { VideoPreview } from "../bindings/VideoPreview"
import { describeHistoryError, describePreviewError, type ErrorDescription } from "../errors"
import { formatCalendarDate, formatDay, formatDuration, joinDetails } from "../format"
import { strings } from "../strings"
import type { PreviewState } from "../usePreview"
import { Button } from "./Button"
import { Card } from "./Card"
import { StatusMessage } from "./StatusMessage"

interface PreviewCardProps {
  state: Exclude<PreviewState, { stage: "idle" }>
  /** Cancel, Dismiss or Close. */
  onClose: () => void
}

/** Under the add bar: what a link is, whether it was downloaded before, and Download. */
export function PreviewCard({ state, onClose }: PreviewCardProps) {
  return (
    <Card className="flex flex-col gap-3 p-4">
      {state.stage === "loading" && (
        <div className="flex items-center gap-3">
          <Loader2 size={16} className="shrink-0 animate-spin text-fg-secondary" />
          <div className="min-w-0 flex-1">
            <p>{strings.preview.fetching}</p>
            <p className="truncate text-xs text-fg-muted">{state.link}</p>
          </div>
          <Button size="sm" onClick={onClose}>
            {strings.preview.cancel}
          </Button>
        </div>
      )}
      {state.stage === "failed" && (
        <div className="flex items-start gap-3">
          <div className="min-w-0 flex-1">
            <StatusMessage tone="danger" {...describePreviewError(state.error)} />
          </div>
          <Button size="sm" variant="ghost" onClick={onClose}>
            {strings.preview.dismiss}
          </Button>
        </div>
      )}
      {state.stage === "ready" && <VideoDetails preview={state.preview} onClose={onClose} />}
    </Card>
  )
}

function VideoDetails({ preview, onClose }: { preview: VideoPreview; onClose: () => void }) {
  const { video, lastDownload, missingJsRuntime } = preview
  return (
    <>
      <div className="flex gap-4">
        {/* Keyed by its address, so a thumbnail that failed doesn't hide the next video's. */}
        <Thumbnail key={video.thumbnailUrl} url={video.thumbnailUrl} />
        <div className="flex min-w-0 flex-1 flex-col gap-1">
          <p className="line-clamp-2 font-semibold select-text" title={video.title}>
            {video.title}
          </p>
          <p className="truncate text-fg-secondary">{joinDetails([video.uploader, video.siteName])}</p>
          <p className="text-xs text-fg-secondary tabular-nums">
            {joinDetails([
              video.durationSeconds !== null && formatDuration(video.durationSeconds),
              video.uploadDate && formatCalendarDate(video.uploadDate),
              video.bestHeight === null ? strings.preview.audioOnly : strings.preview.quality(video.bestHeight),
            ])}
          </p>
        </div>
        <Button size="icon" variant="ghost" aria-label={strings.preview.close} title={strings.preview.close} onClick={onClose}>
          <X size={16} />
        </Button>
      </div>
      {lastDownload && <AlreadyDownloaded download={lastDownload} />}
      {missingJsRuntime && <StatusMessage tone="warning" message={strings.preview.missingDeno} />}
      <div className="flex justify-end">
        {/* Downloads arrive with the queue (M3). */}
        <Button variant="primary" disabled>
          {lastDownload ? strings.preview.downloadAgain : strings.preview.download}
        </Button>
      </div>
    </>
  )
}

/** The video's thumbnail, or an empty tile when it has none or it doesn't load. */
function Thumbnail({ url }: { url: string | null }) {
  const [failed, setFailed] = useState(false)
  return (
    <div className="aspect-video w-44 shrink-0 overflow-hidden rounded-md bg-input">
      {url && !failed && (
        <img
          src={url}
          alt=""
          referrerPolicy="no-referrer"
          className="size-full object-cover"
          onError={() => setFailed(true)}
        />
      )}
    </div>
  )
}

/** The duplicate warning: when and how the video was downloaded, and where the file is. */
function AlreadyDownloaded({ download }: { download: PastDownload }) {
  const [openError, setOpenError] = useState<ErrorDescription | null>(null)
  const run = (action: (downloadId: number) => Promise<void>) => {
    setOpenError(null)
    action(download.id).catch((error: unknown) => setOpenError(describeHistoryError(error)))
  }
  return (
    <>
      <StatusMessage
        tone="warning"
        message={strings.preview.alreadyDownloaded(formatDay(download.downloadedAt), download.preset)}
        detail={download.filePath}
      >
        {download.fileExists ? (
          <>
            <Button size="sm" onClick={() => run(openDownload)}>
              {strings.preview.openFile}
            </Button>
            <Button size="sm" variant="ghost" onClick={() => run(showDownloadInFolder)}>
              {strings.preview.showInFolder}
            </Button>
          </>
        ) : (
          <p className="text-xs text-fg-secondary">{strings.history.errors.fileMissing}</p>
        )}
      </StatusMessage>
      {openError && <StatusMessage tone="danger" {...openError} />}
    </>
  )
}

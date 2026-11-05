import { useState } from "react"
import { Loader2, X } from "lucide-react"
import { addToQueue, confirmWithUser, openDownload, showDownloadInFolder } from "../api"
import type { PastDownload } from "../bindings/PastDownload"
import type { VideoPreview } from "../bindings/VideoPreview"
import {
  describeDownloadError,
  describeHistoryError,
  describePreviewError,
  fixFor,
  type ErrorDescription,
} from "../errors"
import { formatCalendarDate, formatDay, formatDuration, joinDetails } from "../format"
import { usePresets } from "../queries"
import { strings } from "../strings"
import type { PreviewState } from "../usePreview"
import { Button } from "./Button"
import { Card } from "./Card"
import { FailureFixes } from "./FailureFixes"
import { SelectInput } from "./Field"
import { StatusMessage } from "./StatusMessage"
import { Thumbnail } from "./Thumbnail"

interface PreviewCardProps {
  state: Exclude<PreviewState, { stage: "idle" }>
  /** Cancel, Dismiss or Close. */
  onClose: () => void
  /** The video joined the queue. */
  onAdded: () => void
}

/** Under the add bar: what a link is, whether it was downloaded before, and Download. */
export function PreviewCard({ state, onClose, onAdded }: PreviewCardProps) {
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
            <StatusMessage tone="danger" {...describePreviewError(state.error)}>
              {fixFor(state.error) && <FailureFixes error={state.error} />}
            </StatusMessage>
          </div>
          <Button size="sm" variant="ghost" onClick={onClose}>
            {strings.preview.dismiss}
          </Button>
        </div>
      )}
      {state.stage === "ready" && <VideoDetails preview={state.preview} onClose={onClose} onAdded={onAdded} />}
    </Card>
  )
}

interface VideoDetailsProps {
  preview: VideoPreview
  onClose: () => void
  onAdded: () => void
}

function VideoDetails({ preview, onClose, onAdded }: VideoDetailsProps) {
  const { video, lastDownload, missingJsRuntime } = preview
  return (
    <>
      <div className="flex gap-4">
        <Thumbnail key={video.thumbnailUrl} url={video.thumbnailUrl} className="w-44" />
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
      {preview.inQueue ? (
        <p className="text-right text-fg-secondary">{strings.preview.inQueue}</p>
      ) : (
        <DownloadControls preview={preview} onAdded={onAdded} />
      )}
    </>
  )
}

/** The preset picker and Download. Downloading again with the preset used before asks first, since the new file
 * replaces the old one. */
function DownloadControls({ preview, onAdded }: { preview: VideoPreview; onAdded: () => void }) {
  const { data: presets } = usePresets()
  const [chosenId, setChosenId] = useState<string | null>(null)
  const [adding, setAdding] = useState(false)
  const [error, setError] = useState<ErrorDescription | null>(null)
  const presetId = chosenId ?? presets?.defaultId ?? null

  async function download() {
    const preset = presets?.presets.find((candidate) => candidate.id === presetId)
    if (!preset) return
    const sameAsBefore = preview.lastDownload?.preset === preset.name
    if (
      sameAsBefore &&
      !(await confirmWithUser({
        title: strings.preview.overwriteTitle,
        message: strings.preview.overwriteMessage(preset.name),
        confirmLabel: strings.preview.overwriteConfirm,
        cancelLabel: strings.dialogs.cancel,
      }))
    ) {
      return
    }
    setAdding(true)
    setError(null)
    try {
      await addToQueue(preview.video, preset.id, sameAsBefore)
      onAdded()
    } catch (caught) {
      setError(describeDownloadError(caught))
      setAdding(false)
    }
  }

  return (
    <div className="flex flex-col items-end gap-2">
      {error && <StatusMessage tone="danger" {...error} />}
      <div className="flex items-center gap-2">
        <SelectInput
          aria-label={strings.preview.preset}
          title={strings.preview.preset}
          value={presetId ?? ""}
          disabled={!presets || adding}
          onChange={(event) => setChosenId(event.target.value)}
        >
          {presets?.presets.map((preset) => (
            <option key={preset.id} value={preset.id}>
              {preset.name}
            </option>
          ))}
        </SelectInput>
        <Button variant="primary" loading={adding} disabled={!presetId} onClick={() => void download()}>
          {preview.lastDownload ? strings.preview.downloadAgain : strings.preview.download}
        </Button>
      </div>
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

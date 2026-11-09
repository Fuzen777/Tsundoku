import { useState } from "react"
import { FolderOpen, Loader2, Play, X } from "lucide-react"
import { addPostToQueue, addToQueue, confirmWithUser, openDownload, showDownloadInFolder } from "../api"
import type { PastDownload } from "../bindings/PastDownload"
import type { VideoPreview } from "../bindings/VideoPreview"
import {
  describeDownloadError,
  describeHistoryError,
  describePreviewError,
  fixFor,
  type ErrorDescription,
} from "../errors"
import { formatCalendarDate, formatContents, formatDay, formatDuration, joinDetails } from "../format"
import { defaultPick, withPicked } from "../postFiles"
import { useDownloadDestination, useLeftOutFiles, usePresets, useRulePreset } from "../queries"
import { strings } from "../strings"
import type { PreviewState } from "../usePreview"
import { Button } from "./Button"
import { Card } from "./Card"
import { DestinationLine } from "./DestinationLine"
import { FailureFixes, InstallTool } from "./FailureFixes"
import { SelectInput } from "./Field"
import { IconButton } from "./IconButton"
import { ListingPicker } from "./ListingPicker"
import { PostFilesStrip } from "./PostFilesStrip"
import { PresetRuleLink } from "./PresetRuleLink"
import { StatusMessage } from "./StatusMessage"
import { Thumbnail } from "./Thumbnail"

interface PreviewCardProps {
  state: Exclude<PreviewState, { stage: "idle" }>
  /** Cancel, Dismiss or Close. */
  onClose: () => void
  /** The video joined the queue. */
  onAdded: () => void
  /** Fetch the link again, after a fix such as setting up gallery-dl. */
  onRetry: (link: string) => void
}

/** Under the add bar: what a link is, whether it was downloaded before, and Download; or, for a channel, playlist or
 * account, its items to pick from (`ListingPicker`). */
export function PreviewCard({ state, onClose, onAdded, onRetry }: PreviewCardProps) {
  return (
    <Card className="flex flex-col gap-3 p-4">
      {state.stage === "loading" && (
        <div className="flex items-center gap-3">
          <Loader2 size={16} className="shrink-0 animate-spin text-fg-secondary" />
          <div className="min-w-0 flex-1">
            <p>{state.listing ? strings.preview.listing[state.listing] : strings.preview.fetching}</p>
            <p className="truncate text-xs text-fg-muted">{state.link}</p>
          </div>
          <IconButton label={strings.preview.cancel} icon={X} onClick={onClose} />
        </div>
      )}
      {state.stage === "failed" && (
        <div className="flex items-start gap-3">
          <div className="min-w-0 flex-1">
            <StatusMessage tone="danger" {...describePreviewError(state.error)}>
              {fixFor(state.error) && <FailureFixes error={state.error} onRetry={() => onRetry(state.link)} />}
            </StatusMessage>
          </div>
          <IconButton label={strings.preview.dismiss} icon={X} onClick={onClose} />
        </div>
      )}
      {state.stage === "ready" &&
        (state.preview.kind === "listing" ? (
          <ListingPicker listing={state.preview.listing} onClose={onClose} onAdded={onAdded} />
        ) : (
          <VideoDetails
            preview={state.preview}
            onClose={onClose}
            onAdded={onAdded}
            onRetry={onRetry}
          />
        ))}
    </Card>
  )
}

interface VideoDetailsProps {
  preview: VideoPreview
  onClose: () => void
  onAdded: () => void
  /** Read the link again, once Deno is set up. */
  onRetry: (link: string) => void
}

function VideoDetails({ preview, onClose, onAdded, onRetry }: VideoDetailsProps) {
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
              formatContents(video) ??
                (video.bestHeight === null ? strings.preview.audioOnly : strings.preview.quality(video.bestHeight)),
            ])}
          </p>
        </div>
        <IconButton label={strings.preview.close} icon={X} onClick={onClose} />
      </div>
      {lastDownload && <AlreadyDownloaded download={lastDownload} />}
      {missingJsRuntime && (
        <StatusMessage tone="warning" message={strings.preview.missingDeno}>
          <InstallTool tool="deno" label={strings.preview.setUpDeno} onDone={() => onRetry(video.url)} />
        </StatusMessage>
      )}
      {preview.inQueue ? (
        <p className="text-right text-fg-secondary">{strings.preview.inQueue}</p>
      ) : video.post || video.files ? (
        <PostDownloadControls preview={preview} onAdded={onAdded} />
      ) : (
        <DownloadControls preview={preview} onAdded={onAdded} />
      )}
    </>
  )
}

/** The preset picker and Download. The preset a rule picks comes first, with the rule beside it, else the default one.
 * Downloading again with the preset used before asks first, since the new file replaces the old one. */
function DownloadControls({ preview, onAdded }: { preview: VideoPreview; onAdded: () => void }) {
  const { data: presets } = usePresets()
  const { data: rulePreset, isPending: pickingPreset } = useRulePreset(preview.video)
  const [chosenId, setChosenId] = useState<string | null>(null)
  const [adding, setAdding] = useState(false)
  const [error, setError] = useState<ErrorDescription | null>(null)
  const presetId = chosenId ?? rulePreset?.presetId ?? presets?.defaultId ?? null
  const pickedByRule = rulePreset && rulePreset.presetId === presetId ? rulePreset.rule : null
  const ready = presetId !== null && !pickingPreset
  const { data: destination } = useDownloadDestination(preview.video, presetId, ready)
  // A folder picked for this download instead of the one the rules give.
  const [chosenFolder, setChosenFolder] = useState<string | null>(null)

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
      await addToQueue(preview.video, preset.id, sameAsBefore, chosenFolder)
      onAdded()
    } catch (caught) {
      setError(describeDownloadError(caught))
      setAdding(false)
    }
  }

  return (
    <div className="flex flex-col items-end gap-2">
      {error && <StatusMessage tone="danger" {...error} />}
      <div className="flex w-full items-center gap-2">
        <DestinationLine
          destination={destination}
          chosenFolder={chosenFolder}
          onChoose={setChosenFolder}
          disabled={adding}
          className="flex-1"
        />
        <PresetRuleLink rule={pickedByRule} />
        <SelectInput
          aria-label={strings.preview.preset}
          title={strings.preview.preset}
          value={presetId ?? ""}
          disabled={!presets || adding || pickingPreset}
          onChange={(event) => setChosenId(event.target.value)}
        >
          {presets?.presets.map((preset) => (
            <option key={preset.id} value={preset.id}>
              {preset.name}
            </option>
          ))}
        </SelectInput>
        <Button variant="primary" loading={adding} disabled={!ready} onClick={() => void download()}>
          {preview.lastDownload ? strings.preview.downloadAgain : strings.preview.download}
        </Button>
      </div>
    </div>
  )
}

/** Download for a post of images or a file host's link: every file as the site gives it, so there's no preset to
 * pick. Downloading again only fetches the files that aren't in the folder any more. */
function PostDownloadControls({ preview, onAdded }: { preview: VideoPreview; onAdded: () => void }) {
  const { video, lastDownload } = preview
  const files = video.postFiles ?? []
  // A post of one file has nothing to pick.
  const picksFiles = files.length > 1
  const leftOutQuery = useLeftOutFiles(video, picksFiles && lastDownload !== null)
  const leftOut = leftOutQuery.data ?? null
  const [chosen, setChosen] = useState<ReadonlySet<number> | null>(null)
  const picked = chosen ?? defaultPick(files, leftOut)
  const [adding, setAdding] = useState(false)
  const [error, setError] = useState<ErrorDescription | null>(null)
  const { data: destination } = useDownloadDestination(video, null)
  const [chosenFolder, setChosenFolder] = useState<string | null>(null)

  async function download() {
    setAdding(true)
    setError(null)
    try {
      await addPostToQueue(picksFiles ? withPicked(video, picked) : video, chosenFolder)
      onAdded()
    } catch (caught) {
      setError(describeDownloadError(caught))
      setAdding(false)
    }
  }

  return (
    <div className="flex flex-col gap-3">
      {picksFiles && (
        <>
          {leftOut && <StatusMessage tone="neutral" message={strings.postFiles.leftOut(leftOut.length)} />}
          <PostFilesStrip
            files={files}
            picked={picked}
            leftOut={leftOut}
            downloadedBefore={lastDownload !== null}
            disabled={adding}
            onChange={setChosen}
          />
        </>
      )}
      <div className="flex flex-col items-end gap-2">
        {error && <StatusMessage tone="danger" {...error} />}
        <div className="flex w-full items-center gap-2">
          <DestinationLine
            destination={destination}
            chosenFolder={chosenFolder}
            onChoose={setChosenFolder}
            disabled={adding}
            className="flex-1"
          />
          <Button
            variant="primary"
            loading={adding}
            // Until the history says which files were left out, the ticks may still change.
            disabled={picksFiles && (picked.size === 0 || (lastDownload !== null && leftOutQuery.isPending))}
            onClick={() => void download()}
          >
            {lastDownload ? strings.preview.downloadAgain : strings.preview.download}
          </Button>
        </div>
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
            <IconButton
              label={strings.preview.openFile}
              icon={Play}
              variant="secondary"
              onClick={() => run(openDownload)}
            />
            <IconButton
              label={strings.preview.showInFolder}
              icon={FolderOpen}
              onClick={() => run(showDownloadInFolder)}
            />
          </>
        ) : (
          <p className="text-xs text-fg-secondary">{strings.history.errors.fileMissing}</p>
        )}
      </StatusMessage>
      {openError && <StatusMessage tone="danger" {...openError} />}
    </>
  )
}

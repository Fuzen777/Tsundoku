import { useState } from "react"
import { Download, Inbox, Loader2, RefreshCw, X } from "lucide-react"
import {
  confirmWithUser,
  readPendingLinkAgain,
  removePendingLinks,
  setPendingPreset,
  startPendingLinks,
} from "../api"
import type { PendingLink } from "../bindings/PendingLink"
import type { PresetsOverview } from "../bindings/PresetsOverview"
import type { VideoInfo } from "../bindings/VideoInfo"
import { Button } from "../components/Button"
import { Card } from "../components/Card"
import { DestinationLine } from "../components/DestinationLine"
import { EmptyState } from "../components/EmptyState"
import { FailureFixes } from "../components/FailureFixes"
import { Checkbox, SelectInput } from "../components/Field"
import { IconButton } from "../components/IconButton"
import { Page } from "../components/Page"
import { StatusMessage } from "../components/StatusMessage"
import { Thumbnail } from "../components/Thumbnail"
import { describeDownloadError, describePreviewError, fixFor, type ErrorDescription } from "../errors"
import { formatCalendarDate, formatContents, formatDay, formatDuration, joinDetails } from "../format"
import { shownPendingLinks } from "../pendingLinks"
import { useDownloadDestination, usePendingLinks, usePresets, useSaveAddingOptions } from "../queries"
import { strings } from "../strings"

/**
 * Links waiting to be started: pasted or dropped several at once, or copied while Watch Clipboard is on, each read in
 * the background to show what it is. Copied links still being read are only counted, since they may turn out to be
 * nothing to download. Start All sends every link not downloaded before to the queue (those still being read go once
 * they're read); with links selected, Start Selected and Remove Selected act on those instead.
 */
export function PendingView() {
  const { data: allLinks = [] } = usePendingLinks()
  const links = shownPendingLinks(allLinks)
  const checking = allLinks.length - links.length
  const adding = useSaveAddingOptions()
  const watching = adding.options?.watchClipboard ?? false
  const { data: presets } = usePresets()
  const [chosenPresetId, setChosenPresetId] = useState<string | null>(null)
  const [selection, setSelection] = useState<ReadonlySet<number>>(new Set())
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<ErrorDescription | null>(null)

  const presetId = chosenPresetId ?? presets?.defaultId ?? null
  // Links that left (started, or removed elsewhere) drop out of the selection.
  const selected = links.filter((link) => selection.has(link.id))
  const startable = links.filter(startsWithAll)
  const allSelected = links.length > 0 && selected.length === links.length

  async function run(action: () => Promise<void>) {
    setBusy(true)
    setError(null)
    try {
      await action()
      setSelection(new Set())
    } catch (caught) {
      setError(describeDownloadError(caught))
    } finally {
      setBusy(false)
    }
  }

  async function start(toStart: PendingLink[]) {
    if (!presetId || toStart.length === 0) return
    const replacing = toStart.filter((link) => replacesFile(link, presetId, presets)).length
    if (
      replacing > 0 &&
      !(await confirmWithUser({
        title: strings.pending.overwriteTitle,
        message: strings.pending.overwriteMessage(replacing),
        confirmLabel: strings.pending.overwriteConfirm,
        cancelLabel: strings.dialogs.cancel,
      }))
    ) {
      return
    }
    await run(() =>
      startPendingLinks(
        toStart.map((link) => link.id),
        presetId,
      ),
    )
  }

  function toggle(id: number, on: boolean) {
    setSelection((current) => {
      const next = new Set(current)
      if (on) next.add(id)
      else next.delete(id)
      return next
    })
  }

  const watchSwitch = (
    <span title={strings.pending.watchClipboardHint} className="mr-2 text-fg-secondary">
      <Checkbox
        label={strings.pending.watchClipboard}
        checked={watching}
        disabled={!adding.options}
        onChange={(checked) => void adding.save({ watchClipboard: checked })}
      />
    </span>
  )

  const buttons =
    selected.length > 0 ? (
      <>
        <Button
          size="sm"
          variant="ghost"
          disabled={busy}
          onClick={() => void run(() => removePendingLinks(selected.map((link) => link.id)))}
        >
          {strings.pending.removeSelected}
        </Button>
        <Button size="sm" variant="primary" loading={busy} disabled={!presetId} onClick={() => void start(selected)}>
          {strings.pending.startSelected}
        </Button>
      </>
    ) : (
      links.length > 0 && (
        <Button
          size="sm"
          variant="primary"
          loading={busy}
          disabled={!presetId || startable.length === 0}
          onClick={() => void start(startable)}
        >
          {strings.pending.startAll}
        </Button>
      )
    )

  const emptyBody = !watching
    ? strings.pending.emptyBody
    : adding.options?.downloadRightAway
      ? strings.pending.emptyBodyRightAway
      : strings.pending.emptyBodyWatching

  return (
    <Page
      title={strings.nav.pending}
      actions={
        <>
          {watchSwitch}
          {buttons}
        </>
      }
    >
      {adding.error && <StatusMessage tone="danger" {...adding.error} />}
      {checking > 0 && (
        <p className="mb-3 flex items-center gap-1.5 text-fg-secondary">
          <Loader2 size={14} className="shrink-0 animate-spin" />
          {strings.pending.checkingCopied(checking)}
        </p>
      )}
      {links.length === 0 ? (
        <EmptyState icon={Inbox} title={strings.pending.emptyTitle} body={emptyBody} />
      ) : (
        <div className="flex flex-col gap-3">
          <div className="flex items-center justify-between gap-4">
            <label className="flex items-center gap-2 text-fg-secondary">
              <input
                type="checkbox"
                className="size-4 accent-accent"
                aria-label={strings.pending.selectAll}
                title={strings.pending.selectAll}
                checked={allSelected}
                onChange={(event) => setSelection(new Set(event.target.checked ? links.map((link) => link.id) : []))}
              />
              {strings.pending.count(links.length)}
            </label>
            <label className="flex items-center gap-2 text-fg-secondary">
              {strings.pending.videosStartAs}
              <SelectInput
                value={presetId ?? ""}
                disabled={!presets}
                onChange={(event) => setChosenPresetId(event.target.value)}
              >
                {presets?.presets.map((preset) => (
                  <option key={preset.id} value={preset.id}>
                    {preset.name}
                  </option>
                ))}
              </SelectInput>
            </label>
          </div>
          {error && <StatusMessage tone="danger" {...error} />}
          <div className="flex flex-col gap-2">
            {links.map((link) => (
              <PendingRow
                key={link.id}
                link={link}
                presets={presets}
                allPresetId={presetId}
                selected={selection.has(link.id)}
                onSelect={(on) => toggle(link.id, on)}
                onStart={() => void start([link])}
                busy={busy}
              />
            ))}
          </div>
        </div>
      )}
    </Page>
  )
}

interface PendingRowProps {
  link: PendingLink
  presets: PresetsOverview | undefined
  /** The preset picked for them all, which a link follows until it gets its own. */
  allPresetId: string | null
  selected: boolean
  onSelect: (selected: boolean) => void
  onStart: () => void
  busy: boolean
}

function PendingRow({ link, presets, allPresetId, selected, onSelect, onStart, busy }: PendingRowProps) {
  const [actionError, setActionError] = useState<ErrorDescription | null>(null)
  const preview = link.preview
  const video = preview?.video ?? null
  const presetId = link.presetId ?? allPresetId

  const act = (action: () => Promise<void>) => {
    setActionError(null)
    action().catch((error: unknown) => setActionError(describeDownloadError(error)))
  }

  return (
    <Card className="flex items-start gap-3 p-3">
      <input
        type="checkbox"
        className="mt-1 size-4 shrink-0 accent-accent"
        aria-label={strings.pending.select}
        title={strings.pending.select}
        checked={selected}
        onChange={(event) => onSelect(event.target.checked)}
      />
      <Thumbnail key={video?.thumbnailUrl ?? link.link} url={video?.thumbnailUrl ?? null} className="w-24" />
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <p className="truncate font-medium" title={video?.title ?? link.link}>
          {video?.title ?? link.link}
        </p>
        {video ? (
          <p className="truncate text-xs text-fg-secondary tabular-nums">
            {joinDetails([
              video.uploader,
              video.siteName,
              video.durationSeconds !== null && formatDuration(video.durationSeconds),
              video.uploadDate && formatCalendarDate(video.uploadDate),
              formatContents(video) ??
                (video.bestHeight === null ? strings.preview.audioOnly : strings.preview.quality(video.bestHeight)),
            ])}
          </p>
        ) : (
          <ReadingStatus link={link} />
        )}
        {video && !preview?.inQueue && <PendingDestination video={video} presetId={presetId} />}
        {preview?.lastDownload && (
          <StatusMessage
            tone="warning"
            message={strings.preview.alreadyDownloaded(
              formatDay(preview.lastDownload.downloadedAt),
              preview.lastDownload.preset,
            )}
          />
        )}
        {preview?.inQueue && <StatusMessage tone="neutral" message={strings.pending.inQueue} />}
        {link.state === "failed" && link.error && (
          <StatusMessage tone="danger" {...describePreviewError(link.error)}>
            {fixFor(link.error) && (
              <FailureFixes
                error={link.error}
                // The row has Try Again of its own; setting up gallery-dl reads the link again by itself.
                onRetry={
                  fixFor(link.error) === "signIn" ? undefined : () => act(() => readPendingLinkAgain(link.id))
                }
              />
            )}
          </StatusMessage>
        )}
        {actionError && <StatusMessage tone="danger" {...actionError} />}
      </div>
      <div className="flex shrink-0 flex-wrap items-center justify-end gap-1">
        {video && !video.post && !video.files && !preview?.inQueue && (
          <SelectInput
            aria-label={strings.pending.preset}
            title={strings.pending.preset}
            value={presetId ?? ""}
            disabled={!presets}
            onChange={(event) => {
              const chosen = event.target.value
              act(() => setPendingPreset(link.id, chosen === allPresetId ? null : chosen))
            }}
          >
            {presets?.presets.map((preset) => (
              <option key={preset.id} value={preset.id}>
                {preset.name}
              </option>
            ))}
          </SelectInput>
        )}
        {link.state === "failed" ? (
          <IconButton
            label={strings.pending.tryAgain}
            icon={RefreshCw}
            variant="secondary"
            onClick={() => act(() => readPendingLinkAgain(link.id))}
          />
        ) : (
          !preview?.inQueue &&
          !link.startWhenRead && (
            <IconButton
              label={preview?.lastDownload ? strings.pending.downloadAgain : strings.pending.start}
              icon={Download}
              variant="secondary"
              disabled={busy || !presetId}
              onClick={onStart}
            />
          )
        )}
        <IconButton label={strings.pending.remove} icon={X} onClick={() => act(() => removePendingLinks([link.id]))} />
      </div>
    </Card>
  )
}

/** Where a read link will be saved, and what picked the folder, as the preview card shows it: with the preset it would
 * start with, or none for a post or a file host's files. Nothing until it's known. */
function PendingDestination({ video, presetId }: { video: VideoInfo; presetId: string | null }) {
  const usesPreset = !video.post && !video.files
  const { data: destination } = useDownloadDestination(
    video,
    usesPreset ? presetId : null,
    !usesPreset || presetId !== null,
  )
  return destination ? <DestinationLine destination={destination} /> : null
}

/** For a link not read yet: whether it waits its turn or is being read, and whether it starts once read. */
function ReadingStatus({ link }: { link: PendingLink }) {
  if (link.state === "failed") return null
  return (
    <p className="flex items-center gap-1.5 text-xs text-fg-secondary">
      {link.state === "reading" && <Loader2 size={12} className="shrink-0 animate-spin" />}
      {joinDetails([
        link.state === "reading" ? strings.pending.reading : strings.pending.waiting,
        link.startWhenRead && strings.pending.startsWhenRead,
      ])}
    </p>
  )
}

/** Whether Start All takes the link: not one that couldn't be read, is in the queue, or was downloaded before. */
function startsWithAll(link: PendingLink): boolean {
  return link.state !== "failed" && !link.preview?.inQueue && !link.preview?.lastDownload && !link.startWhenRead
}

/** Whether starting the link replaces a file: a video downloaded before with the preset it would start with. A post
 * or a file host's link only fetches the files that are missing. */
function replacesFile(link: PendingLink, allPresetId: string, presets: PresetsOverview | undefined): boolean {
  const last = link.preview?.lastDownload
  const video = link.preview?.video
  if (!last || video?.post || video?.files) return false
  const preset = presets?.presets.find((candidate) => candidate.id === (link.presetId ?? allPresetId))
  return preset?.name === last.preset
}

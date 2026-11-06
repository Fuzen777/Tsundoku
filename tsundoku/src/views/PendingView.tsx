import { useState } from "react"
import { Download, Inbox, Loader2, RefreshCw, X } from "lucide-react"
import {
  confirmWithUser,
  readPendingLinkAgain,
  removePendingLinks,
  restoreRemovedPendingLinks,
  setPendingPreset,
  startPendingLinks,
} from "../api"
import { arrange, folderGroup, type GroupPlace, type SortFacts } from "../arrange"
import type { Destination } from "../bindings/Destination"
import type { ListGroup } from "../bindings/ListGroup"
import type { PendingLink } from "../bindings/PendingLink"
import type { PresetsOverview } from "../bindings/PresetsOverview"
import type { RulePreset } from "../bindings/RulePreset"
import type { VideoInfo } from "../bindings/VideoInfo"
import { ArrangeControls, GroupedList } from "../components/ArrangedList"
import { Button } from "../components/Button"
import { Card } from "../components/Card"
import { DestinationLine } from "../components/DestinationLine"
import { EmptyState } from "../components/EmptyState"
import { FailureFixes } from "../components/FailureFixes"
import { Checkbox, SelectInput } from "../components/Field"
import { IconButton } from "../components/IconButton"
import { Page } from "../components/Page"
import { PresetRuleLink } from "../components/PresetRuleLink"
import { StatusMessage } from "../components/StatusMessage"
import { Thumbnail } from "../components/Thumbnail"
import { describeDownloadError, describePreviewError, fixFor, type ErrorDescription } from "../errors"
import { formatCalendarDate, formatContents, formatDay, formatDuration, joinDetails } from "../format"
import { describeSiteWait, useTimeLeft } from "../holds"
import { shownPendingLinks } from "../pendingLinks"
import {
  useDownloadDestination,
  useDownloadDestinations,
  useListArrangement,
  usePendingLinks,
  usePresets,
  useRulePresets,
  useSaveAddingOptions,
  type DestinationRequest,
} from "../queries"
import { useDeleteKey } from "../shortcuts"
import { strings } from "../strings"
import { useOfferUndo } from "../undo"

/**
 * Links waiting to be started: pasted or dropped several at once, or copied while Watch Clipboard is on, each read in
 * the background to show what it is. Copied links still being read are only counted, since they may turn out to be
 * nothing to download. Start All sends every link not downloaded before to the queue (those still being read go once
 * they're read); with links selected, Start Selected and Remove Selected act on those instead. The list is grouped and
 * sorted as its toolbar says.
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
  const removeLinks = useRemoveLinks()
  const [error, setError] = useState<ErrorDescription | null>(null)

  const presetId = chosenPresetId ?? presets?.defaultId ?? null
  // Links that left (started, or removed elsewhere) drop out of the selection.
  const selected = links.filter((link) => selection.has(link.id))
  const startable = links.filter(startsWithAll)
  const allSelected = links.length > 0 && selected.length === links.length

  const { arrangement, ready, change: changeArrangement, error: arrangeError } = useListArrangement("pending")
  const { group } = arrangement
  // Where each read link will be saved, to group them by folder. The rows ask the same, so they share the answers.
  const readLinks = links.flatMap((link) => (link.preview ? [{ link, video: link.preview.video }] : []))
  // The preset a rule picks for each read video. A link follows it, else the one picked for them all, until it gets
  // its own.
  const rulePresets = useRulePresets(readLinks.map(({ video }) => video))
  const rulePresetOf = new Map(readLinks.map(({ link }, index) => [link.id, rulePresets[index] ?? null]))
  const followedPreset = (link: PendingLink) => rulePresetOf.get(link.id)?.presetId ?? presetId
  const destinations = useDownloadDestinations(
    readLinks.map(({ link, video }) =>
      destinationRequest(video, link.presetId ?? followedPreset(link), group === "destination"),
    ),
  )
  const destinationOf = new Map(readLinks.map(({ link }, index) => [link.id, destinations[index]]))
  const groups = arrange(
    links,
    arrangement,
    pendingFacts,
    group === "none" ? null : (link) => pendingGroup(link, group, destinationOf.get(link.id)),
  )

  async function run(action: () => Promise<void>) {
    setBusy(true)
    setError(null)
    try {
      await action()
    } catch (caught) {
      setError(describeDownloadError(caught))
    } finally {
      setBusy(false)
    }
  }

  async function start(toStart: PendingLink[]) {
    if (!presetId || toStart.length === 0) return
    const replacing = toStart.filter((link) => replacesFile(link, followedPreset(link), presets)).length
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
    await run(async () => {
      await startPendingLinks(
        toStart.map((link) => link.id),
        presetId,
      )
      // Links that start once they're read stay, unticked.
      setSelection(new Set())
    })
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

  // The removed links stay in the selection, out of sight, so Undo brings them back ticked.
  const removeSelected = () => void run(() => removeLinks(selected.map((link) => link.id)))
  // Delete outside a text field does what Remove Selected does.
  useDeleteKey(selected.length > 0 && !busy ? removeSelected : null)

  const buttons =
    selected.length > 0 ? (
      <>
        <Button
          size="sm"
          variant="ghost"
          disabled={busy}
          title={strings.shortcuts.withKeys(strings.pending.removeSelected, strings.shortcuts.delete)}
          onClick={removeSelected}
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
          <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
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
            <ArrangeControls
              arrangement={arrangement}
              disabled={!ready}
              onChange={(changes) => void changeArrangement(changes)}
            />
            <label className="ml-auto flex items-center gap-2 text-fg-secondary" title={strings.pending.videosStartAsHint}>
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
          {arrangeError && <StatusMessage tone="danger" {...arrangeError} />}
          {error && <StatusMessage tone="danger" {...error} />}
          <GroupedList
            groups={groups}
            grouped={group !== "none"}
            keyOf={(link) => link.id}
            renderItem={(link) => (
              <PendingRow
                link={link}
                presets={presets}
                allPresetId={followedPreset(link)}
                rulePreset={rulePresetOf.get(link.id) ?? null}
                selected={selection.has(link.id)}
                onSelect={(on) => toggle(link.id, on)}
                onStart={() => void start([link])}
                busy={busy}
              />
            )}
          />
        </div>
      )}
    </Page>
  )
}

interface PendingRowProps {
  link: PendingLink
  presets: PresetsOverview | undefined
  /** The preset a link follows until it gets its own: the one a rule picks, else the one picked for them all. */
  allPresetId: string | null
  /** The preset a rule picks for the link, and which rule. */
  rulePreset: RulePreset | null
  selected: boolean
  onSelect: (selected: boolean) => void
  onStart: () => void
  busy: boolean
}

function PendingRow({ link, presets, allPresetId, rulePreset, selected, onSelect, onStart, busy }: PendingRowProps) {
  const [actionError, setActionError] = useState<ErrorDescription | null>(null)
  const removeLinks = useRemoveLinks()
  const preview = link.preview
  const video = preview?.video ?? null
  const presetId = link.presetId ?? allPresetId
  const hold = link.state === "waiting" ? link.hold : null
  const holdTimeLeft = useTimeLeft(hold?.until ?? null)

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
        {hold && <StatusMessage tone="neutral" message={describeSiteWait(hold)} detail={holdTimeLeft ?? undefined} />}
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
          <>
            <PresetRuleLink rule={rulePreset && rulePreset.presetId === presetId ? rulePreset.rule : null} />
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
          </>
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
        <IconButton label={strings.pending.remove} icon={X} onClick={() => act(() => removeLinks([link.id]))} />
      </div>
    </Card>
  )
}

/** Where a read link will be saved, and what picked the folder, as the preview card shows it: with the preset it would
 * start with, or none for a post or a file host's files. Nothing until it's known. */
function PendingDestination({ video, presetId }: { video: VideoInfo; presetId: string | null }) {
  const request = destinationRequest(video, presetId)
  const { data: destination } = useDownloadDestination(request.video, request.presetId, request.enabled)
  return destination ? <DestinationLine destination={destination} /> : null
}

/** Removes links, then offers for a few seconds to put them back as they were. */
function useRemoveLinks(): (ids: number[]) => Promise<void> {
  const offerUndo = useOfferUndo()
  return async (ids) => {
    const removed = await removePendingLinks(ids)
    if (removed > 0) {
      offerUndo({
        message: strings.pending.removed(removed),
        undo: restoreRemovedPendingLinks,
        describeError: describeDownloadError,
      })
    }
  }
}

/** Asks where a read link will be saved: with the preset it would start with, or none for a post or a file host's
 * files, which don't use one. A video waits for a preset. */
function destinationRequest(video: VideoInfo, presetId: string | null, enabled = true): DestinationRequest {
  const usesPreset = !video.post && !video.files
  return { video, presetId: usesPreset ? presetId : null, enabled: enabled && (!usesPreset || presetId !== null) }
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

type PendingStatus = keyof typeof strings.pending.groups

/** Where links stand, in the order their groups come: those Start All takes first. */
const PENDING_STATUSES: readonly PendingStatus[] = ["ready", "downloadedBefore", "notRead", "failed", "inQueue"]

function pendingStatus(link: PendingLink): PendingStatus {
  if (link.state === "failed") return "failed"
  if (!link.preview) return "notRead"
  if (link.preview.inQueue) return "inQueue"
  return link.preview.lastDownload ? "downloadedBefore" : "ready"
}

/** Until it's read, a link is sorted by its address, with no uploader or site. */
function pendingFacts(link: PendingLink): SortFacts {
  const video = link.preview?.video
  return {
    id: link.id,
    addedAt: link.addedAt,
    title: video?.title ?? link.link,
    uploader: video?.uploader ?? null,
    site: video?.siteName ?? null,
  }
}

/** The group a link falls in: its folder, its site, or where it stands. Without a folder or site known yet (not read,
 * or couldn't be read), it goes by where it stands, after the folders or sites. */
function pendingGroup(
  link: PendingLink,
  group: Exclude<ListGroup, "none">,
  destination: Destination | undefined,
): GroupPlace {
  const video = link.preview?.video
  if (group === "destination" && destination) return folderGroup(destination.folder)
  if (group === "site" && video) return { key: `site:${video.site}`, label: video.siteName, rank: 0 }
  const status = pendingStatus(link)
  const rank = PENDING_STATUSES.indexOf(status)
  return { key: status, label: strings.pending.groups[status], rank: group === "status" ? rank : rank + 1 }
}

/** Whether Start All takes the link: not one that couldn't be read, is in the queue, or was downloaded before. */
function startsWithAll(link: PendingLink): boolean {
  return link.state !== "failed" && !link.preview?.inQueue && !link.preview?.lastDownload && !link.startWhenRead
}

/** Whether starting the link replaces a file: a video downloaded before with the preset it would start with. A post
 * or a file host's link only fetches the files that are missing. */
function replacesFile(link: PendingLink, followedPresetId: string | null, presets: PresetsOverview | undefined): boolean {
  const last = link.preview?.lastDownload
  const video = link.preview?.video
  if (!last || video?.post || video?.files) return false
  const preset = presets?.presets.find((candidate) => candidate.id === (link.presetId ?? followedPresetId))
  return preset?.name === last.preset
}

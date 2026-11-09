import { useState } from "react"
import { Download, Images, Inbox, Loader2, RefreshCw, X } from "lucide-react"
import {
  confirmWithUser,
  readPendingLinkAgain,
  removePendingLinks,
  restoreRemovedPendingLinks,
  setPendingPostFiles,
  setPendingPreset,
  startPendingLinks,
} from "../api"
import { AFTER_GROUPS, arrangeNested, firstItems, folderGroup, NO_GROUP, type GroupPlace, type SortFacts } from "../arrange"
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
import { PostFilesStrip } from "../components/PostFilesStrip"
import { Checkbox, SelectInput } from "../components/Field"
import { IconButton } from "../components/IconButton"
import { Page } from "../components/Page"
import { PresetRuleLink } from "../components/PresetRuleLink"
import { StatusMessage } from "../components/StatusMessage"
import { Thumbnail } from "../components/Thumbnail"
import { WaysToAdd, type WayToAdd } from "../components/WaysToAdd"
import { describeDownloadError, describePreviewError, fixFor, type ErrorDescription } from "../errors"
import { formatCalendarDate, formatContents, formatDay, formatDuration, joinDetails } from "../format"
import { pickedOf, pickSummary } from "../postFiles"
import { describeSiteWait, useTimeLeft } from "../holds"
import { checkingCounts, shownPendingLinks } from "../pendingLinks"
import {
  useDownloadDestination,
  useDownloadDestinations,
  useLeftOutFiles,
  useListArrangement,
  useRuleGroupOf,
  usePendingLinks,
  usePresets,
  useRulePresets,
  useSaveAddingOptions,
  type DestinationRequest,
} from "../queries"
import { useDeleteKey } from "../shortcuts"
import { strings } from "../strings"
import { useOfferUndo } from "../undo"

/** The ways to add links an empty Pending offers: the ones that work on their own first. */
const PENDING_WAYS: readonly WayToAdd[] = ["watch", "shortcut", "bookmark", "sendButtons", "drop", "paste"]

/** How many links Pending shows at first, and how many more Show More adds. */
const PENDING_PAGE = 200
/** The row's preset list's value for following Videos Start As. */
const DEFAULT_PRESET = ""

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
  const checking = checkingCounts(allLinks)
  // A long list, such as a bookmarks file's, shows a page at a time.
  const [shownCount, setShownCount] = useState(PENDING_PAGE)
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
  const { group, byStatus } = arrangement
  const needsDestinations = group === "destination" || group === "rule"
  const ruleGroupOf = useRuleGroupOf(group === "rule")
  // Where each read link will be saved, to group them by folder. The rows ask the same, so they share the answers.
  const readLinks = links.flatMap((link) => (link.preview ? [{ link, video: link.preview.video }] : []))
  // The preset a rule picks for each read video. A link follows it, else the one picked for them all, until it gets
  // its own.
  const rulePresets = useRulePresets(readLinks.map(({ video }) => video))
  const rulePresetOf = new Map(readLinks.map(({ link }, index) => [link.id, rulePresets[index] ?? null]))
  const followedPreset = (link: PendingLink) => rulePresetOf.get(link.id)?.presetId ?? presetId
  const destinations = useDownloadDestinations(
    readLinks.map(({ link, video }) =>
      destinationRequest(video, link.presetId ?? followedPreset(link), needsDestinations),
    ),
  )
  const destinationOf = new Map(readLinks.map(({ link }, index) => [link.id, destinations[index]]))
  const groups = arrangeNested(
    links,
    arrangement,
    pendingFacts,
    byStatus ? pendingStatusGroup : null,
    group === "none" ? null : (link) => pendingGroup(link, group, destinationOf.get(link.id), ruleGroupOf, byStatus),
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
      {[
        checking.copied > 0 && strings.pending.checkingCopied(checking.copied),
        checking.fromFiles > 0 && strings.pending.checkingFiles(checking.fromFiles),
      ].map(
        (line) =>
          line && (
            <p key={line} className="mb-3 flex items-center gap-1.5 text-fg-secondary">
              <Loader2 size={14} className="shrink-0 animate-spin" />
              {line}
            </p>
          ),
      )}
      {links.length === 0 ? (
        <EmptyState icon={Inbox} title={strings.pending.emptyTitle} body={strings.pending.emptyBody}>
          <WaysToAdd ways={PENDING_WAYS} />
        </EmptyState>
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
            groups={firstItems(groups, shownCount)}
            grouped={group !== "none" || byStatus}
            keyOf={(link) => link.id}
            renderItem={(link) => (
              <PendingRow
                link={link}
                presets={presets}
                allPresetId={followedPreset(link)}
                batchPresetId={presetId}
                rulePreset={rulePresetOf.get(link.id) ?? null}
                selected={selection.has(link.id)}
                onSelect={(on) => toggle(link.id, on)}
                onStart={() => void start([link])}
                busy={busy}
              />
            )}
          />
          {links.length > shownCount && (
            <div>
              <Button size="sm" onClick={() => setShownCount((count) => count + PENDING_PAGE)}>
                {strings.pending.showMore(Math.min(PENDING_PAGE, links.length - shownCount))}
              </Button>
            </div>
          )}
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
  /** Videos Start As: a row whose preset is that one shows Default. */
  batchPresetId: string | null
  selected: boolean
  onSelect: (selected: boolean) => void
  onStart: () => void
  busy: boolean
}

function PendingRow({
  link,
  presets,
  allPresetId,
  rulePreset,
  batchPresetId,
  selected,
  onSelect,
  onStart,
  busy,
}: PendingRowProps) {
  const [actionError, setActionError] = useState<ErrorDescription | null>(null)
  const removeLinks = useRemoveLinks()
  const preview = link.preview
  const video = preview?.video ?? null
  const presetId = link.presetId ?? allPresetId
  // Videos Start As's preset shows as Default, unless a rule picks the link's preset, which the rule's link says.
  const showsDefault = presetId !== null && presetId === batchPresetId && !rulePreset
  const batchPresetName = presets?.presets.find((preset) => preset.id === batchPresetId)?.name
  const [showsFiles, setShowsFiles] = useState(false)
  const files = video?.postFiles ?? []
  // A post of one file has nothing to pick.
  const picksFiles = files.length > 1 && !preview?.inQueue
  const nothingPicked = files.length > 0 && files.every((file) => !file.picked)
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
              pickSummary(video) ??
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
        {video && picksFiles && showsFiles && (
          <PendingPostFiles
            video={video}
            downloadedBefore={!!preview?.lastDownload}
            onPick={(picked) => act(() => setPendingPostFiles(link.id, [...picked]))}
          />
        )}
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
              title={
                showsDefault && batchPresetName ? strings.pending.defaultPresetHint(batchPresetName) : strings.pending.preset
              }
              value={showsDefault ? DEFAULT_PRESET : (presetId ?? "")}
              disabled={!presets}
              onChange={(event) => {
                const chosen = event.target.value
                act(() =>
                  setPendingPreset(link.id, chosen === DEFAULT_PRESET || chosen === allPresetId ? null : chosen),
                )
              }}
            >
              {batchPresetId && !rulePreset && <option value={DEFAULT_PRESET}>{strings.pending.defaultPreset}</option>}
              {presets?.presets.map((preset) => (
                <option key={preset.id} value={preset.id}>
                  {preset.name}
                </option>
              ))}
            </SelectInput>
          </>
        )}
        {picksFiles && (
          <IconButton
            label={showsFiles ? strings.postFiles.hideFiles : strings.postFiles.pickFiles}
            icon={Images}
            aria-expanded={showsFiles}
            onClick={() => setShowsFiles((shown) => !shown)}
          />
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
              disabled={busy || !presetId || nothingPicked}
              onClick={onStart}
            />
          )
        )}
        <IconButton label={strings.pending.remove} icon={X} onClick={() => act(() => removeLinks([link.id]))} />
      </div>
    </Card>
  )
}

/** A read post's files to pick from, as on the preview card. The ticks are the link's own, saved as they change; a
 * post downloaded before starts with the files left out then. */
function PendingPostFiles({
  video,
  downloadedBefore,
  onPick,
}: {
  video: VideoInfo
  downloadedBefore: boolean
  onPick: (picked: Set<number>) => void
}) {
  const { data: leftOut } = useLeftOutFiles(video, downloadedBefore)
  const files = video.postFiles ?? []
  return (
    <div className="flex flex-col gap-2 pt-1">
      {leftOut && <StatusMessage tone="neutral" message={strings.postFiles.leftOut(leftOut.length)} />}
      <PostFilesStrip
        files={files}
        picked={pickedOf(files)}
        leftOut={leftOut ?? null}
        downloadedBefore={downloadedBefore}
        onChange={onPick}
      />
    </div>
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

/** The group of where a link stands. */
function pendingStatusGroup(link: PendingLink): GroupPlace {
  const status = pendingStatus(link)
  return { key: status, label: strings.pending.groups[status], rank: PENDING_STATUSES.indexOf(status) }
}

/**
 * The group a link falls in: its folder, its site, or what picks its folder. Without one known yet (not read, or
 * couldn't be read), it goes under no heading of its own when grouped by status too, else by where it stands, after
 * the others.
 */
function pendingGroup(
  link: PendingLink,
  group: Exclude<ListGroup, "none">,
  destination: Destination | undefined,
  ruleGroupOf: ((destination: Destination) => GroupPlace) | null,
  byStatus: boolean,
): GroupPlace {
  const video = link.preview?.video
  if (group === "destination" && destination) return folderGroup(destination.folder)
  if (group === "rule" && destination && ruleGroupOf) return ruleGroupOf(destination)
  if (group === "site" && video) return { key: `site:${video.site}`, label: video.siteName, rank: 0 }
  if (byStatus) return NO_GROUP
  const status = pendingStatusGroup(link)
  return { ...status, rank: AFTER_GROUPS + status.rank }
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

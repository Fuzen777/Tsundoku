import { useMemo, useRef, useState, type ReactNode } from "react"
import {
  ChevronUp,
  CircleAlert,
  Clock,
  Download,
  Film,
  History,
  Images,
  Inbox,
  KeyRound,
  ListChecks,
  ListVideo,
  Loader2,
  Lock,
  PanelTopOpen,
  RefreshCw,
  X,
  type LucideIcon,
} from "lucide-react"
import {
  confirmWithUser,
  readPendingLinkAgain,
  unlockPendingLink,
  removePendingLinks,
  restoreRemovedPendingLinks,
  setPendingPostFiles,
  setPendingSharedFiles,
  setPendingFolder,
  setPendingPreset,
  setPendingRule,
  startPendingLinks,
} from "../api"
import {
  AFTER_GROUPS,
  arrange,
  arrangeNested,
  chaptersOf,
  firstIndexOf,
  firstItems,
  folderGroup,
  groupPath,
  LIST_GROUPS,
  LIST_SORTS,
  NO_GROUP,
  type GroupPlace,
  type ItemGroup,
  type SortFacts,
} from "../arrange"
import type { Destination } from "../bindings/Destination"
import type { ListGroup } from "../bindings/ListGroup"
import type { PendingLink } from "../bindings/PendingLink"
import type { PreviewError } from "../bindings/PreviewError"
import type { PresetsOverview } from "../bindings/PresetsOverview"
import type { RulePreset } from "../bindings/RulePreset"
import type { VideoInfo } from "../bindings/VideoInfo"
import { useChapters } from "../chapters"
import { ArrangeControls, GroupedList } from "../components/ArrangedList"
import { Button } from "../components/Button"
import { Card } from "../components/Card"
import { DestinationLine, PickedRuleWarning } from "../components/DestinationLine"
import { EmptyState } from "../components/EmptyState"
import { FailureFixes } from "../components/FailureFixes"
import { PostFilesPicker } from "../components/PostFilesPicker"
import { SharedFilesList } from "../components/SharedFilesList"
import { Checkbox, SelectInput } from "../components/Field"
import { IconButton } from "../components/IconButton"
import { ItemTile, ItemTiles, type TilePlace } from "../components/ItemTiles"
import { TileLabel } from "../components/MediaTiles"
import type { MenuItem } from "../components/Menu"
import { Page } from "../components/Page"
import { PickerViewControls } from "../components/PickerViewControls"
import { PasswordForm } from "../components/PasswordForm"
import { PresetRuleLink } from "../components/PresetRuleLink"
import { StatusMessage } from "../components/StatusMessage"
import { classes } from "../components/styles"
import { Thumbnail } from "../components/Thumbnail"
import { WaysToAdd, type WayToAdd } from "../components/WaysToAdd"
import { describeDownloadError, describePreviewError, fixFor, needsPassword, type ErrorDescription } from "../errors"
import { useFoldedGroups } from "../foldedGroups"
import { formatCalendarDate, formatContents, formatDay, formatDuration, joinDetails } from "../format"
import { listsPostFiles, pickedOf, pickSummary, postLeftOutMessage } from "../postFiles"
import { sameFiles, sharedLeftOutMessage, sharedPickedOf } from "../sharedFiles"
import { describeSiteWait, useTimeLeft } from "../holds"
import { useAppActions } from "../navigation"
import { checkingCounts, shownPendingLinks } from "../pendingLinks"
import {
  useDownloadDestination,
  useDownloadDestinations,
  useLeftOutFiles,
  useLeftOutSharedFiles,
  useListArrangement,
  usePickerLayout,
  useRuleGroupOf,
  usePendingLinks,
  usePresets,
  useRulePresets,
  useSaveAddingOptions,
  type DestinationRequest,
} from "../queries"
import { useDeleteKey } from "../shortcuts"
import { strings } from "../strings"
import { useShownRow, type ShownRow } from "../tileRows"
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
 * sorted as its toolbar says, under Needs Attention's channels, playlists and accounts, which nothing starts: each is
 * picked from in the preview card. The links show as rows (List) or pictures (Masonry, Gallery), chosen in the header
 * so the choice stays in sight; grouped, the sidebar lists the groups under it as chapters.
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
  // The preset a rule picks for each read video, the one picked for the link first. A link follows it, else the one
  // picked for them all, until it gets its own.
  const rulePresets = useRulePresets(
    readLinks.map(({ video }) => video),
    readLinks.map(({ link }) => link.ruleId),
  )
  const rulePresetOf = new Map(readLinks.map(({ link }, index) => [link.id, rulePresets[index] ?? null]))
  const followedPreset = (link: PendingLink) => rulePresetOf.get(link.id)?.presetId ?? presetId
  const destinations = useDownloadDestinations(
    readLinks.map(({ link, video }) =>
      destinationRequest(video, link.presetId ?? followedPreset(link), link.ruleId, needsDestinations),
    ),
  )
  const destinationOf = new Map(readLinks.map(({ link }, index) => [link.id, destinations[index]]))
  // Links that list several items sit in Needs Attention, above the rest however they're grouped.
  const toPick = links.filter((link) => link.state === "needsPicking")
  const others = links.filter((link) => link.state !== "needsPicking")
  const grouped = group !== "none" || byStatus
  const arranged = arrangeNested(
    others,
    arrangement,
    pendingFacts,
    byStatus ? pendingStatusGroup : null,
    group === "none" ? null : (link) => pendingGroup(link, group, destinationOf.get(link.id), ruleGroupOf, byStatus),
  )
  const attentionGroups: ItemGroup<PendingLink>[] =
    toPick.length > 0
      ? [
          {
            ...STATUS_PLACES.needsAttention,
            items: arrange(toPick, arrangement, pendingFacts, null).flatMap(({ items }) => items),
          },
        ]
      : []
  // Ungrouped, the rest gets a heading of its own under Needs Attention, so it doesn't seem to belong there.
  const groups =
    grouped || toPick.length === 0
      ? arranged
      : arranged.map((only) => ({ ...only, key: "otherLinks", label: strings.pending.otherLinks }))
  const headed = grouped || toPick.length > 0
  const { layout, change: changeLayout } = usePickerLayout("pending")
  const tiles = layout.view === "list" ? null : layout.view

  // Going to a group shows the links up to it, and unfolds the status holding it.
  const folds = useFoldedGroups("pending")
  const { scrollRef, markShown } = useChapters({
    view: "pending",
    chapters: [...chaptersOf(attentionGroups), ...(headed ? chaptersOf(groups) : [])],
    reveal: (id) => {
      const holder = groupPath("", id.split("/")[1])
      if (holder !== id && folds.collapsed.has(holder)) folds.toggle(holder)
      const first = firstIndexOf(groups, id)
      if (first !== null && first >= shownCount) setShownCount((Math.floor(first / PENDING_PAGE) + 1) * PENDING_PAGE)
    },
  })

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

  /** A group heading's box: ticks or unticks every link in the group. */
  function selectAllOf(groupLinks: readonly PendingLink[], on: boolean) {
    setSelection((current) => {
      const next = new Set(current)
      for (const link of groupLinks) {
        if (on) next.add(link.id)
        else next.delete(link.id)
      }
      return next
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


  const pendingRow = (link: PendingLink, shownFor?: { shown: ShownRow<PendingOpening>; hide: () => void }) => (
    <PendingRow
      key={shownFor?.shown.opened}
      link={link}
      presets={presets}
      allPresetId={followedPreset(link)}
      batchPresetId={presetId}
      rulePreset={rulePresetOf.get(link.id) ?? null}
      folderInHeading={group === "destination"}
      selected={selection.has(link.id)}
      onSelect={(on) => toggle(link.id, on)}
      onStart={() => void start([link])}
      busy={busy}
      opens={shownFor?.shown.opens}
      onHide={shownFor?.hide}
    />
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
      viewControls={links.length > 0 && <PickerViewControls layout={layout} onChange={changeLayout} />}
      scrollRef={scrollRef}
      onScroll={markShown}
    >
      {adding.error && <StatusMessage tone="danger" {...adding.error} />}
      {[
        checking.copied > 0 && strings.pending.checkingCopied(checking.copied),
        checking.fromFiles > 0 && strings.pending.checkingFiles(checking.fromFiles),
        checking.fromPages > 0 && strings.pending.checkingPages(checking.fromPages),
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
              groups={LIST_GROUPS}
              sorts={LIST_SORTS}
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
          {[attentionGroups, firstItems(groups, shownCount).filter(({ items }) => items.length > 0)].map(
            (shownGroups, index) =>
              shownGroups.length > 0 && (
                <GroupedList
                  key={index}
                  view="pending"
                  groups={shownGroups}
                  grouped={index === 0 || headed}
                  keyOf={(link) => link.id}
                  selection={{ isSelected: (link) => selection.has(link.id), onSelect: selectAllOf }}
                  renderItem={(link) => pendingRow(link)}
                  renderItems={
                    tiles
                      ? (run) => (
                          <PendingTiles
                            links={run}
                            view={tiles}
                            size={layout.size}
                            renderRow={pendingRow}
                            tileOf={(link) => ({
                              byStatus,
                              selected: selection.has(link.id),
                              anySelected: selected.length > 0,
                              onSelect: (on) => toggle(link.id, on),
                              onStart: startsFromTile(link) && presetId && !busy ? () => void start([link]) : null,
                            })}
                          />
                        )
                      : undefined
                  }
                  folds={folds}
                />
              ),
          )}
          {others.length > shownCount && (
            <div>
              <Button size="sm" onClick={() => setShownCount((count) => count + PENDING_PAGE)}>
                {strings.pending.showMore(Math.min(PENDING_PAGE, others.length - shownCount))}
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
  /** Grouped by folder: the heading shows the folder, so the row shows only what picked it. */
  folderInHeading: boolean
  selected: boolean
  onSelect: (selected: boolean) => void
  onStart: () => void
  busy: boolean
  /** Shown for a picture: what it opens with (its files, or its password field). */
  opens?: PendingOpening
  /** Shown for a picture: hides the row again, with a button of its own. */
  onHide?: () => void
}

/** What a link's row opens with, when it's shown for a picture in Masonry or Gallery: its files to pick, or the field
 * for its password. */
type PendingOpening = "files" | "password" | null

function PendingRow({
  link,
  presets,
  allPresetId,
  rulePreset,
  batchPresetId,
  folderInHeading,
  selected,
  onSelect,
  onStart,
  busy,
  opens = null,
  onHide,
}: PendingRowProps) {
  const [actionError, setActionError] = useState<ErrorDescription | null>(null)
  const removeLinks = useRemoveLinks()
  const { pickFromPending } = useAppActions()
  const preview = link.preview
  const video = preview?.video ?? null
  const presetId = link.presetId ?? allPresetId
  // Videos Start As's preset shows as Default, unless a rule picks the link's preset, which the rule's link says.
  const showsDefault = presetId !== null && presetId === batchPresetId && !rulePreset
  const batchPresetName = presets?.presets.find((preset) => preset.id === batchPresetId)?.name
  const [showsFiles, setShowsFiles] = useState(opens === "files")
  const sharedFiles = video?.sharedFiles ?? []
  const picksFiles = picksFilesOf(link)
  const nothingPicked = video !== null && nothingPickedIn(video)
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
        {video && !preview?.inQueue && (
          <PendingDestination
            folderInHeading={folderInHeading}
            video={video}
            presetId={presetId}
            chosenFolder={link.folder}
            onChoose={(folder) => act(() => setPendingFolder(link.id, folder))}
            chosenRuleId={link.ruleId}
            onChooseRule={(ruleId) => act(() => setPendingRule(link.id, ruleId))}
          />
        )}
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
        {video && picksFiles && showsFiles && (sharedFiles.length > 0 ? (
          <PendingSharedFiles
            video={video}
            downloadedBefore={!!preview?.lastDownload}
            save={(picked) => setPendingSharedFiles(link.id, [...picked])}
            onError={(error) => setActionError(describeDownloadError(error))}
          />
        ) : (
          <PendingPostFiles
            video={video}
            downloadedBefore={!!preview?.lastDownload}
            save={(picked) => setPendingPostFiles(link.id, [...picked])}
            onError={(error) => setActionError(describeDownloadError(error))}
          />
        ))}
        {link.state === "needsPassword" && (
          <PendingPassword
            link={link}
            autoFocus={opens === "password"}
            onUnlock={(password) => act(() => unlockPendingLink(link.id, password))}
          />
        )}
        {link.state === "needsPicking" && (
          <p className="flex items-center gap-1.5 text-xs text-fg-secondary">
            <ListChecks size={12} className="shrink-0" />
            {strings.pending.toPick[link.listing ?? "other"]}
          </p>
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
            icon={ListChecks}
            aria-expanded={showsFiles}
            onClick={() => setShowsFiles((shown) => !shown)}
          />
        )}
        {link.state === "needsPicking" ? (
          <IconButton
            label={strings.pending.pick}
            icon={ListChecks}
            variant="secondary"
            onClick={() => pickFromPending(link.id, link.link)}
          />
        ) : link.state === "failed" ? (
          <IconButton
            label={strings.pending.tryAgain}
            icon={RefreshCw}
            variant="secondary"
            onClick={() => act(() => readPendingLinkAgain(link.id))}
          />
        ) : (
          link.state !== "needsPassword" &&
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
        {onHide && <IconButton label={strings.tiles.hideDetails} icon={ChevronUp} onClick={onHide} />}
      </div>
    </Card>
  )
}

/** Whether a link's files can be picked: a post of several, or a file host's folder, not in Downloads already. */
function picksFilesOf(link: PendingLink): boolean {
  const video = link.preview?.video
  if (!video || link.preview?.inQueue) return false
  // A post of one file has nothing to pick; a file host's link lists its files only when it holds several.
  return (video.postFiles?.length ?? 0) > 1 || (video.sharedFiles?.length ?? 0) > 0
}

/** Whether every file of a post or a folder is unticked, which leaves nothing to start. */
function nothingPickedIn(video: VideoInfo): boolean {
  const files = video.postFiles ?? []
  const sharedFiles = video.sharedFiles ?? []
  return (
    (files.length > 0 && files.every((file) => !file.picked)) ||
    (sharedFiles.length > 0 && sharedFiles.every((file) => !file.picked))
  )
}

/** Whether a link's Start (or Download Again) shows, as its row's button does: read or not, but not one that needs
 * picking, a password, or reading again, is in Downloads already, waits to start once read, or has nothing ticked. */
function startsFromTile(link: PendingLink): boolean {
  const video = link.preview?.video
  return (
    link.state !== "needsPicking" &&
    link.state !== "failed" &&
    link.state !== "needsPassword" &&
    !link.preview?.inQueue &&
    !link.startWhenRead &&
    !(video && nothingPickedIn(video))
  )
}

/** What a link's picture needs from the view. */
interface PendingTileFacts {
  /** Grouped by where links stand: the heading says it, so the picture doesn't, unless it says more (being read or
   * waiting its turn, under Not Read Yet). */
  byStatus: boolean
  selected: boolean
  /** Whether anything is ticked, which shows every picture's tick box. */
  anySelected: boolean
  onSelect: (selected: boolean) => void
  /** Starts it, or null when it can't start from here now. */
  onStart: (() => void) | null
}

interface PendingTilesProps {
  links: readonly PendingLink[]
  view: "masonry" | "gallery"
  /** The step of the size buttons (`TILE_WIDTHS`). */
  size: number
  renderRow: (link: PendingLink, shownFor: { shown: ShownRow<PendingOpening>; hide: () => void }) => ReactNode
  tileOf: (link: PendingLink) => PendingTileFacts
}

/** Links as pictures (`ItemTiles`), each showing its row under its line when clicked. Gallery cuts them to squares
 * when they're all posts, else to 16:9. */
function PendingTiles({ links, view, size, renderRow, tileOf }: PendingTilesProps) {
  const [error, setError] = useState<ErrorDescription | null>(null)
  const box = useRef<HTMLDivElement>(null)
  const rows = useShownRow<PendingOpening>(box)
  const galleryRatio = links.every((link) => link.preview?.video.post) ? 1 : 16 / 9
  return (
    <div ref={box} className="flex flex-col gap-2">
      {error && <StatusMessage tone="danger" {...error} />}
      <ItemTiles
        items={links}
        view={view}
        size={size}
        keyOf={(link) => link.id}
        galleryRatio={galleryRatio}
        rows={rows}
        renderRow={(link, shown) => renderRow(link, { shown, hide: rows.hide })}
        renderTile={(link, place) => (
          <PendingTile
            link={link}
            place={place}
            facts={tileOf(link)}
            onShow={(opens) => rows.show(link.id, opens)}
            onHide={rows.hide}
            onError={setError}
          />
        )}
      />
    </div>
  )
}

/** The icon in a link's picture for where it stands, and its color; none when it's ready to start. */
const STATUS_ICONS: Partial<Record<PendingStatus | "reading" | "waiting", { icon: LucideIcon; className: string }>> = {
  reading: { icon: Loader2, className: "animate-spin text-fg-secondary" },
  waiting: { icon: Clock, className: "text-fg-secondary" },
  needsAttention: { icon: ListChecks, className: "text-accent" },
  needsPassword: { icon: KeyRound, className: "text-warning" },
  needsAccount: { icon: Lock, className: "text-warning" },
  failed: { icon: CircleAlert, className: "text-danger" },
  inQueue: { icon: ListVideo, className: "text-fg-secondary" },
  downloadedBefore: { icon: History, className: "text-warning" },
}

/** A link as its picture: where it stands in its corner, its row's actions in its right-click menu. */
function PendingTile({
  link,
  place,
  facts,
  onShow,
  onHide,
  onError,
}: {
  link: PendingLink
  place: TilePlace
  facts: PendingTileFacts
  onShow: (opens: PendingOpening) => void
  onHide: () => void
  onError: (error: ErrorDescription | null) => void
}) {
  const removeLinks = useRemoveLinks()
  const { pickFromPending } = useAppActions()
  const video = link.preview?.video ?? null
  const status = pendingStatus(link)
  const shownStatus = status === "notRead" ? (link.state === "reading" ? "reading" : "waiting") : status
  // Needs Attention is always a heading of its own.
  const headingSaysIt = (facts.byStatus || status === "needsAttention") && shownStatus === status
  const statusIcon = headingSaysIt ? undefined : STATUS_ICONS[shownStatus]
  const title = video?.title ?? link.link
  const details = video
    ? joinDetails([
        video.uploader,
        video.siteName,
        video.durationSeconds !== null && formatDuration(video.durationSeconds),
        pickSummary(video) ?? formatContents(video),
      ])
    : null
  const statusText = shownStatus === "reading" ? strings.pending.reading : strings.pending.groups[status]
  const hint = [title, details, statusText, strings.tiles.hint].filter(Boolean).join("\n")
  const act = (action: () => Promise<void>) => {
    onError(null)
    action().catch((error: unknown) => onError(describeDownloadError(error)))
  }
  const menuItems = (): MenuItem[] => [
    ...(link.state === "needsPicking"
      ? [{ label: strings.pending.pick, icon: ListChecks, onSelect: () => pickFromPending(link.id, link.link) }]
      : []),
    ...(link.state === "failed"
      ? [{ label: strings.pending.tryAgain, icon: RefreshCw, onSelect: () => act(() => readPendingLinkAgain(link.id)) }]
      : []),
    ...(link.state === "needsPassword"
      ? [{ label: strings.queue.enterPassword, icon: KeyRound, onSelect: () => onShow("password") }]
      : []),
    ...(facts.onStart
      ? [
          {
            label: link.preview?.lastDownload ? strings.pending.downloadAgain : strings.pending.start,
            icon: Download,
            onSelect: facts.onStart,
          },
        ]
      : []),
    ...(picksFilesOf(link) ? [{ label: strings.postFiles.pickFiles, icon: ListChecks, onSelect: () => onShow("files") }] : []),
    place.shown
      ? { label: strings.tiles.hideDetails, icon: ChevronUp, onSelect: onHide }
      : { label: strings.tiles.showDetails, icon: PanelTopOpen, onSelect: () => onShow(null) },
    { label: strings.pending.remove, icon: X, danger: true, onSelect: () => act(() => removeLinks([link.id])) },
  ]
  const isPost = video?.post != null
  return (
    <ItemTile
      id={link.id}
      place={place}
      title={title}
      thumbnailUrl={video?.thumbnailUrl ?? null}
      fallbackIcon={isPost ? Images : Film}
      shape={isPost ? 4 / 5 : 16 / 9}
      hint={hint}
      ariaLabel={joinDetails([title, statusText])}
      dimmed={!video}
      failed={link.state === "failed"}
      corner={
        statusIcon && (
          <TileLabel>
            <statusIcon.icon size={12} className={statusIcon.className} />
          </TileLabel>
        )
      }
      selection={{
        selected: facts.selected,
        showing: facts.anySelected,
        label: strings.pending.select,
        onSelect: facts.onSelect,
      }}
      menuLabel={strings.tiles.menu(title)}
      menuItems={menuItems}
      onToggle={place.shown ? onHide : () => onShow(null)}
    />
  )
}

/** A read post's files to pick from, as on the preview card: a strip, or a list for a post of files. The ticks are the
 * link's own, saved as they change; a post downloaded before starts with the files left out then. */
function PendingPostFiles({
  video,
  downloadedBefore,
  save,
  onError,
}: {
  video: VideoInfo
  downloadedBefore: boolean
  save: (picked: Set<number>) => Promise<void>
  onError: (error: unknown) => void
}) {
  const { data: leftOut } = useLeftOutFiles(video, downloadedBefore)
  const files = video.postFiles
  const saved = useMemo(() => pickedOf(files ?? []), [files])
  const [picked, change] = useHeldTicks(saved, save, onError)
  return (
    <div className="flex flex-col gap-2 pt-1">
      {leftOut && <StatusMessage tone="neutral" message={postLeftOutMessage(leftOut, picked)} />}
      <PostFilesPicker
        asList={listsPostFiles(video)}
        files={files ?? []}
        picked={picked}
        leftOut={leftOut ?? null}
        downloadedBefore={downloadedBefore}
        onChange={change}
      />
    </div>
  )
}

/** A read shared folder's files to pick from, as on the preview card. The ticks are the link's own, saved as they
 * change; a folder downloaded before starts with the files it didn't take then. */
function PendingSharedFiles({
  video,
  downloadedBefore,
  save,
  onError,
}: {
  video: VideoInfo
  downloadedBefore: boolean
  save: (picked: Set<number>) => Promise<void>
  onError: (error: unknown) => void
}) {
  const { data: leftOut } = useLeftOutSharedFiles(video, downloadedBefore)
  const files = video.sharedFiles
  const saved = useMemo(() => sharedPickedOf(files ?? []), [files])
  const [picked, change] = useHeldTicks(saved, save, onError)
  return (
    <div className="flex flex-col gap-2 pt-1">
      {leftOut && <StatusMessage tone="neutral" message={sharedLeftOutMessage(leftOut, picked)} />}
      <SharedFilesList
        files={files ?? []}
        picked={picked}
        leftOut={leftOut ?? null}
        downloadedBefore={downloadedBefore}
        onChange={change}
      />
    </div>
  )
}

/** A pending link's ticks as they show: the ones made here show at once and hold until the link comes back with them
 * all (`saved`), so a tick made meanwhile builds on them, not on what the link said before, and quick ticks aren't
 * lost. A tick that couldn't be saved goes back. */
function useHeldTicks(
  saved: ReadonlySet<number>,
  save: (picked: Set<number>) => Promise<void>,
  onError: (error: unknown) => void,
): [ReadonlySet<number>, (picked: Set<number>) => void] {
  const [ticked, setTicked] = useState<ReadonlySet<number> | null>(null)
  if (ticked && sameFiles(ticked, saved)) setTicked(null)
  return [
    ticked ?? saved,
    (next) => {
      setTicked(next)
      save(next).catch((error: unknown) => {
        setTicked(null)
        onError(error)
      })
    },
  ]
}

/** Where a read link will be saved, and what picked the folder, as the preview card shows it: with the preset it would
 * start with, or none for a post or a file host's files, and the rule picked for it, with a warning under it when that's
 * a manual rule that doesn't match the link. Nothing until it's known. */
function PendingDestination({
  video,
  presetId,
  chosenFolder,
  chosenRuleId,
  folderInHeading,
  onChoose,
  onChooseRule,
}: {
  video: VideoInfo
  presetId: string | null
  chosenFolder: string | null
  chosenRuleId: string | null
  folderInHeading: boolean
  onChoose: (folder: string | null) => void
  onChooseRule: (ruleId: string | null) => void
}) {
  const request = destinationRequest(video, presetId, chosenRuleId)
  const { data: destination } = useDownloadDestination(
    request.video,
    request.presetId,
    request.enabled,
    request.ruleId,
  )
  return destination || chosenFolder ? (
    <>
      <DestinationLine
        destination={destination}
        chosenFolder={chosenFolder}
        folderShown={!folderInHeading}
        onChoose={onChoose}
        chosenRuleId={chosenRuleId}
        onChooseRule={onChooseRule}
      />
      <PickedRuleWarning destination={destination} />
    </>
  ) : null
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
 * files, which don't use one, and the rule picked for it. A video waits for a preset. */
function destinationRequest(
  video: VideoInfo,
  presetId: string | null,
  ruleId: string | null,
  enabled = true,
): DestinationRequest {
  const usesPreset = !video.post && !video.files
  return {
    video,
    presetId: usesPreset ? presetId : null,
    ruleId,
    enabled: enabled && (!usesPreset || presetId !== null),
  }
}

/** For a link that needs a password: why it waits, whether a password tried was wrong, and the field to unlock it,
 * which reads it again with the password. */
function PendingPassword({
  link,
  autoFocus,
  onUnlock,
}: {
  link: PendingLink
  autoFocus: boolean
  onUnlock: (password: string) => void
}) {
  const wrong = needsPassword(link.error) && link.error.wrong
  return (
    <div className="flex flex-col gap-1.5">
      <p className={classes("flex items-center gap-1.5 text-xs", wrong ? "text-danger" : "text-fg-secondary")}>
        <KeyRound size={12} className="shrink-0" />
        {joinDetails([
          link.error && describePreviewError(link.error).message,
          link.startWhenRead && strings.pending.startsWhenUnlocked,
        ])}
      </p>
      <PasswordForm size="sm" autoFocus={autoFocus} onUnlock={onUnlock} />
    </div>
  )
}

/** For a link not read yet: whether it waits its turn or is being read, and whether it starts once read. */
function ReadingStatus({ link }: { link: PendingLink }) {
  if (link.state === "failed" || link.state === "needsPassword" || link.state === "needsPicking") return null
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

/** Where links stand, in the order their groups come: Needs Attention, which shows above the rest whatever the
 * grouping, then those Start All takes. */
const PENDING_STATUSES: readonly PendingStatus[] = [
  "needsAttention",
  "ready",
  "downloadedBefore",
  "notRead",
  "needsPassword",
  "needsAccount",
  "failed",
  "inQueue",
]

function pendingStatus(link: PendingLink): PendingStatus {
  if (link.state === "needsPicking") return "needsAttention"
  if (link.state === "failed") return needsAccount(link.error) ? "needsAccount" : "failed"
  if (link.state === "needsPassword") return "needsPassword"
  if (!link.preview) return "notRead"
  if (link.preview.inQueue) return "inQueue"
  return link.preview.lastDownload ? "downloadedBefore" : "ready"
}

/** Whether a link couldn't be read because the site wants an account: a sign-in (Tsundoku's own for Pixiv and Fanbox,
 * a browser's elsewhere) or a subscription it doesn't have (a Fanbox plan, 1fichier's subscribers). */
function needsAccount(error: PreviewError | null): boolean {
  return (
    error !== null &&
    (error.kind === "signInRequired" ||
      error.kind === "pixivSignInNeeded" ||
      error.kind === "fanboxSignInNeeded" ||
      error.kind === "fanboxLocked" ||
      error.kind === "subscribersOnly")
  )
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

/** Each status's group. */
const STATUS_PLACES = Object.fromEntries(
  PENDING_STATUSES.map((status, rank) => [status, { key: status, label: strings.pending.groups[status], rank }]),
) as Record<PendingStatus, GroupPlace>

/** The group of where a link stands. */
function pendingStatusGroup(link: PendingLink): GroupPlace {
  return STATUS_PLACES[pendingStatus(link)]
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
  const shown: Destination | undefined = link.folder ? { folder: link.folder, source: { kind: "chosen" } } : destination
  if (group === "destination" && shown) return folderGroup(shown.folder)
  if (group === "rule" && shown && ruleGroupOf) return ruleGroupOf(shown)
  if (group === "site" && video) return { key: `site:${video.site}`, label: video.siteName, rank: 0 }
  if (byStatus) return NO_GROUP
  const status = pendingStatusGroup(link)
  return { ...status, rank: AFTER_GROUPS + status.rank }
}

/** Whether Start All takes the link: not one that couldn't be read, needs a password or picking, is in the queue, or
 * was downloaded before. */
function startsWithAll(link: PendingLink): boolean {
  return (
    link.state !== "failed" &&
    link.state !== "needsPassword" &&
    link.state !== "needsPicking" &&
    !link.preview?.inQueue &&
    !link.preview?.lastDownload &&
    !link.startWhenRead
  )
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

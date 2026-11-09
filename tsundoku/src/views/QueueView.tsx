import { useState } from "react"
import { CircleStop, FileText, FolderOpen, ListVideo, Play, RefreshCw, X } from "lucide-react"
import {
  cancelDownload,
  clearFinishedDownloads,
  confirmWithUser,
  downloadNext,
  openDownload,
  openDownloadLog,
  pickFolder,
  restoreClearedDownloads,
  resumeAllDownloads,
  resumeDownload,
  retryDownload,
  retryFailedDownloads,
  setQueueFolder,
  showDownloadInFolder,
  stopAllDownloads,
  stopDownload,
  tryDownloadNow,
} from "../api"
import { arrangeNested, chosenFolderGroup, folderGroup, NO_GROUP, type GroupPlace, type SortFacts } from "../arrange"
import type { Destination } from "../bindings/Destination"
import type { DownloadProgress } from "../bindings/DownloadProgress"
import type { ListGroup } from "../bindings/ListGroup"
import type { QueueItem } from "../bindings/QueueItem"
import type { QueueState } from "../bindings/QueueState"
import { ArrangeControls, GroupedList } from "../components/ArrangedList"
import { Button } from "../components/Button"
import { Card } from "../components/Card"
import { CopyDetailsButton } from "../components/CopyDetailsButton"
import { EmptyState } from "../components/EmptyState"
import { FailureFixes } from "../components/FailureFixes"
import { IconButton } from "../components/IconButton"
import { Menu } from "../components/Menu"
import { Page } from "../components/Page"
import { ProgressBar } from "../components/ProgressBar"
import { StatusMessage } from "../components/StatusMessage"
import { Thumbnail } from "../components/Thumbnail"
import { WaysToAdd, type WayToAdd } from "../components/WaysToAdd"
import { describeDownloadError, describeHistoryError, fixFor, type ErrorDescription } from "../errors"
import { formatBytes, formatContents, formatDuration, formatSpeed, joinDetails } from "../format"
import { pickSummary } from "../postFiles"
import { describeHold, holdEnd, useTimeLeft } from "../holds"
import { useDownloadDestinations, useListArrangement, useQueue, useRuleGroupOf } from "../queries"
import { useDownloadProgress } from "../queueEvents"
import { strings } from "../strings"
import { useOfferUndo, type UndoOffer } from "../undo"

/** The buttons for the whole queue, each shown while a download is in a state it acts on, as the row's own button. An
 * action that removes something offers to undo it. */
const WHOLE_QUEUE_ACTIONS: readonly {
  label: string
  states: readonly QueueState[]
  action: (offerUndo: (offer: UndoOffer) => void) => Promise<void>
}[] = [
  { label: strings.queue.pauseAll, states: ["running", "waiting"], action: stopAllDownloads },
  { label: strings.queue.resumeAll, states: ["stopped"], action: resumeAllDownloads },
  { label: strings.queue.retryFailed, states: ["failed"], action: retryFailedDownloads },
  { label: strings.queue.clearFinished, states: ["done"], action: clearFinishedWithUndo },
]

/** Clear Finished, then an offer for a few seconds to put the downloads back where they were. */
async function clearFinishedWithUndo(offerUndo: (offer: UndoOffer) => void): Promise<void> {
  const cleared = await clearFinishedDownloads()
  if (cleared > 0) {
    offerUndo({
      message: strings.queue.cleared(cleared),
      undo: restoreClearedDownloads,
      describeError: describeDownloadError,
    })
  }
}

/** The ways to add links an empty queue offers: by hand first. */
const QUEUE_WAYS: readonly WayToAdd[] = ["paste", "drop", "watch", "shortcut", "bookmark", "sendButtons"]

/** Every download added and not cleared yet, grouped and sorted as its toolbar says. */
export function QueueView() {
  const { data: items = [] } = useQueue()
  const [queueError, setQueueError] = useState<ErrorDescription | null>(null)
  const { arrangement, ready, change: changeArrangement, error: arrangeError } = useListArrangement("queue")
  const { group, byStatus } = arrangement
  // What picks each download's folder is asked again, since the queue keeps only the folder.
  const ruleGroupOf = useRuleGroupOf(group === "rule")
  const destinations = useDownloadDestinations(
    items.map((item) => ({ video: item.video, presetId: item.preset?.id ?? null, enabled: group === "rule" })),
  )
  const destinationOf = new Map(items.map((item, index) => [item.id, destinations[index]]))
  const groups = arrangeNested(
    items,
    arrangement,
    queueFacts,
    byStatus ? queueStatusGroup : null,
    group === "none" ? null : (item) => queueGroup(item, group, destinationOf.get(item.id), ruleGroupOf),
  )
  const actions = WHOLE_QUEUE_ACTIONS.filter(({ states }) => items.some((item) => states.includes(item.state)))

  const offerUndo = useOfferUndo()
  const actOnAll = (action: (offerUndo: (offer: UndoOffer) => void) => Promise<void>) => {
    setQueueError(null)
    action(offerUndo).catch((error: unknown) => setQueueError(describeDownloadError(error)))
  }

  return (
    <Page
      title={strings.nav.queue}
      actions={
        actions.length > 0 &&
        actions.map(({ label, action }) => (
          <Button key={label} size="sm" onClick={() => actOnAll(action)}>
            {label}
          </Button>
        ))
      }
    >
      {queueError && <StatusMessage tone="danger" {...queueError} />}
      {items.length === 0 ? (
        <EmptyState icon={ListVideo} title={strings.queue.emptyTitle} body={strings.queue.emptyBody}>
          <WaysToAdd ways={QUEUE_WAYS} />
        </EmptyState>
      ) : (
        <div className="flex flex-col gap-3">
          <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
            <p className="text-fg-secondary">{strings.queue.count(items.length)}</p>
            <ArrangeControls
              arrangement={arrangement}
              disabled={!ready}
              onChange={(changes) => void changeArrangement(changes)}
            />
          </div>
          {arrangeError && <StatusMessage tone="danger" {...arrangeError} />}
          <GroupedList
            groups={groups}
            grouped={group !== "none" || byStatus}
            keyOf={(item) => item.id}
            renderItem={(item) => <QueueRow item={item} />}
          />
        </div>
      )}
    </Page>
  )
}

/** Where downloads stand, in the order their groups come: running first, done last. */
const QUEUE_STATUSES: readonly QueueState[] = ["running", "waiting", "stopped", "failed", "done"]

function queueFacts(item: QueueItem): SortFacts {
  const { video } = item
  return { id: item.id, addedAt: item.addedAt, title: video.title, uploader: video.uploader, site: video.siteName }
}

/** The group of where a download stands. */
function queueStatusGroup(item: QueueItem): GroupPlace {
  return { key: item.state, label: strings.queue.groups[item.state], rank: QUEUE_STATUSES.indexOf(item.state) }
}

/** The group a download falls in: its folder, its site, or what picks its folder (none until that's known). */
function queueGroup(
  item: QueueItem,
  group: Exclude<ListGroup, "none">,
  destination: Destination | undefined,
  ruleGroupOf: ((destination: Destination) => GroupPlace) | null,
): GroupPlace {
  switch (group) {
    case "destination":
      return folderGroup(item.folder)
    case "site":
      return { key: item.video.site, label: item.video.siteName, rank: 0 }
    case "rule":
      if (item.folderChosen) return chosenFolderGroup(item.folder)
      return destination && ruleGroupOf ? ruleGroupOf(destination) : NO_GROUP
  }
}

function QueueRow({ item }: { item: QueueItem }) {
  const progress = useDownloadProgress(item)
  const [actionError, setActionError] = useState<ErrorDescription | null>(null)
  const stage = stageLabel(item, progress)
  const hold = item.state === "waiting" ? item.hold : null
  const holdTimeLeft = useTimeLeft(hold ? holdEnd(hold) : null)

  const act = (action: (id: number) => Promise<void>, describe = describeDownloadError) => {
    setActionError(null)
    action(item.id).catch((error: unknown) => setActionError(describe(error)))
  }
  const actOnFile = (action: (downloadId: number) => Promise<void>) => {
    if (!item.result) return
    setActionError(null)
    action(item.result.downloadId).catch((error: unknown) => setActionError(describeHistoryError(error)))
  }

  return (
    <Card className="flex items-start gap-3 p-3">
      <Thumbnail key={item.video.thumbnailUrl} url={item.video.thumbnailUrl} className="w-24" />
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <p className="truncate font-medium" title={item.video.title}>
          {item.video.title}
        </p>
        <p className="truncate text-xs text-fg-secondary tabular-nums">
          {joinDetails([
            item.preset?.name ?? pickSummary(item.video) ?? formatContents(item.video),
            stage,
            ...sizeDetails(item, progress),
          ])}
        </p>
        {item.state === "running" && <ProgressBar value={progress?.fraction ?? undefined} label={stage} />}
        {hold && <StatusMessage tone="neutral" message={describeHold(hold)} detail={holdTimeLeft ?? undefined} />}
        {item.state === "failed" && item.error && (
          <StatusMessage tone="danger" {...describeDownloadError(item.error)}>
            {fixFor(item.error) && <FailureFixes error={item.error} />}
          </StatusMessage>
        )}
        {actionError && <StatusMessage tone="danger" {...actionError} />}
      </div>
      <div className="flex shrink-0 flex-wrap items-start justify-end gap-1">
        {(item.state === "running" || item.state === "waiting") && (
          <IconButton
            label={strings.queue.stop}
            icon={CircleStop}
            variant="secondary"
            onClick={() => act(stopDownload)}
          />
        )}
        {hold && hold.kind !== "hourlyLimit" && (
          <IconButton label={strings.queue.tryNow} icon={RefreshCw} onClick={() => act(tryDownloadNow)} />
        )}
        {item.state === "stopped" && (
          <IconButton
            label={strings.queue.resume}
            icon={Play}
            variant="secondary"
            onClick={() => act(resumeDownload)}
          />
        )}
        {item.state === "failed" && (
          <IconButton
            label={strings.queue.retry}
            icon={RefreshCw}
            variant="secondary"
            onClick={() => act(retryDownload)}
          />
        )}
        {item.state === "done" && (
          <>
            <IconButton
              label={strings.queue.openFile}
              icon={Play}
              variant="secondary"
              onClick={() => actOnFile(openDownload)}
            />
            <IconButton
              label={strings.queue.showInFolder}
              icon={FolderOpen}
              onClick={() => actOnFile(showDownloadInFolder)}
            />
          </>
        )}
        {(item.state === "failed" || item.state === "done") && (
          <IconButton label={strings.queue.openLog} icon={FileText} onClick={() => act(openDownloadLog)} />
        )}
        {item.state === "failed" && item.error && (
          <CopyDetailsButton id={item.id} error={describeDownloadError(item.error)} onError={setActionError} />
        )}
        {item.state !== "done" && (
          <IconButton label={strings.queue.cancel} icon={X} onClick={() => act(cancelDownload)} />
        )}
        {(item.state === "waiting" || item.state === "stopped" || item.state === "failed") && (
          <Menu
            label={strings.queue.moreActions}
            items={[
              ...(item.state === "waiting" ? [{ label: strings.queue.downloadNext, onSelect: () => act(downloadNext) }] : []),
              { label: strings.queue.changeFolder, onSelect: () => act((id) => changeFolder(id, item)) },
              ...(item.folderChosen
                ? [{ label: strings.queue.usualFolder, onSelect: () => act((id) => moveToFolder(id, item, null)) }]
                : []),
            ]}
          />
        )}
      </div>
    </Card>
  )
}

/** Change Folder…: asks for a folder, then saves the download there. */
async function changeFolder(id: number, item: QueueItem): Promise<void> {
  const folder = await pickFolder(strings.queue.changeFolderTitle)
  if (folder) await moveToFolder(id, item, folder)
}

/** Saves the download in `folder`, or the usual one, asking first when a stopped one would lose what it downloaded. */
async function moveToFolder(id: number, item: QueueItem, folder: string | null): Promise<void> {
  const startsOver = item.state === "stopped"
  if (
    startsOver &&
    !(await confirmWithUser({
      title: strings.queue.changeFolderStoppedTitle,
      message: strings.queue.changeFolderStoppedMessage,
      confirmLabel: strings.queue.changeFolderConfirm,
      cancelLabel: strings.dialogs.cancel,
    }))
  ) {
    return
  }
  await setQueueFolder(id, folder)
}

/** What the item is doing: its state, or while it runs, the stage yt-dlp is at. */
function stageLabel(item: QueueItem, progress: DownloadProgress | null): string {
  const stages = strings.queue.stages
  return item.state === "running" ? stages[progress?.stage ?? "starting"] : stages[item.state]
}

/** Size, speed and time left while downloading; the file's size once done. */
function sizeDetails(item: QueueItem, progress: DownloadProgress | null): (string | null)[] {
  if (item.state === "done") {
    return [item.result?.fileSize ? formatBytes(item.result.fileSize) : null]
  }
  if (!progress || progress.totalBytes === null) return []
  return [
    strings.queue.progress(formatBytes(progress.downloadedBytes ?? 0), formatBytes(progress.totalBytes)),
    progress.speed ? formatSpeed(progress.speed) : null,
    progress.eta !== null && progress.fraction !== 1 ? strings.queue.timeLeft(formatDuration(progress.eta)) : null,
  ]
}

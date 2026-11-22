import { useMemo, useState } from "react"
import { ListVideo } from "lucide-react"
import {
  clearFinishedDownloads,
  restoreClearedDownloads,
  resumeAllDownloads,
  retryFailedDownloads,
  stopAllDownloads,
} from "../api"
import {
  arrangeNested,
  chaptersOf,
  chosenFolderGroup,
  folderGroup,
  groupPath,
  LIST_GROUPS,
  LIST_SORTS,
  NO_GROUP,
  sameFolder,
  unknownRuleGroup,
  type GroupPlace,
  type SortFacts,
} from "../arrange"
import type { Destination } from "../bindings/Destination"
import type { FolderFrom } from "../bindings/FolderFrom"
import type { ListGroup } from "../bindings/ListGroup"
import type { QueueItem } from "../bindings/QueueItem"
import type { QueueState } from "../bindings/QueueState"
import { useChapters } from "../chapters"
import { ArrangeControls, GroupedList, type Bundling } from "../components/ArrangedList"
import { Button } from "../components/Button"
import { EmptyState } from "../components/EmptyState"
import { Page } from "../components/Page"
import { PickerViewControls } from "../components/PickerViewControls"
import { QueueRow } from "../components/QueueRow"
import { QueueSourceGroup } from "../components/QueueSourceGroup"
import { QueueTiles } from "../components/QueueTiles"
import { StatusMessage } from "../components/StatusMessage"
import { WaysToAdd, type WayToAdd } from "../components/WaysToAdd"
import { describeDownloadError, type ErrorDescription } from "../errors"
import { useFoldedGroups } from "../foldedGroups"
import {
  useDownloadDestinations,
  useFolderFromGroupOf,
  useListArrangement,
  usePickerLayout,
  useQueue,
  useRuleGroupOf,
} from "../queries"
import { QueueLookContext } from "../queueItems"
import { sourceKey } from "../queueSources"
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

/** The downloads picked together from a channel, playlist or account stay together under its heading. */
const BY_SOURCE: Bundling<QueueItem> = {
  keyOf: sourceKey,
  render: ({ items, folded, onFold, rows }) =>
    items[0].source && (
      <QueueSourceGroup source={items[0].source} items={items} folded={folded} onFold={onFold} rows={rows} />
    ),
}

/** The ways to add links an empty queue offers: by hand first. */
const QUEUE_WAYS: readonly WayToAdd[] = ["paste", "drop", "watch", "shortcut", "bookmark", "sendButtons"]

/**
 * The Downloads view (the queue, in the code): every download added and not cleared yet, grouped and sorted as its
 * toolbar says, those picked together from one source under its heading, as rows (List) or pictures (Masonry, Gallery),
 * one choice for the whole view, kept in its header so it stays in sight however far the list is scrolled. Grouped, the
 * sidebar lists its groups under it as chapters, a status's groups as their subchapters.
 */
export function QueueView() {
  const { data: items = [] } = useQueue()
  const [queueError, setQueueError] = useState<ErrorDescription | null>(null)
  const { arrangement, ready, change: changeArrangement, error: arrangeError } = useListArrangement("queue")
  const { group, byStatus } = arrangement
  // Grouped by rule, each download is grouped by what gave it its folder when it was added, which it keeps. One added
  // before that was kept is asked about again, as the rules say now.
  const folderFromGroupOf = useFolderFromGroupOf(group === "rule")
  const ruleGroupOf = useRuleGroupOf(group === "rule")
  const destinations = useDownloadDestinations(
    items.map((item) => ({
      video: item.video,
      presetId: item.preset?.id ?? null,
      ruleId: item.ruleId,
      enabled: group === "rule" && !item.folderChosen && item.folderFrom === null,
    })),
  )
  const destinationOf = new Map(items.map((item, index) => [item.id, destinations[index]]))
  const groupers: Groupers = { folderFromGroupOf, ruleGroupOf }
  const groups = arrangeNested(
    items,
    arrangement,
    queueFacts,
    byStatus ? queueStatusGroup : null,
    group === "none" ? null : (item) => queueGroup(item, group, destinationOf.get(item.id), groupers),
  )
  const actions = WHOLE_QUEUE_ACTIONS.filter(({ states }) => items.some((item) => states.includes(item.state)))
  const grouped = group !== "none" || byStatus
  const look = useMemo(() => ({ byStatus }), [byStatus])
  const { layout, change: changeLayout } = usePickerLayout("queueSources")
  const tiles = layout.view === "list" ? null : layout.view

  const folds = useFoldedGroups("queue")
  const { scrollRef, markShown } = useChapters({
    view: "queue",
    chapters: grouped ? chaptersOf(groups) : [],
    // A status's group folded away is unfolded to go to one of its groups.
    reveal: (id) => {
      const holder = groupPath("", id.split("/")[1])
      if (holder !== id && folds.collapsed.has(holder)) folds.toggle(holder)
    },
  })

  const offerUndo = useOfferUndo()
  const actOnAll = (action: (offerUndo: (offer: UndoOffer) => void) => Promise<void>) => {
    setQueueError(null)
    action(offerUndo).catch((error: unknown) => setQueueError(describeDownloadError(error)))
  }

  return (
    <Page
      title={strings.nav.queue}
      scrollRef={scrollRef}
      onScroll={markShown}
      actions={
        actions.length > 0 &&
        actions.map(({ label, action }) => (
          <Button key={label} size="sm" onClick={() => actOnAll(action)}>
            {label}
          </Button>
        ))
      }
      viewControls={items.length > 0 && <PickerViewControls layout={layout} onChange={changeLayout} />}
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
              groups={LIST_GROUPS}
              sorts={LIST_SORTS}
              disabled={!ready}
              onChange={(changes) => void changeArrangement(changes)}
            />
          </div>
          {arrangeError && <StatusMessage tone="danger" {...arrangeError} />}
          <QueueLookContext value={look}>
            <GroupedList
              view="queue"
              groups={groups}
              grouped={grouped}
              keyOf={(item) => item.id}
              renderItem={(item) => <QueueRow item={item} />}
              renderItems={tiles ? (run) => <QueueTiles items={run} view={tiles} size={layout.size} /> : undefined}
              bundles={BY_SOURCE}
              folds={folds}
            />
          </QueueLookContext>
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

/** What groups a download by rule, once the rules are known. */
interface Groupers {
  /** By what gave it its folder when it was added. */
  folderFromGroupOf: ((from: FolderFrom, folder: string) => GroupPlace) | null
  /** By what the rules give it now. */
  ruleGroupOf: ((destination: Destination) => GroupPlace) | null
}

/**
 * The group a download falls in: its folder, its site, or what gave it its folder when it was added (none until that's
 * known). One added before that was kept goes by what the rules say now when they still give it the same folder, else
 * under its folder as an unknown rule's.
 */
function queueGroup(
  item: QueueItem,
  group: Exclude<ListGroup, "none">,
  destination: Destination | undefined,
  { folderFromGroupOf, ruleGroupOf }: Groupers,
): GroupPlace {
  switch (group) {
    case "destination":
      return folderGroup(item.folder)
    case "site":
      return { key: item.video.site, label: item.video.siteName, rank: 0 }
    case "rule":
      if (item.folderChosen) return chosenFolderGroup(item.folder)
      if (item.folderFrom) return folderFromGroupOf ? folderFromGroupOf(item.folderFrom, item.folder) : NO_GROUP
      if (!destination || !ruleGroupOf) return NO_GROUP
      return sameFolder(destination.folder, item.folder) ? ruleGroupOf(destination) : unknownRuleGroup(item.folder)
  }
}

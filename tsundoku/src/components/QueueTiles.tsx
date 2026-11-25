import { useState } from "react"
import {
  Check,
  CircleAlert,
  CircleStop,
  ClipboardCopy,
  Clock,
  Download,
  FileText,
  Film,
  FolderInput,
  FolderOpen,
  Images,
  KeyRound,
  Link,
  ListStart,
  PanelBottomClose,
  PanelBottomOpen,
  Play,
  RefreshCw,
  RotateCcw,
  ShieldCheck,
  X,
  type LucideIcon,
} from "lucide-react"
import {
  cancelDownload,
  downloadNext,
  openCheckWindow,
  openDownload,
  resumeDownload,
  retryDownload,
  showDownloadInFolder,
  stopDownload,
  tryDownloadNow,
} from "../api"
import type { QueueItem } from "../bindings/QueueItem"
import type { QueueState } from "../bindings/QueueState"
import { useCopyLink } from "../copyLink"
import { describeDownloadError, type ErrorDescription } from "../errors"
import { joinDetails } from "../format"
import { describeHold } from "../holds"
import { useDownloadProgress } from "../queueEvents"
import {
  changeFolder,
  copyDownloadDetails,
  moveToFolder,
  shownStage,
  sizeDetails,
  stageLabel,
  useItemActions,
  useQueueLook,
} from "../queueItems"
import { tileActions, type TileAction } from "../queueSources"
import { strings } from "../strings"
import type { TileRows } from "../tileRows"
import { ItemTile, ItemTiles, type TilePlace } from "./ItemTiles"
import { TileLabel } from "./MediaTiles"
import type { MenuItem } from "./Menu"
import { ProgressBar } from "./ProgressBar"
import type { RowOpening } from "./QueueRow"
import { StatusMessage } from "./StatusMessage"

const text = strings.tiles

interface QueueTilesProps {
  items: readonly QueueItem[]
  view: "masonry" | "gallery"
  /** The step of the size buttons (`TILE_WIDTHS`). */
  size: number
  /** The view's download whose row shows in the details panel. */
  rows: TileRows<RowOpening>
}

/**
 * Downloads as pictures (`ItemTiles`), in Masonry or Gallery: a run of them in the Downloads view, or a source's under
 * its heading. Each is its picture alone (its title when it has none), with where it stands in its corner (left out
 * where its status's heading says it) and a bar while it downloads; its title and status are its hover text. A
 * right-click (or the menu key) opens its actions, which its row shows as buttons in List; a click shows its row in
 * the details panel along the bottom of the view (`QueueView`), with everything a row has (its error, log, password
 * field), and a second click hides it. A double-click on a finished one opens its file. Gallery cuts them to squares
 * when they're all posts, else to 16:9.
 */
export function QueueTiles({ items, view, size, rows }: QueueTilesProps) {
  const [error, setError] = useState<ErrorDescription | null>(null)
  const galleryRatio = items.every((item) => item.video.post !== null) ? 1 : 16 / 9

  return (
    <div className="flex flex-col gap-2">
      {error && <StatusMessage tone="danger" {...error} />}
      <ItemTiles
        items={items}
        view={view}
        size={size}
        keyOf={(item) => item.id}
        galleryRatio={galleryRatio}
        shownId={rows.shown?.id ?? null}
        renderTile={(item, place) => (
          <QueueTile item={item} place={place} onShow={rows.show} onHide={rows.hide} onError={setError} />
        )}
      />
    </div>
  )
}

interface QueueTileProps {
  item: QueueItem
  place: TilePlace
  onShow: (id: number, opens: RowOpening) => void
  onHide: () => void
  onError: (error: ErrorDescription | null) => void
}

/** The icon in a picture's corner for where its download stands, and its color. */
const STATE_ICONS: Record<QueueState, { icon: LucideIcon; className: string }> = {
  running: { icon: Download, className: "text-accent" },
  waiting: { icon: Clock, className: "text-fg-secondary" },
  stopped: { icon: CircleStop, className: "text-fg-secondary" },
  failed: { icon: CircleAlert, className: "text-danger" },
  done: { icon: Check, className: "text-success" },
}

function QueueTile({ item, place, onShow, onHide, onError }: QueueTileProps) {
  const progress = useDownloadProgress(item)
  const { act, removeWithUndo, actOnFile } = useItemActions(item, onError)
  const copyLink = useCopyLink()
  const { shown, labels } = place

  const { video, state } = item
  const hold = state === "waiting" ? item.hold : null
  // The check window saving the file shows as a download, with its progress.
  const savingInWindow = hold?.kind === "windowDownload"
  const stage = savingInWindow ? strings.queue.stages.downloading : stageLabel(item, progress)
  const look = useQueueLook()
  const status = joinDetails([shownStage(stage, item, look), ...sizeDetails(item, progress)])
  const error = state === "failed" && item.error ? describeDownloadError(item.error) : null
  const downloading = state === "running" || savingInWindow
  const percent = downloading && progress?.fraction != null ? Math.round(progress.fraction * 100) : null
  const isPost = video.post !== null
  const shownState = downloading ? "running" : state
  const { icon: StateIcon, className: stateColor } = STATE_ICONS[shownState]
  // Grouped by status, the heading says where it stands, unless the check window downloads it while it waits.
  const showsState = !(look.byStatus && shownState === state)
  const cornerLabel = showsState || (percent !== null && labels !== "hidden")
  const opensFile = state === "done" && item.result !== null
  const hint = [video.title, status, hold && describeHold(hold), error?.message, opensFile && text.openHint, text.hint]
    .filter(Boolean)
    .join("\n")

  const copyDetails = () => {
    if (!error) return
    onError(null)
    copyDownloadDetails(item.id, error).catch((caught: unknown) => onError(describeDownloadError(caught)))
  }
  const menuItem = (action: TileAction): MenuItem => {
    const queue = strings.queue
    switch (action) {
      case "stop":
        return { label: queue.stop, icon: CircleStop, onSelect: () => act(stopDownload) }
      case "tryNow":
        return { label: queue.tryNow, icon: RefreshCw, onSelect: () => act(tryDownloadNow) }
      case "openCheck":
        return { label: queue.openCheck, icon: ShieldCheck, onSelect: () => act(openCheckWindow) }
      case "enterPassword":
        return { label: queue.enterPassword, icon: KeyRound, onSelect: () => onShow(item.id, "password") }
      case "resume":
        return { label: queue.resume, icon: Play, onSelect: () => act(resumeDownload) }
      case "retry":
        return { label: queue.retry, icon: RefreshCw, onSelect: () => act(retryDownload) }
      case "openFile":
        return { label: queue.openFile, icon: Play, onSelect: () => actOnFile(openDownload) }
      case "showInFolder":
        return { label: queue.showInFolder, icon: FolderOpen, onSelect: () => actOnFile(showDownloadInFolder) }
      case "showLog":
        return { label: queue.showLog, icon: FileText, onSelect: () => onShow(item.id, "log") }
      case "copyDetails":
        return { label: queue.copyDetails, icon: ClipboardCopy, onSelect: copyDetails }
      case "copyLink":
        return { label: strings.copyLink.label, icon: Link, onSelect: () => copyLink(video.url) }
      case "details":
        return shown
          ? { label: text.hideDetails, icon: PanelBottomClose, onSelect: onHide }
          : { label: text.showDetails, icon: PanelBottomOpen, onSelect: () => onShow(item.id, null) }
      case "downloadNext":
        return { label: queue.downloadNext, icon: ListStart, onSelect: () => act(downloadNext) }
      case "changeFolder":
        return { label: queue.changeFolder, icon: FolderInput, onSelect: () => act((id) => changeFolder(id, item)) }
      case "usualFolder":
        return { label: queue.usualFolder, icon: RotateCcw, onSelect: () => act((id) => moveToFolder(id, item, null)) }
      case "remove":
        return { label: queue.removeFinished, icon: X, danger: true, onSelect: () => act(removeWithUndo) }
      case "cancel":
        return { label: queue.cancel, icon: X, danger: true, onSelect: () => act(cancelDownload) }
    }
  }

  return (
    <ItemTile
      id={item.id}
      place={place}
      title={video.title}
      thumbnailUrl={video.thumbnailUrl}
      fallbackIcon={isPost ? Images : Film}
      shape={isPost ? 4 / 5 : 16 / 9}
      hint={hint}
      ariaLabel={joinDetails([video.title, status])}
      dimmed={(state === "waiting" || state === "stopped") && !savingInWindow}
      failed={state === "failed"}
      corner={
        cornerLabel && (
          <TileLabel>
            <span className="flex items-center gap-1">
              {showsState && <StateIcon size={12} className={stateColor} />}
              {percent !== null && labels !== "hidden" && `${percent}%`}
            </span>
          </TileLabel>
        )
      }
      bottom={downloading && <ProgressBar value={progress?.fraction ?? undefined} label={stage} />}
      menuLabel={text.menu(video.title)}
      menuItems={() => tileActions(item).map(menuItem)}
      onToggle={() => (shown ? onHide() : onShow(item.id, null))}
      onOpen={opensFile ? () => actOnFile(openDownload) : undefined}
    />
  )
}

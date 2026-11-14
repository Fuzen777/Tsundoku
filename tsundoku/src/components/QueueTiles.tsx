import { useRef, useState } from "react"
import {
  Check,
  ChevronUp,
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
  ListStart,
  PanelTopOpen,
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
import { describeDownloadError, type ErrorDescription } from "../errors"
import { joinDetails } from "../format"
import { describeHold } from "../holds"
import { ownShape } from "../pickerLayout"
import { useDownloadProgress } from "../queueEvents"
import { changeFolder, copyDownloadDetails, moveToFolder, sizeDetails, stageLabel, useItemActions } from "../queueItems"
import { tileActions, type TileAction } from "../queueSources"
import { strings } from "../strings"
import { ContextMenu, type MenuClose } from "./ContextMenu"
import { TiledItems, TileLabel, type TileLabels } from "./MediaTiles"
import type { MenuItem } from "./Menu"
import { ProgressBar } from "./ProgressBar"
import type { RowOpening } from "./QueueRow"
import { classes } from "./styles"

const text = strings.queue.tiles

interface QueueTilesProps {
  items: readonly QueueItem[]
  view: "masonry" | "gallery"
  /** The step of the size buttons (`TILE_WIDTHS`). */
  size: number
  /** Gallery's one shape for every picture, as width over height. */
  galleryRatio: number
  /** The download whose row is shown, if any. */
  shownId: number | null
  /** Shows a download's row, opened on its log or its password field, or on neither. */
  onShow: (id: number, opens: RowOpening) => void
  onHide: () => void
  /** Called with why an action failed, and with null when a new one starts. */
  onError: (error: ErrorDescription | null) => void
}

/**
 * A source's downloads as pictures, in Masonry or Gallery: each with where it stands in its corner, a bar while it
 * downloads, and its title and progress under it. A right-click (or the menu key) opens its actions, which its row shows
 * as buttons in List; a click shows its row above the pictures, with everything a row has (its error, log, password
 * field), and a second click hides it.
 */
export function QueueTiles({ items, view, size, galleryRatio, shownId, onShow, onHide, onError }: QueueTilesProps) {
  return (
    <div className="p-1">
      <TiledItems
        items={items}
        view={view}
        size={size}
        render={(item, labels) => (
          <QueueTile
            key={item.id}
            item={item}
            ratio={view === "gallery" ? galleryRatio : null}
            labels={labels}
            shown={item.id === shownId}
            onShow={onShow}
            onHide={onHide}
            onError={onError}
          />
        )}
      />
    </div>
  )
}

interface QueueTileProps {
  item: QueueItem
  /** Gallery's shape, or null for the picture's own (Masonry). */
  ratio: number | null
  labels: TileLabels
  shown: boolean
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

function QueueTile({ item, ratio, labels, shown, onShow, onHide, onError }: QueueTileProps) {
  const progress = useDownloadProgress(item)
  const tile = useRef<HTMLDivElement>(null)
  const [menuAt, setMenuAt] = useState<{ x: number; y: number } | null>(null)
  const [failed, setFailed] = useState(false)
  const [ownRatio, setOwnRatio] = useState<number | null>(null)
  const { act, removeWithUndo, actOnFile } = useItemActions(item, onError)

  const { video, state } = item
  const hold = state === "waiting" ? item.hold : null
  // The check window saving the file shows as a download, with its progress.
  const savingInWindow = hold?.kind === "windowDownload"
  const stage = savingInWindow ? strings.queue.stages.downloading : stageLabel(item, progress)
  const status = joinDetails([stage, ...sizeDetails(item, progress)])
  const error = state === "failed" && item.error ? describeDownloadError(item.error) : null
  const downloading = state === "running" || savingInWindow
  const percent = downloading && progress?.fraction != null ? Math.round(progress.fraction * 100) : null
  const pictured = video.thumbnailUrl !== null && !failed
  const isPost = video.post !== null
  const FallbackIcon = isPost ? Images : Film
  const { icon: StateIcon, className: stateColor } = STATE_ICONS[downloading ? "running" : state]
  const hint = [video.title, status, hold && describeHold(hold), error?.message, text.hint].filter(Boolean).join("\n")

  const toggle = () => (shown ? onHide() : onShow(item.id, null))
  const closeMenu = (why: MenuClose) => {
    setMenuAt(null)
    if (why !== "elsewhere") tile.current?.focus()
  }
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
      case "details":
        return shown
          ? { label: text.hideDetails, icon: ChevronUp, onSelect: onHide }
          : { label: text.showDetails, icon: PanelTopOpen, onSelect: () => onShow(item.id, null) }
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
    <li className="min-w-0">
      <div
        ref={tile}
        role="button"
        tabIndex={0}
        aria-label={joinDetails([video.title, status])}
        aria-haspopup="menu"
        aria-pressed={shown}
        title={hint}
        onClick={toggle}
        onContextMenu={(event) => {
          event.preventDefault()
          setMenuAt({ x: event.clientX, y: event.clientY })
        }}
        onKeyDown={(event) => {
          if (event.key === "ContextMenu" || (event.shiftKey && event.key === "F10")) {
            const box = event.currentTarget.getBoundingClientRect()
            setMenuAt({ x: box.left + 8, y: box.top + 8 })
          } else if (event.key === "Enter" || event.key === " ") {
            toggle()
          } else {
            return
          }
          event.preventDefault()
        }}
        style={{ aspectRatio: ratio ?? (pictured ? (ownRatio ?? (isPost ? 4 / 5 : 16 / 9)) : 1) }}
        className={classes(
          "relative block w-full cursor-pointer overflow-hidden rounded-md bg-input",
          "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent",
          shown ? "ring-2 ring-accent" : state === "failed" && "ring-2 ring-danger",
        )}
      >
        {pictured ? (
          <img
            src={video.thumbnailUrl ?? undefined}
            alt=""
            loading="lazy"
            referrerPolicy="no-referrer"
            className={classes(
              "size-full object-cover transition-opacity",
              (state === "waiting" || state === "stopped") && !savingInWindow && "opacity-50",
            )}
            onLoad={(event) => setOwnRatio(ownShape(event.currentTarget))}
            onError={() => setFailed(true)}
          />
        ) : (
          <span className="flex size-full items-center justify-center text-fg-muted">
            <FallbackIcon size={labels === "full" ? 24 : 16} />
          </span>
        )}
        <span className={classes("absolute top-1 left-1 origin-top-left", labels !== "full" && "scale-75")}>
          <TileLabel>
            <span className="flex items-center gap-1">
              <StateIcon size={12} className={stateColor} />
              {percent !== null && labels !== "hidden" && `${percent}%`}
            </span>
          </TileLabel>
        </span>
        {downloading && (
          <span className="absolute inset-x-1 bottom-1">
            <ProgressBar value={progress?.fraction ?? undefined} label={stage} />
          </span>
        )}
      </div>
      {labels !== "hidden" && (
        <div className="mt-1 flex min-w-0 flex-col">
          <p className="truncate text-xs font-medium">{video.title}</p>
          <p className={classes("truncate text-xs tabular-nums", error ? "text-danger" : "text-fg-secondary")}>
            {status}
          </p>
        </div>
      )}
      {menuAt && (
        <ContextMenu
          label={text.menu(video.title)}
          items={tileActions(item).map(menuItem)}
          at={menuAt}
          onClose={closeMenu}
        />
      )}
    </li>
  )
}

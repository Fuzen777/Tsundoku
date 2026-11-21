import { useLayoutEffect, useRef, useState } from "react"
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
import { ContextMenu, type MenuClose } from "./ContextMenu"
import { TiledItems, TileLabel, type TileLabels } from "./MediaTiles"
import type { MenuItem } from "./Menu"
import { ProgressBar } from "./ProgressBar"
import { QueueRow, type RowOpening } from "./QueueRow"
import { StatusMessage } from "./StatusMessage"
import { classes } from "./styles"

const text = strings.queue.tiles

interface QueueTilesProps {
  items: readonly QueueItem[]
  view: "masonry" | "gallery"
  /** The step of the size buttons (`TILE_WIDTHS`). */
  size: number
}

/** A download's row shown under its line of pictures, opened as asked; `opened` counts each ask, so asking again opens
 * it afresh (on its log, say, when it's already shown). */
interface ShownRow {
  id: number
  opens: RowOpening
  opened: number
}

/**
 * Downloads as pictures, in Masonry or Gallery: a run of them in the Downloads view, or a source's under its heading.
 * Each is its picture alone (its title when it has none), with where it stands in its corner (left out where its
 * status's heading says it) and a bar while it downloads; its title and status are its hover text. A right-click (or
 * the menu key) opens its actions, which its row shows as buttons in List; a click shows its row under its line of
 * pictures, with everything a row has (its error, log, password field), keeping the picture where it was on screen, and
 * a second click hides it. A double-click on a finished one opens its file. Gallery cuts them to squares when they're
 * all posts, else to 16:9.
 */
export function QueueTiles({ items, view, size }: QueueTilesProps) {
  const [error, setError] = useState<ErrorDescription | null>(null)
  const [shownRow, setShownRow] = useState<ShownRow | null>(null)
  const shownItem = shownRow && items.find((item) => item.id === shownRow.id)
  const shownBox = useRef<HTMLDivElement>(null)
  const box = useRef<HTMLDivElement>(null)
  // Where the picture asked about was, so it stays there as the row moves from under another line to under its own.
  const held = useRef<{ id: number; top: number } | null>(null)
  const galleryRatio = items.every((item) => item.video.post !== null) ? 1 : 16 / 9

  const pictureOf = (id: number) => box.current?.querySelector(`[data-download="${id}"]`)
  const showRow = (id: number, opens: RowOpening) => {
    const top = pictureOf(id)?.getBoundingClientRect().top
    held.current = top === undefined ? null : { id, top }
    setShownRow((shown) => ({ id, opens, opened: (shown?.opened ?? 0) + 1 }))
  }
  const hideRow = () => setShownRow(null)
  // Before it's painted: the picture asked about back where it was (a row shown higher up has closed), then the row
  // shown under it brought into sight, scrolling no more than that takes.
  useLayoutEffect(() => {
    const before = held.current
    held.current = null
    const picture = before && box.current?.querySelector(`[data-download="${before.id}"]`)
    if (before && picture) {
      const moved = picture.getBoundingClientRect().top - before.top
      const scroller = scrollerOf(picture)
      if (moved !== 0 && scroller) scroller.scrollTop += moved
    }
    if (shownRow) shownBox.current?.scrollIntoView({ block: "nearest" })
  }, [shownRow])

  return (
    <div ref={box} className="flex flex-col gap-2">
      {error && <StatusMessage tone="danger" {...error} />}
      {/* Padded for the focus outline. */}
      <div className="p-1">
        <TiledItems
          items={items}
          view={view}
          size={size}
          after={
            shownItem
              ? {
                  index: items.indexOf(shownItem),
                  content: (
                    <div ref={shownBox} className="scroll-my-2">
                      <QueueRow
                        key={`${shownItem.id}-${shownRow.opened}`}
                        item={shownItem}
                        opens={shownRow.opens}
                        onHide={hideRow}
                      />
                    </div>
                  ),
                }
              : undefined
          }
          render={(item, labels) => (
            <QueueTile
              key={item.id}
              item={item}
              ratio={view === "gallery" ? galleryRatio : null}
              labels={labels}
              shown={item.id === shownItem?.id}
              onShow={showRow}
              onHide={hideRow}
              onError={setError}
            />
          )}
        />
      </div>
    </div>
  )
}

/** The box an element scrolls in: the nearest one above it that scrolls, or the page. */
function scrollerOf(element: Element): Element | null {
  for (let parent = element.parentElement; parent; parent = parent.parentElement) {
    const { overflowY } = getComputedStyle(parent)
    if ((overflowY === "auto" || overflowY === "scroll") && parent.scrollHeight > parent.clientHeight) return parent
  }
  return document.scrollingElement
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

/** Pictures' own shapes once loaded, by address: a picture drawn anew in another column (Masonry starts the pictures
 * under an open row again) has its size from its first frame, so nothing above or beside it moves. */
const knownShapes = new Map<string, number>()

function QueueTile({ item, ratio, labels, shown, onShow, onHide, onError }: QueueTileProps) {
  const progress = useDownloadProgress(item)
  const tile = useRef<HTMLDivElement>(null)
  const [menuAt, setMenuAt] = useState<{ x: number; y: number } | null>(null)
  const [failed, setFailed] = useState(false)
  const thumbnail = item.video.thumbnailUrl
  const [ownRatio, setOwnRatio] = useState<number | null>(() =>
    thumbnail === null ? null : (knownShapes.get(thumbnail) ?? null),
  )
  const { act, removeWithUndo, actOnFile } = useItemActions(item, onError)

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
  const pictured = video.thumbnailUrl !== null && !failed
  const isPost = video.post !== null
  const FallbackIcon = isPost ? Images : Film
  const shownState = downloading ? "running" : state
  const { icon: StateIcon, className: stateColor } = STATE_ICONS[shownState]
  // Grouped by status, the heading says where it stands, unless the check window downloads it while it waits.
  const showsState = !(look.byStatus && shownState === state)
  const cornerLabel = showsState || (percent !== null && labels !== "hidden")
  const opensFile = state === "done" && item.result !== null
  const dimmed = (state === "waiting" || state === "stopped") && !savingInWindow
  const hint = [video.title, status, hold && describeHold(hold), error?.message, opensFile && text.openHint, text.hint]
    .filter(Boolean)
    .join("\n")

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
        data-download={item.id}
        role="button"
        tabIndex={0}
        aria-label={joinDetails([video.title, status])}
        aria-haspopup="menu"
        aria-pressed={shown}
        title={hint}
        // The second click of a double-click leaves the row shown by the first.
        onClick={(event) => {
          if (event.detail < 2) toggle()
        }}
        onDoubleClick={() => {
          if (opensFile) actOnFile(openDownload)
        }}
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
            className={classes("size-full object-cover transition-opacity", dimmed && "opacity-50")}
            onLoad={(event) => {
              const shape = ownShape(event.currentTarget)
              setOwnRatio(shape)
              if (shape !== null && thumbnail !== null) knownShapes.set(thumbnail, shape)
            }}
            onError={() => setFailed(true)}
          />
        ) : labels === "hidden" ? (
          <span className="flex size-full items-center justify-center text-fg-muted">
            <FallbackIcon size={16} />
          </span>
        ) : (
          // Without a picture, its title stands in for it.
          <span className="flex size-full items-center justify-center overflow-hidden px-2 pt-6 pb-2">
            <span
              className={classes(
                "line-clamp-3 text-center text-xs font-medium break-words",
                dimmed ? "text-fg-muted" : "text-fg-secondary",
              )}
            >
              {video.title}
            </span>
          </span>
        )}
        {cornerLabel && (
          <span className={classes("absolute top-1 left-1 origin-top-left", labels !== "full" && "scale-75")}>
            <TileLabel>
              <span className="flex items-center gap-1">
                {showsState && <StateIcon size={12} className={stateColor} />}
                {percent !== null && labels !== "hidden" && `${percent}%`}
              </span>
            </TileLabel>
          </span>
        )}
        {downloading && (
          <span className="absolute inset-x-1 bottom-1">
            <ProgressBar value={progress?.fraction ?? undefined} label={stage} />
          </span>
        )}
      </div>
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

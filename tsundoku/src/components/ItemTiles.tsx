import { useRef, useState, type ReactNode } from "react"
import type { LucideIcon } from "lucide-react"
import { ownShape } from "../pickerLayout"
import { ContextMenu, type MenuClose } from "./ContextMenu"
import { TiledItems, type TileLabels } from "./MediaTiles"
import type { MenuItem } from "./Menu"
import { PresentItem } from "./PresentItem"
import { classes, LIFTED_SHADOW, LIFTS_ON_HOVER } from "./styles"
import { sitePictureUrl } from "../api"

/*
  A list's items as pictures, in Masonry or Gallery, a click opening an item's full row in the details panel along the
  bottom of the view (`DetailsDock`), so no picture moves: the Downloads, Pending and History views all show their items
  this way, so they look and behave alike. `ItemTile` is one picture, `ItemTiles` a run of them, and `useShownRow`
  (tileRows.ts) the row shown, one for the whole view.
*/

/** How one picture is drawn in its run. */
export interface TilePlace {
  /** Gallery's shape, or null for the picture's own (Masonry). */
  ratio: number | null
  labels: TileLabels
  /** Whether its row shows in the details panel. */
  shown: boolean
}

interface ItemTilesProps<Item> {
  items: readonly Item[]
  view: "masonry" | "gallery"
  /** The step of the size buttons (`TILE_WIDTHS`). */
  size: number
  keyOf: (item: Item) => number
  /** Gallery's one shape for every picture, as width over height. */
  galleryRatio: number
  /** The item whose row the details panel shows, if any. */
  shownId: number | null
  renderTile: (item: Item, place: TilePlace) => ReactNode
}

/** A run of items as pictures, the one whose row the details panel shows ringed. */
export function ItemTiles<Item>({ items, view, size, keyOf, galleryRatio, shownId, renderTile }: ItemTilesProps<Item>) {
  return (
    // Padded for the focus outline.
    <div className="p-1">
      <TiledItems
        items={items}
        view={view}
        size={size}
        render={(item, labels) => (
          <PresentItem key={keyOf(item)} itemKey={keyOf(item)} as="li" className="min-w-0">
            {renderTile(item, {
              ratio: view === "gallery" ? galleryRatio : null,
              labels,
              shown: keyOf(item) === shownId,
            })}
          </PresentItem>
        )}
      />
    </div>
  )
}

/** Pictures' own shapes once loaded, by address: a picture drawn anew (a view switched, a group unfolded) has its size
 * from its first frame, so nothing above or beside it moves. */
const knownShapes = new Map<string, number>()

interface ItemTileProps {
  /** Its item's ID, which `useShownRow` finds it by. */
  id: number
  place: TilePlace
  title: string
  thumbnailUrl: string | null
  /** Shown on a tiny tile without a picture, where its title wouldn't fit. */
  fallbackIcon: LucideIcon
  /** Its shape in Masonry until its picture loads, as width over height. */
  shape: number
  /** Its hover text: its title, then what it is and where it stands, a line each. */
  hint: string
  /** What screen readers call it. */
  ariaLabel: string
  /** Faded, such as a download that waits or a link not read yet. */
  dimmed?: boolean
  /** A ring in the danger color, such as a failed download's. */
  failed?: boolean
  /** A label in its top corner, such as where it stands: at the top right when it has a tick box, else the top left. */
  corner?: ReactNode
  /** Across its bottom, such as a download's progress. */
  bottom?: ReactNode
  /** Its tick box at the top left, for acting on several items at once: shown while it's ticked, while any is
   * (`showing`), or while the pointer or the focus is on the picture. */
  selection?: { selected: boolean; showing: boolean; label: string; onSelect: (selected: boolean) => void }
  menuLabel: string
  /** Its actions, read when its menu opens: what its row's buttons do. */
  menuItems: () => MenuItem[]
  /** A click (Enter or Space): shows or hides its row. */
  onToggle: () => void
  /** A double-click, such as opening a download's file. */
  onOpen?: () => void
}

/**
 * One item as its picture alone (its title when it has none), its row's actions in its right-click menu (or the menu
 * key's, or Shift+F10's), a click showing its row and a double-click doing `onOpen`. In Masonry, one without a picture
 * is square; with one, its shape before it loads is `shape`.
 */
export function ItemTile({
  id,
  place,
  title,
  thumbnailUrl,
  fallbackIcon: FallbackIcon,
  shape,
  hint,
  ariaLabel,
  dimmed = false,
  failed: inDanger = false,
  corner,
  bottom,
  selection,
  menuLabel,
  menuItems,
  onToggle,
  onOpen,
}: ItemTileProps) {
  const { ratio, labels, shown } = place
  const tile = useRef<HTMLDivElement>(null)
  const [menuAt, setMenuAt] = useState<{ x: number; y: number } | null>(null)
  const [failed, setFailed] = useState(false)
  const [ownRatio, setOwnRatio] = useState<number | null>(() =>
    thumbnailUrl === null ? null : (knownShapes.get(thumbnailUrl) ?? null),
  )
  const pictured = thumbnailUrl !== null && !failed
  const closeMenu = (why: MenuClose) => {
    setMenuAt(null)
    if (why !== "elsewhere") tile.current?.focus()
  }

  return (
    <div className={classes("relative", LIFTS_ON_HOVER)}>
      <div
        ref={tile}
        data-tile={id}
        role="button"
        tabIndex={0}
        aria-label={ariaLabel}
        aria-haspopup="menu"
        aria-pressed={shown}
        title={hint}
        // The second click of a double-click leaves the row shown by the first.
        onClick={(event) => {
          if (event.detail < 2) onToggle()
        }}
        onDoubleClick={onOpen}
        onContextMenu={(event) => {
          event.preventDefault()
          setMenuAt({ x: event.clientX, y: event.clientY })
        }}
        onKeyDown={(event) => {
          if (event.key === "ContextMenu" || (event.shiftKey && event.key === "F10")) {
            const box = event.currentTarget.getBoundingClientRect()
            setMenuAt({ x: box.left + 8, y: box.top + 8 })
          } else if (event.key === "Enter" || event.key === " ") {
            onToggle()
          } else {
            return
          }
          event.preventDefault()
        }}
        style={{ aspectRatio: ratio ?? (pictured ? (ownRatio ?? shape) : 1) }}
        className={classes(
          "relative block w-full cursor-pointer overflow-hidden rounded-md bg-input",
          LIFTED_SHADOW,
          "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent",
          shown ? "ring-2 ring-accent" : inDanger && "ring-2 ring-danger",
        )}
      >
        {pictured ? (
          <img
            src={sitePictureUrl(thumbnailUrl)}
            alt=""
            loading="lazy"
            referrerPolicy="no-referrer"
            className={classes("size-full object-cover transition-opacity", dimmed && "opacity-50")}
            onLoad={(event) => {
              const loaded = ownShape(event.currentTarget)
              setOwnRatio(loaded)
              if (loaded !== null && thumbnailUrl !== null) knownShapes.set(thumbnailUrl, loaded)
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
              {title}
            </span>
          </span>
        )}
        {corner && (
          <span
            className={classes(
              "absolute top-1",
              selection ? "right-1 origin-top-right" : "left-1 origin-top-left",
              labels !== "full" && "scale-75",
            )}
          >
            {corner}
          </span>
        )}
        {bottom && <span className="absolute inset-x-1 bottom-1">{bottom}</span>}
      </div>
      {selection && (
        // Beside the picture's button rather than inside it, so it's a control of its own.
        <input
          type="checkbox"
          className={classes(
            "absolute top-1.5 left-1.5 size-4 accent-accent transition-opacity",
            !selection.selected &&
              !selection.showing &&
              "opacity-0 group-focus-within:opacity-100 group-hover:opacity-100 focus-visible:opacity-100",
          )}
          aria-label={selection.label}
          title={selection.label}
          checked={selection.selected}
          onChange={(event) => selection.onSelect(event.target.checked)}
        />
      )}
      {menuAt && <ContextMenu label={menuLabel} items={menuItems()} at={menuAt} onClose={closeMenu} />}
    </div>
  )
}

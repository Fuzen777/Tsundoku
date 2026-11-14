import { useLayoutEffect, useRef, useState, type ReactNode } from "react"
import { Check, type LucideIcon } from "lucide-react"
import { columnsFor, intoColumns, ownShape, SMALL_TILE, TILE_WIDTHS, TINY_TILE, tileWidthIn } from "../pickerLayout"
import { classes } from "./styles"

/** What a tile says under its picture, as the post strip's marks do. */
export interface TileMark {
  text: string
  /** `done`: after a check in the success color. `notPicked`: in the accent, with a ring around the picture. `plain`:
   * muted. */
  tone: "done" | "notPicked" | "plain"
}

/** One picture to tick in a picker's Masonry or Gallery. */
export interface MediaTile {
  key: string
  thumbnailUrl: string | null
  /** Shown on the empty tile when there's no picture, or it doesn't load. */
  fallbackIcon: LucideIcon
  /** Its checkbox's name, for screen readers. */
  label: string
  /** Its hover text. */
  hint: string
  picked: boolean
  /** Can't be ticked, such as an item already in the queue. */
  locked?: boolean
  /** Its shape until its picture loads, as width over height: Masonry then shows the picture's own, or a square
   * when there's none. */
  ratio: number
  /** At the bottom left, such as a file's number in its post. */
  badge?: ReactNode
  /** At the bottom right, such as a video's length. */
  corner?: ReactNode
  mark?: TileMark | null
  onToggle: (picked: boolean) => void
}

interface MediaTilesProps {
  tiles: readonly MediaTile[]
  view: "masonry" | "gallery"
  /** The step of the size buttons (`TILE_WIDTHS`). */
  size: number
  /** Gallery's one shape for every picture, as width over height. */
  galleryRatio: number
  disabled?: boolean
  /** The scrolling box's own classes, such as its height. */
  className?: string
}

/**
 * Pictures to tick, in as many columns as fit at the size picked: in Gallery, a grid of pictures cut to one shape; in
 * Masonry, columns of pictures in their own shapes, dealt left to right so the first ones stay at the top. The whole
 * tile ticks it; Tab and Space work.
 */
export function MediaTiles({ tiles, view, size, galleryRatio, disabled, className }: MediaTilesProps) {
  return (
    // The scrollbar's room is kept, so its coming and going never changes how many columns fit.
    <div className={classes("overflow-y-auto [scrollbar-gutter:stable]", className)}>
      {/* Padded for the focus outline. */}
      <div className="p-1">
        <TiledItems
          items={tiles}
          view={view}
          size={size}
          render={(spec, labels) => (
            <Tile
              key={spec.key}
              spec={spec}
              ratio={view === "gallery" ? galleryRatio : null}
              labels={labels}
              disabled={disabled}
            />
          )}
        />
      </div>
    </div>
  )
}

interface TiledItemsProps<Item> {
  items: readonly Item[]
  view: "masonry" | "gallery"
  /** The step of the size buttons (`TILE_WIDTHS`). */
  size: number
  /** One item's tile: an `li`, with its key, its labels as they fit at the size the tiles came to. */
  render: (item: Item, labels: TileLabels) => ReactNode
}

/**
 * Items as tiles, in as many columns as fit the box's width at the size picked: a grid in Gallery, columns dealt left to
 * right in Masonry, so the first ones stay at the top. The pickers' pictures and the Queue's both lay out this way.
 */
export function TiledItems<Item>({ items, view, size, render }: TiledItemsProps<Item>) {
  const inner = useRef<HTMLDivElement>(null)
  const [width, setWidth] = useState(0)

  // Measured before the first paint, so the columns never show at one before settling.
  useLayoutEffect(() => {
    const box = inner.current
    if (!box) return
    setWidth(box.getBoundingClientRect().width)
    const observer = new ResizeObserver(([entry]) => setWidth(entry.contentRect.width))
    observer.observe(box)
    return () => observer.disconnect()
  }, [])

  const columns = columnsFor(width, size)
  const tileWidth = width > 0 ? tileWidthIn(width, columns) : TILE_WIDTHS[size]
  const labels: TileLabels = tileWidth < TINY_TILE ? "hidden" : tileWidth < SMALL_TILE ? "small" : "full"
  return (
    <div ref={inner}>
      {view === "gallery" ? (
        <ul className="grid gap-2" style={{ gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))` }}>
          {items.map((item) => render(item, labels))}
        </ul>
      ) : (
        <div className="flex items-start gap-2">
          {intoColumns(items, columns).map((column, index) => (
            <ul key={index} className="flex min-w-0 flex-1 flex-col gap-2">
              {column.map((item) => render(item, labels))}
            </ul>
          ))}
        </div>
      )}
    </div>
  )
}

/** How a picture's corner labels show: as they are, shrunk on a small picture, or left out on a tiny one, where they
 * would cover it. */
export type TileLabels = "full" | "small" | "hidden"


interface TileProps {
  spec: MediaTile
  /** Gallery's shape, or null for the picture's own (Masonry). */
  ratio: number | null
  labels: TileLabels
  disabled?: boolean
}

/** One picture, with its box at the top left, dimmed while unticked, and its mark under it. In Masonry, one without a
 * picture is square. */
function Tile({ spec, ratio, labels, disabled }: TileProps) {
  const [failed, setFailed] = useState(false)
  const [ownRatio, setOwnRatio] = useState<number | null>(null)
  const { thumbnailUrl, fallbackIcon: FallbackIcon, picked, locked, mark } = spec
  const pictured = thumbnailUrl !== null && !failed
  const shown = picked && !locked
  const inactive = disabled || locked
  return (
    <li className="min-w-0">
      <label
        title={spec.hint}
        style={{ aspectRatio: ratio ?? (pictured ? (ownRatio ?? spec.ratio) : 1) }}
        className={classes(
          "relative block w-full overflow-hidden rounded-md bg-input",
          "has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-offset-2 has-[:focus-visible]:outline-accent",
          inactive ? "cursor-default" : "cursor-pointer",
          mark?.tone === "notPicked" && "ring-2 ring-accent",
        )}
      >
        {pictured ? (
          <img
            src={thumbnailUrl}
            alt=""
            loading="lazy"
            referrerPolicy="no-referrer"
            className={classes("size-full object-cover transition-opacity", !shown && "opacity-35")}
            onLoad={(event) => {
              const shape = ownShape(event.currentTarget)
              if (shape !== null) setOwnRatio(shape)
            }}
            onError={() => setFailed(true)}
          />
        ) : (
          <span
            className={classes(
              "flex size-full items-center justify-center text-fg-muted transition-opacity",
              !shown && "opacity-35",
            )}
          >
            <FallbackIcon size={labels === "full" ? 24 : 16} />
          </span>
        )}
        <input
          type="checkbox"
          className="absolute top-1.5 left-1.5 size-4 accent-accent"
          aria-label={spec.label}
          checked={shown}
          disabled={inactive}
          onChange={(event) => spec.onToggle(event.target.checked)}
        />
        {spec.badge && labels !== "hidden" && (
          <span className={classes("absolute bottom-1 left-1 origin-bottom-left", labels === "small" && "scale-75")}>
            {spec.badge}
          </span>
        )}
        {spec.corner && labels !== "hidden" && (
          <span className={classes("absolute right-1 bottom-1 origin-bottom-right", labels === "small" && "scale-75")}>
            {spec.corner}
          </span>
        )}
      </label>
      {mark && (
        <p
          title={mark.text}
          className={classes(
            "mt-1 flex items-center gap-1 text-xs",
            mark.tone === "notPicked" ? "font-medium text-accent" : "text-fg-muted",
          )}
        >
          {mark.tone === "done" && <Check size={12} className="shrink-0 text-success" />}
          <span className="min-w-0 truncate">{mark.text}</span>
        </p>
      )}
    </li>
  )
}

/** A label laid over a picture's corner, such as a file's number or a video's length. */
export function TileLabel({ children }: { children: ReactNode }) {
  return <span className="block rounded bg-bg/85 px-1 text-xs text-fg tabular-nums">{children}</span>
}

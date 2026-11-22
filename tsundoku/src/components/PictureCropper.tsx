import { useEffect, useRef, useState, type KeyboardEvent, type PointerEvent } from "react"
import type { ListingKind } from "../bindings/ListingKind"
import type { PictureCrop } from "../bindings/PictureCrop"
import { coverCrop, cropPlacement, frameAspect, MOST_ZOOM, movedBy, zoomedTo, zoomOf } from "../pictureCrop"
import { strings } from "../strings"
import { CroppedPicture } from "./SourcePicture"
import { classes } from "./styles"
import { sitePictureUrl } from "../api"

const text = strings.watched.pictures

/** How much a wheel's notch zooms, and an arrow key moves (a share of the frame). */
const WHEEL_ZOOM = 1.15
const KEY_STEP = 0.05

interface PictureCropperProps {
  kind: ListingKind
  url: string
  /** The area shown, or null to start with the whole picture's middle, which `onChange` then gives once it loads. */
  crop: PictureCrop | null
  onChange: (crop: PictureCrop) => void
  disabled?: boolean
}

/**
 * Picks the area of a picture a source shows: the picture behind a frame of the source's shape (a circle, or a
 * playlist's 16:9), dimmed outside it, moved by dragging it or the arrow keys and zoomed by the wheel, + and −, or the
 * Zoom slider; the source's picture as it will show beside it. Give it `key={url}`, so another picture starts afresh.
 */
export function PictureCropper({ kind, url, crop, onChange, disabled = false }: PictureCropperProps) {
  const frame = frameAspect(kind)
  // The picture's width over its height, once it's loaded.
  const [aspect, setAspect] = useState<number | null>(null)
  const [failed, setFailed] = useState(false)
  const frameRef = useRef<HTMLDivElement>(null)
  const areaRef = useRef<HTMLDivElement>(null)
  // Where a drag started: the pointer's place and the area then, which each move is worked out from.
  const drag = useRef<{ pointer: number; x: number; y: number; from: PictureCrop } | null>(null)
  const shown = aspect !== null ? (crop ?? coverCrop(aspect, frame)) : null
  const zoom = shown && aspect ? zoomOf(shown, aspect, frame) : 1

  // The latest values, for the wheel's listener, which is added once.
  const latest = useRef({ shown, aspect, onChange, disabled })
  useEffect(() => {
    latest.current = { shown, aspect, onChange, disabled }
  })

  // A wheel over the picture zooms it rather than scrolling the view: React's own wheel listener can't stop that.
  useEffect(() => {
    const area = areaRef.current
    if (!area) return
    const zoomByWheel = (event: WheelEvent) => {
      const { shown, aspect, onChange, disabled } = latest.current
      if (!shown || !aspect || disabled) return
      event.preventDefault()
      const factor = WHEEL_ZOOM ** (-event.deltaY / 100)
      onChange(zoomedTo(shown, zoomOf(shown, aspect, frame) * factor, aspect, frame))
    }
    area.addEventListener("wheel", zoomByWheel, { passive: false })
    return () => area.removeEventListener("wheel", zoomByWheel)
  }, [frame])

  const zoomTo = (next: number) => {
    if (shown && aspect) onChange(zoomedTo(shown, next, aspect, frame))
  }

  const startDrag = (event: PointerEvent<HTMLDivElement>) => {
    if (!shown || disabled || event.button !== 0) return
    event.currentTarget.setPointerCapture(event.pointerId)
    drag.current = { pointer: event.pointerId, x: event.clientX, y: event.clientY, from: shown }
  }
  const moveDrag = (event: PointerEvent<HTMLDivElement>) => {
    const start = drag.current
    const box = frameRef.current?.getBoundingClientRect()
    if (!start || start.pointer !== event.pointerId || !box) return
    onChange(movedBy(start.from, (event.clientX - start.x) / box.width, (event.clientY - start.y) / box.height))
  }
  const endDrag = (event: PointerEvent<HTMLDivElement>) => {
    if (drag.current?.pointer === event.pointerId) drag.current = null
  }

  const moveByKeys = (event: KeyboardEvent<HTMLDivElement>) => {
    if (!shown || disabled) return
    const moves: Record<string, [number, number]> = {
      ArrowLeft: [-KEY_STEP, 0],
      ArrowRight: [KEY_STEP, 0],
      ArrowUp: [0, -KEY_STEP],
      ArrowDown: [0, KEY_STEP],
    }
    const move = moves[event.key]
    if (move) {
      event.preventDefault()
      // The arrow moves the area the frame shows, the way the picture would go under a drag the other way.
      onChange(movedBy(shown, -move[0], -move[1]))
    } else if (event.key === "+" || event.key === "=") {
      event.preventDefault()
      zoomTo(zoom * WHEEL_ZOOM)
    } else if (event.key === "-") {
      event.preventDefault()
      zoomTo(zoom / WHEEL_ZOOM)
    }
  }

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-end gap-3">
        <div
          ref={areaRef}
          role="application"
          aria-label={text.area}
          title={text.areaHint}
          tabIndex={disabled ? -1 : 0}
          onPointerDown={startDrag}
          onPointerMove={moveDrag}
          onPointerUp={endDrag}
          onPointerCancel={endDrag}
          onKeyDown={moveByKeys}
          className={classes(
            "relative h-48 w-72 shrink-0 touch-none overflow-hidden rounded-md bg-input select-none focus-visible:outline-2 focus-visible:outline-accent",
            shown && !disabled && "cursor-grab active:cursor-grabbing",
            disabled && "opacity-60",
          )}
        >
          <div
            ref={frameRef}
            className={classes("absolute inset-0 m-auto", kind === "playlist" ? "aspect-video h-36" : "size-40")}
          >
            {!failed && (
              <img
                src={sitePictureUrl(url)}
                alt=""
                referrerPolicy="no-referrer"
                draggable={false}
                className={classes("absolute max-w-none", !shown && "invisible")}
                style={shown ? cropPlacement(shown) : undefined}
                onLoad={(event) => {
                  const { naturalWidth, naturalHeight } = event.currentTarget
                  if (naturalWidth === 0 || naturalHeight === 0) return
                  const loaded = naturalWidth / naturalHeight
                  setAspect(loaded)
                  if (!crop) onChange(coverCrop(loaded, frame))
                }}
                onError={() => setFailed(true)}
              />
            )}
          </div>
          {/* Outside the frame, the picture is dimmed; the frame's edge in the accent. */}
          <div
            aria-hidden
            className={classes(
              "pointer-events-none absolute inset-0 m-auto shadow-[0_0_0_100vmax_color-mix(in_oklab,var(--color-bg)_70%,transparent)] outline-2 outline-accent",
              kind === "playlist" ? "aspect-video h-36 rounded-md" : "size-40 rounded-full",
            )}
          />
          {failed && (
            <p className="absolute inset-x-4 bottom-3 text-center text-xs text-fg-secondary">{text.notLoaded}</p>
          )}
        </div>
        {shown && !failed && <CroppedPicture kind={kind} url={url} crop={shown} />}
      </div>
      <label className="flex w-72 items-center gap-2 text-xs text-fg-secondary">
        {text.zoom}
        <input
          type="range"
          min={1}
          max={MOST_ZOOM}
          step={0.01}
          value={zoom}
          disabled={!shown || disabled}
          onChange={(event) => zoomTo(Number(event.target.value))}
          className="flex-1 accent-accent"
        />
      </label>
    </div>
  )
}

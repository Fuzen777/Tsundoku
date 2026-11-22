import { useEffect, useRef, useState } from "react"
import { Check, Film, Image as ImageIcon, Play } from "lucide-react"
import type { PostFile } from "../bindings/PostFile"
import { formatBytes } from "../format"
import { usePickerLayout } from "../queries"
import { strings } from "../strings"
import { MediaTiles, TileLabel, type MediaTile } from "./MediaTiles"
import { PickerViewControls } from "./PickerViewControls"
import { classes } from "./styles"
import { sitePictureUrl } from "../api"

const text = strings.postFiles

/** What a file says about a post downloaded before: taken then, or left out. */
type Mark = "downloaded" | "notPicked"

export interface PostFilesStripProps {
  files: readonly PostFile[]
  /** The numbers of the files ticked. */
  picked: ReadonlySet<number>
  /** For a post downloaded before that left files out: those files, marked Not Picked, the others Downloaded. */
  leftOut: readonly number[] | null
  /** Downloaded before: without `leftOut`, every file is marked Downloaded. */
  downloadedBefore: boolean
  disabled?: boolean
  onChange: (picked: Set<number>) => void
}

/**
 * A post's images and videos to pick from, in the view its picker keeps: as pictures (Gallery, cut square, or Masonry,
 * in their own shapes), each with a box, a play mark on videos and its number in the post, or as rows (List). Above
 * them, a box for all of them with how many are ticked, and the view's controls. For a post downloaded before, each
 * file says whether it was downloaded or not picked then.
 */
export function PostFilesStrip({ files, picked, leftOut, downloadedBefore, disabled, onChange }: PostFilesStripProps) {
  const allBox = useRef<HTMLInputElement>(null)
  const { layout, change: changeLayout } = usePickerLayout("postFiles")
  const count = files.filter((file) => picked.has(file.number)).length
  const some = count > 0 && count < files.length

  useEffect(() => {
    if (allBox.current) allBox.current.indeterminate = some
  }, [some])

  function toggle(number: number, on: boolean) {
    const next = new Set(picked)
    if (on) next.add(number)
    else next.delete(number)
    onChange(next)
  }

  function markOf(number: number): Mark | null {
    if (leftOut) return leftOut.includes(number) ? "notPicked" : "downloaded"
    return downloadedBefore ? "downloaded" : null
  }

  return (
    <div className="flex flex-col gap-2">
      <div className="flex flex-wrap items-center justify-between gap-x-3 gap-y-2">
        <label className="flex w-fit items-center gap-2 text-fg-secondary">
          <input
            ref={allBox}
            type="checkbox"
            className="size-4 accent-accent"
            aria-label={text.selectAll}
            title={text.selectAll}
            checked={count === files.length}
            disabled={disabled}
            onChange={(event) => onChange(new Set(event.target.checked ? files.map((file) => file.number) : []))}
          />
          <span className="whitespace-nowrap tabular-nums">{text.count(count, files.length)}</span>
        </label>
        <PickerViewControls layout={layout} onChange={changeLayout} />
      </div>
      {layout.view === "list" ? (
        <ul className="-mx-2 flex max-h-[min(28rem,50vh)] flex-col overflow-y-auto">
          {files.map((file) => (
            <PostFileRow
              key={file.number}
              file={file}
              picked={picked.has(file.number)}
              mark={markOf(file.number)}
              disabled={disabled}
              onToggle={(on) => toggle(file.number, on)}
            />
          ))}
        </ul>
      ) : (
        <MediaTiles
          tiles={files.map((file) => fileTile(file, picked.has(file.number), markOf(file.number), (on) => toggle(file.number, on)))}
          view={layout.view}
          size={layout.size}
          galleryRatio={1}
          disabled={disabled}
          className="-mx-1 max-h-[min(28rem,50vh)]"
        />
      )}
    </div>
  )
}

/** A file as a picture: its number at the bottom left, a play mark on a video, and its mark under it. */
function fileTile(file: PostFile, picked: boolean, mark: Mark | null, onToggle: (picked: boolean) => void): MediaTile {
  const name = text.file(file.number, file.video)
  return {
    key: String(file.number),
    thumbnailUrl: file.thumbnailUrl,
    fallbackIcon: file.video ? Film : ImageIcon,
    label: name,
    hint: mark === "notPicked" ? text.notPickedHint : mark === "downloaded" ? text.downloadedHint : name,
    picked,
    ratio: 1,
    badge: <TileLabel>{file.number}</TileLabel>,
    corner: file.video && (
      <span className="block rounded bg-accent-fill p-1 text-on-accent">
        <Play size={10} fill="currentColor" />
      </span>
    ),
    mark:
      mark === "notPicked"
        ? { text: text.notPicked, tone: "notPicked" }
        : mark === "downloaded"
          ? { text: text.downloaded, tone: "done" }
          : null,
    onToggle,
  }
}

interface PostFileRowProps {
  file: PostFile
  picked: boolean
  mark: Mark | null
  disabled?: boolean
  onToggle: (picked: boolean) => void
}

/** A file as a row: its box, a small square of its picture, "Image 3" (its file name as the hover text), its mark and
 * its size. */
function PostFileRow({ file, picked, mark, disabled, onToggle }: PostFileRowProps) {
  const [failed, setFailed] = useState(false)
  const label = text.file(file.number, file.video)
  const FallbackIcon = file.video ? Film : ImageIcon
  return (
    <li>
      <label
        title={mark === "notPicked" ? text.notPickedHint : mark === "downloaded" ? text.downloadedHint : undefined}
        className={classes(
          "flex items-center gap-3 rounded-md px-2 py-1 transition-colors",
          disabled ? "cursor-default" : "cursor-pointer hover:bg-input",
        )}
      >
        <input
          type="checkbox"
          className="size-4 shrink-0 accent-accent"
          aria-label={label}
          checked={picked}
          disabled={disabled}
          onChange={(event) => onToggle(event.target.checked)}
        />
        <span
          className={classes(
            "relative flex size-10 shrink-0 items-center justify-center overflow-hidden rounded bg-input text-fg-muted",
            mark === "notPicked" && "ring-2 ring-accent",
          )}
        >
          {file.thumbnailUrl && !failed ? (
            <img
              src={sitePictureUrl(file.thumbnailUrl)}
              alt=""
              loading="lazy"
              referrerPolicy="no-referrer"
              className="size-full object-cover"
              onError={() => setFailed(true)}
            />
          ) : (
            <FallbackIcon size={16} />
          )}
          {file.video && (
            <span className="absolute right-0.5 bottom-0.5 rounded bg-accent-fill p-0.5 text-on-accent">
              <Play size={8} fill="currentColor" />
            </span>
          )}
        </span>
        <span className={classes("min-w-0 flex-1 truncate", !picked && "text-fg-muted")} title={file.name}>
          {label}
        </span>
        {mark && (
          <span
            className={classes(
              "flex shrink-0 items-center gap-1 text-xs",
              mark === "notPicked" ? "font-medium text-accent" : "text-fg-muted",
            )}
          >
            {mark === "downloaded" && <Check size={12} className="shrink-0 text-success" />}
            {mark === "notPicked" ? text.notPicked : text.downloaded}
          </span>
        )}
        {file.size !== undefined && (
          <span className="w-16 shrink-0 text-right text-xs text-fg-secondary tabular-nums">{formatBytes(file.size)}</span>
        )}
      </label>
    </li>
  )
}

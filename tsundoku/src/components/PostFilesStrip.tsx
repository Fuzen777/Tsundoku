import { useEffect, useRef, useState } from "react"
import { Check, Film, Image as ImageIcon, Play } from "lucide-react"
import type { PostFile } from "../bindings/PostFile"
import { strings } from "../strings"
import { classes } from "./styles"

const text = strings.postFiles

/** What a tile says about a post downloaded before: taken then, or left out. */
type Mark = "downloaded" | "notPicked"

interface PostFilesStripProps {
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
 * A post's images and videos as small tiles, each with a box to tick it, a play mark on videos, and its number in the
 * post. Above them, a box for all of them with how many are ticked. For a post downloaded before, each tile says
 * whether it was downloaded or not picked then. The tiles wrap, and scroll past a few rows.
 */
export function PostFilesStrip({ files, picked, leftOut, downloadedBefore, disabled, onChange }: PostFilesStripProps) {
  const allBox = useRef<HTMLInputElement>(null)
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
        <span className="tabular-nums">{text.count(count, files.length)}</span>
      </label>
      <ul className="flex max-h-64 flex-wrap gap-2 overflow-y-auto p-0.5">
        {files.map((file) => (
          <PostFileTile
            key={file.number}
            file={file}
            picked={picked.has(file.number)}
            mark={markOf(file.number)}
            disabled={disabled}
            onToggle={(on) => toggle(file.number, on)}
          />
        ))}
      </ul>
    </div>
  )
}

interface PostFileTileProps {
  file: PostFile
  picked: boolean
  mark: Mark | null
  disabled?: boolean
  onToggle: (picked: boolean) => void
}

/** One file: its picture (or an icon when there's none, such as an X video), dimmed while unticked. */
function PostFileTile({ file, picked, mark, disabled, onToggle }: PostFileTileProps) {
  const [failed, setFailed] = useState(false)
  const name = text.file(file.number, file.video)
  const hint = mark === "notPicked" ? text.notPickedHint : mark === "downloaded" ? text.downloadedHint : name
  const FallbackIcon = file.video ? Film : ImageIcon
  return (
    <li className="w-22">
      <label
        title={hint}
        className={classes(
          "relative block size-22 overflow-hidden rounded-md bg-input",
          "has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-offset-2 has-[:focus-visible]:outline-accent",
          disabled ? "cursor-default" : "cursor-pointer",
          mark === "notPicked" && "ring-2 ring-accent",
        )}
      >
        {file.thumbnailUrl && !failed ? (
          <img
            src={file.thumbnailUrl}
            alt=""
            loading="lazy"
            referrerPolicy="no-referrer"
            className={classes("size-full object-cover transition-opacity", !picked && "opacity-35")}
            onError={() => setFailed(true)}
          />
        ) : (
          <span
            className={classes(
              "flex size-full items-center justify-center text-fg-muted transition-opacity",
              !picked && "opacity-35",
            )}
          >
            <FallbackIcon size={24} />
          </span>
        )}
        <input
          type="checkbox"
          className="absolute top-1.5 left-1.5 size-4 accent-accent"
          aria-label={name}
          checked={picked}
          disabled={disabled}
          onChange={(event) => onToggle(event.target.checked)}
        />
        <span className="absolute bottom-1 left-1 rounded bg-bg/85 px-1 text-xs text-fg tabular-nums">{file.number}</span>
        {file.video && (
          <span className="absolute right-1 bottom-1 rounded bg-accent-fill p-1 text-on-accent">
            <Play size={10} fill="currentColor" />
          </span>
        )}
      </label>
      {mark && (
        <p
          className={classes(
            "mt-1 flex items-center gap-1 truncate text-xs",
            mark === "notPicked" ? "font-medium text-accent" : "text-fg-muted",
          )}
        >
          {mark === "downloaded" && <Check size={12} className="shrink-0 text-success" />}
          {mark === "notPicked" ? text.notPicked : text.downloaded}
        </p>
      )}
    </li>
  )
}

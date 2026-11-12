import { memo, useCallback, useDeferredValue, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react"
import { Check, ChevronDown, ChevronRight, Folder } from "lucide-react"
import type { SharedFile } from "../bindings/SharedFile"
import { formatBytes } from "../format"
import {
  FOLDED_ABOVE,
  folderPaths,
  folderState,
  matchingFiles,
  pickedBytes,
  sharedTree,
  shownPaths,
  visibleRows,
  withFiles,
  type SharedFolder,
} from "../sharedFiles"
import { strings } from "../strings"
import { useVirtualRows } from "../virtualRows"
import { TextInput } from "./Field"
import { Tooltip } from "./Tooltip"
import { classes } from "./styles"

const text = strings.sharedFiles

/** The filter shows for a folder of more files than this. */
const FILTER_ABOVE = 10
/** How far each level of folders is indented, in pixels. */
const INDENT_PX = 16
/** Every row's height, in pixels (`h-7`): the list draws only the rows on screen, and stands in for the others. */
const ROW_HEIGHT_PX = 28
/** How far past each edge of the list rows are drawn, in pixels. */
const OVERSCAN_PX = 280

/** What a file says about a folder downloaded before: taken then, or not. */
type Mark = "downloaded" | "notDownloaded"

interface SharedFilesListProps {
  files: readonly SharedFile[]
  /** The places of the files ticked, in `files`. */
  picked: ReadonlySet<number>
  /** For a folder downloaded before that left files out: those files, marked Not Downloaded, the others Downloaded. */
  leftOut: readonly number[] | null
  /** Downloaded before: without `leftOut`, every file is marked Downloaded. */
  downloadedBefore: boolean
  disabled?: boolean
  onChange: (picked: Set<number>) => void
}

/**
 * A shared folder's files as a tree, each file and folder with a box to tick it, a folder's ticking everything under
 * it. Above them, a box for all of them with how many are ticked and their size, and a filter for a folder of many
 * files. For a folder downloaded before, each file says whether it was downloaded then. The list scrolls on its own and
 * draws only the rows on screen (`useVirtualRows`), so a folder of thousands of files ticks and scrolls smoothly.
 */
export function SharedFilesList({ files, picked, leftOut, downloadedBefore, disabled, onChange }: SharedFilesListProps) {
  const tree = useMemo(() => sharedTree(files), [files])
  const paths = useMemo(() => shownPaths(files), [files])
  const [folded, setFolded] = useState<ReadonlySet<string>>(
    () => new Set(files.length > FOLDED_ABOVE ? folderPaths(tree) : []),
  )
  const [filter, setFilter] = useState("")
  const deferredFilter = useDeferredValue(filter)
  const matching = useMemo(() => matchingFiles(paths, deferredFilter), [paths, deferredFilter])
  const rows = useMemo(() => visibleRows(tree, files, folded, matching), [tree, files, folded, matching])
  const leftOutSet = useMemo(() => (leftOut ? new Set(leftOut) : null), [leftOut])
  const scroller = useRef<HTMLUListElement>(null)
  // Every row is as tall as the estimate, so the rows are never measured and their keys are only their places.
  const rowKeys = useMemo(() => rows.map((_, place) => place), [rows])
  const drawn = useVirtualRows({
    keys: rowKeys,
    scroller,
    offset: 0,
    estimate: ROW_HEIGHT_PX,
    overscan: OVERSCAN_PX,
  })

  // The rows' handlers stay the same between renders, so ticking one file redraws only the rows it changes.
  const latest = useRef({ picked, onChange, matching })
  useLayoutEffect(() => {
    latest.current = { picked, onChange, matching }
  })
  const toggleFile = useCallback((index: number, on: boolean) => {
    const { picked, onChange } = latest.current
    onChange(withFiles(picked, [index], on))
  }, [])
  const toggleFolder = useCallback((folder: SharedFolder, on: boolean) => {
    const { picked, onChange, matching } = latest.current
    // While filtering, a folder ticks the files shown under it.
    const files = matching ? folder.allFiles.filter((index) => matching.has(index)) : folder.allFiles
    onChange(withFiles(picked, files, on))
  }, [])
  const fold = useCallback((path: string) => {
    setFolded((before) => {
      const next = new Set(before)
      if (next.has(path)) next.delete(path)
      else next.add(path)
      return next
    })
  }, [])

  // The box for all of them ticks the files shown while filtering, and every file otherwise.
  const scope = matching ? [...matching] : tree.allFiles
  const ticked = scope.filter((index) => picked.has(index)).length
  const allBox = useRef<HTMLInputElement>(null)
  const some = ticked > 0 && ticked < scope.length
  useEffect(() => {
    if (allBox.current) allBox.current.indeterminate = some
  }, [some])
  const allLabel = matching ? text.selectShown : text.selectAll

  function markOf(index: number): Mark | null {
    if (leftOutSet) return leftOutSet.has(index) ? "notDownloaded" : "downloaded"
    return downloadedBefore ? "downloaded" : null
  }

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center gap-3">
        <label className="flex min-w-0 items-center gap-2 text-fg-secondary">
          <input
            ref={allBox}
            type="checkbox"
            className="size-4 shrink-0 accent-accent"
            aria-label={allLabel}
            title={allLabel}
            checked={scope.length > 0 && ticked === scope.length}
            disabled={disabled || scope.length === 0}
            onChange={(event) => onChange(withFiles(picked, scope, event.target.checked))}
          />
          <span className="truncate tabular-nums">
            {text.count(picked.size, files.length, formatBytes(pickedBytes(files, picked)))}
          </span>
        </label>
        {files.length > FILTER_ABOVE && (
          <TextInput
            type="search"
            className="ml-auto w-56"
            placeholder={text.filter}
            aria-label={text.filter}
            value={filter}
            onChange={(event) => setFilter(event.target.value)}
          />
        )}
      </div>
      {rows.length === 0 ? (
        <p className="text-fg-muted">{text.noMatches}</p>
      ) : (
        <ul ref={scroller} className="-mx-2 max-h-[min(24rem,45vh)] overflow-y-auto">
          {drawn.before > 0 && <li aria-hidden style={{ height: drawn.before }} />}
          {rows.slice(drawn.first, drawn.last + 1).map((row) =>
            row.kind === "folder" ? (
              <FolderRow
                key={`folder:${row.folder.path}`}
                folder={row.folder}
                depth={row.depth}
                open={row.open}
                state={folderState(row.folder, picked, matching)}
                // While filtering, every folder is open and can't be folded.
                canFold={matching === null}
                disabled={disabled}
                onToggle={toggleFolder}
                onFold={fold}
              />
            ) : (
              <FileRow
                key={`file:${row.index}`}
                index={row.index}
                name={row.name}
                shownPath={paths[row.index]}
                path={files[row.index].path}
                size={files[row.index].size}
                depth={row.depth}
                picked={picked.has(row.index)}
                mark={markOf(row.index)}
                disabled={disabled}
                onToggle={toggleFile}
              />
            ),
          )}
          {drawn.after > 0 && <li aria-hidden style={{ height: drawn.after }} />}
        </ul>
      )}
    </div>
  )
}

interface FolderRowProps {
  folder: SharedFolder
  depth: number
  open: boolean
  state: "all" | "some" | "none"
  canFold: boolean
  disabled?: boolean
  onToggle: (folder: SharedFolder, on: boolean) => void
  onFold: (path: string) => void
}

/** A folder: a chevron that folds it, its box, which ticks everything under it, its name and what it holds. */
const FolderRow = memo(function FolderRow({
  folder,
  depth,
  open,
  state,
  canFold,
  disabled,
  onToggle,
  onFold,
}: FolderRowProps) {
  const box = useRef<HTMLInputElement>(null)
  useEffect(() => {
    if (box.current) box.current.indeterminate = state === "some"
  }, [state])
  const Chevron = open ? ChevronDown : ChevronRight
  const foldLabel = open ? text.collapse(folder.name) : text.expand(folder.name)
  return (
    <li>
      <div
        className="flex h-7 items-center gap-1 rounded-md pr-2 transition-colors hover:bg-input"
        style={{ paddingLeft: 8 + depth * INDENT_PX }}
      >
        <Tooltip label={foldLabel}>
          <button
            type="button"
            className="flex size-4 shrink-0 items-center justify-center rounded text-fg-secondary hover:text-fg disabled:opacity-40"
            aria-label={foldLabel}
            aria-expanded={open}
            disabled={!canFold}
            onClick={() => onFold(folder.path)}
          >
            <Chevron size={14} aria-hidden />
          </button>
        </Tooltip>
        <label
          className={classes("flex min-w-0 flex-1 items-center gap-2", disabled ? "cursor-default" : "cursor-pointer")}
        >
          <input
            ref={box}
            type="checkbox"
            className="size-4 shrink-0 accent-accent"
            // Its path from the list's top, since folders in different places may share a name.
            aria-label={folder.path}
            checked={state === "all"}
            disabled={disabled}
            onChange={(event) => onToggle(folder, event.target.checked)}
          />
          <Folder size={16} className="shrink-0 text-fg-muted" aria-hidden />
          <span className="min-w-0 flex-1 truncate font-medium" title={folder.path}>
            {folder.name}
          </span>
          <span className="shrink-0 text-xs text-fg-secondary tabular-nums">
            {text.folderContents(folder.allFiles.length, formatBytes(folder.bytes))}
          </span>
        </label>
      </div>
    </li>
  )
})

interface FileRowProps {
  index: number
  name: string
  /** Its path from the list's top: its box's name, since several folders may hold a file of the same name. */
  shownPath: string
  /** Its whole path in the share, for its hover text. */
  path: string
  size: number
  depth: number
  picked: boolean
  mark: Mark | null
  disabled?: boolean
  onToggle: (index: number, on: boolean) => void
}

/** A file: its box, its name (muted while unticked), whether it was downloaded before, and its size. */
const FileRow = memo(function FileRow({
  index,
  name,
  shownPath,
  path,
  size,
  depth,
  picked,
  mark,
  disabled,
  onToggle,
}: FileRowProps) {
  return (
    <li>
      <label
        className={classes(
          "flex h-7 items-center gap-2 rounded-md pr-2 transition-colors",
          disabled ? "cursor-default" : "cursor-pointer hover:bg-input",
        )}
        // The chevrons' column, then the box, so a folder's files sit one level in from its box.
        style={{ paddingLeft: 8 + depth * INDENT_PX + INDENT_PX + 4 }}
        title={path}
      >
        <input
          type="checkbox"
          className="size-4 shrink-0 accent-accent"
          aria-label={shownPath}
          checked={picked}
          disabled={disabled}
          onChange={(event) => onToggle(index, event.target.checked)}
        />
        <span className={classes("min-w-0 flex-1 truncate", !picked && "text-fg-muted")}>{name}</span>
        {mark && (
          <span
            className={classes(
              "flex shrink-0 items-center gap-1 text-xs",
              mark === "notDownloaded" ? "font-medium text-accent" : "text-fg-muted",
            )}
            title={mark === "notDownloaded" ? text.notDownloadedHint : text.downloadedHint}
          >
            {mark === "downloaded" && <Check size={12} className="text-success" aria-hidden />}
            {mark === "notDownloaded" ? text.notDownloaded : text.downloaded}
          </span>
        )}
        <span className="w-18 shrink-0 text-right text-xs text-fg-secondary tabular-nums">{formatBytes(size)}</span>
      </label>
    </li>
  )
})

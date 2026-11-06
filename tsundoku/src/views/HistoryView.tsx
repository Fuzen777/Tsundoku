import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react"
import { useQueryClient } from "@tanstack/react-query"
import {
  ArrowDownWideNarrow,
  ArrowUpNarrowWide,
  ChevronDown,
  ChevronRight,
  ExternalLink,
  FolderOpen,
  History,
  Play,
  Search,
} from "lucide-react"
import {
  confirmWithUser,
  forgetVideo,
  openDownload,
  openDownloadFile,
  openWebPage,
  queryKeys,
  showDownloadFileInFolder,
  showDownloadInFolder,
} from "../api"
import type { HistorySort } from "../bindings/HistorySort"
import type { HistoryVideo } from "../bindings/HistoryVideo"
import type { PastDownload } from "../bindings/PastDownload"
import { Card } from "../components/Card"
import { EmptyState } from "../components/EmptyState"
import { SelectInput, TextInput } from "../components/Field"
import { IconButton } from "../components/IconButton"
import { Menu } from "../components/Menu"
import { Page } from "../components/Page"
import { StatusMessage } from "../components/StatusMessage"
import { Thumbnail } from "../components/Thumbnail"
import { describeHistoryError, type ErrorDescription } from "../errors"
import { formatBytes, formatContents, formatDay, formatDuration, joinDetails } from "../format"
import { useAppActions } from "../navigation"
import { useDownloadFiles, useHistory, useHistoryFilters, useVideoDownloads } from "../queries"
import { strings } from "../strings"
import { useDebouncedValue } from "../useDebouncedValue"
import { useVirtualRows } from "../virtualRows"

const text = strings.history

const SORTS: readonly HistorySort[] = ["downloaded", "title", "uploader", "site", "duration", "size", "preset"]

/** A row's height before it's measured: a card with a 112 px thumbnail, and the 12 px under it (`pb-3`). */
const ROW_ESTIMATE = 102
/** How far past each edge of the screen rows are drawn, in pixels. */
const OVERSCAN = 600
/** How close to the last row loaded the list reads the next page. */
const LOAD_AHEAD = 30

/**
 * Every video downloaded, newest first, with search, filters and sorting. The history is read a page at a time as the
 * list scrolls, and only the rows on screen are drawn, so a history of thousands stays quick. Which rows are expanded is
 * kept here, so a row scrolled away and back keeps its lists open.
 */
export function HistoryView() {
  const [search, setSearch] = useState("")
  const [site, setSite] = useState<string | null>(null)
  const [preset, setPreset] = useState<string | null>(null)
  const [sort, setSort] = useState<HistorySort>("downloaded")
  const [descending, setDescending] = useState(true)
  // The search runs once typing pauses rather than on every key.
  const settledSearch = useDebouncedValue(search.trim(), 250)
  const query = { search: settledSearch || null, site, preset, sort, descending }
  const { data, hasNextPage, isFetchingNextPage, fetchNextPage } = useHistory(query)
  const { data: filters } = useHistoryFilters()
  const isFiltered = query.search !== null || site !== null || preset !== null
  const videos = useMemo(() => uniqueVideos(data?.pages.flatMap((page) => page.videos) ?? []), [data])
  const videoIds = useMemo(() => videos.map((video) => video.id), [videos])
  const total = data?.pages[0]?.total
  const [openFiles, toggleFiles] = useToggledIds()
  const [openDownloads, toggleDownloads] = useToggledIds()

  const scroller = useRef<HTMLDivElement>(null)
  const head = useRef<HTMLDivElement>(null)
  const list = useRef<HTMLDivElement>(null)
  // Where the list starts in the scrolled content, under the toolbar, which wraps at narrow widths: read whenever the
  // toolbar's size changes, which also happens once as soon as it's observed.
  const [listTop, setListTop] = useState(0)
  useEffect(() => {
    if (!head.current) return
    const observer = new ResizeObserver(() => {
      if (!scroller.current || !list.current) return
      const top =
        list.current.getBoundingClientRect().top -
        scroller.current.getBoundingClientRect().top +
        scroller.current.scrollTop
      setListTop(Math.round(top))
    })
    observer.observe(head.current)
    return () => observer.disconnect()
  }, [])

  const rows = useVirtualRows({
    keys: videoIds,
    scroller,
    offset: listTop,
    estimate: ROW_ESTIMATE,
    overscan: OVERSCAN,
  })
  useEffect(() => {
    if (hasNextPage && !isFetchingNextPage && rows.last >= videos.length - LOAD_AHEAD) void fetchNextPage()
  }, [hasNextPage, isFetchingNextPage, rows.last, videos.length, fetchNextPage])

  // A new search, filter or sort starts from the top of its results.
  const queryKey = JSON.stringify(query)
  useEffect(() => {
    scroller.current?.scrollTo({ top: 0 })
  }, [queryKey])

  return (
    <Page title={strings.nav.history} scrollRef={scroller}>
      <div className="flex flex-col gap-3">
        <div ref={head} className="flex flex-col gap-3">
          <div className="flex flex-wrap items-center gap-2">
            <div className="relative min-w-48 flex-1">
              <Search
                size={14}
                className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-fg-muted"
              />
              <TextInput
                type="search"
                aria-label={text.searchPlaceholder}
                placeholder={text.searchPlaceholder}
                value={search}
                onChange={(event) => setSearch(event.target.value)}
                className="w-full pl-8"
              />
            </div>
            <SelectInput
              aria-label={text.allSites}
              value={site ?? ""}
              onChange={(event) => setSite(event.target.value || null)}
            >
              <option value="">{text.allSites}</option>
              {filters?.sites.map((option) => (
                <option key={option.site} value={option.site}>
                  {option.siteName}
                </option>
              ))}
            </SelectInput>
            <SelectInput
              aria-label={text.allPresets}
              value={preset ?? ""}
              onChange={(event) => setPreset(event.target.value || null)}
            >
              <option value="">{text.allPresets}</option>
              {filters?.presets.map((name) => (
                <option key={name} value={name}>
                  {name}
                </option>
              ))}
            </SelectInput>
            <SelectInput
              aria-label={text.sortBy}
              title={text.sortBy}
              value={sort}
              onChange={(event) => setSort(event.target.value as HistorySort)}
            >
              {SORTS.map((option) => (
                <option key={option} value={option}>
                  {text.sorts[option]}
                </option>
              ))}
            </SelectInput>
            <IconButton
              label={descending ? text.descending : text.ascending}
              icon={descending ? ArrowDownWideNarrow : ArrowUpNarrowWide}
              variant="secondary"
              onClick={() => setDescending((isDescending) => !isDescending)}
            />
          </div>

          {total !== undefined && total > 0 && <p className="text-xs text-fg-muted">{text.videoCount(total)}</p>}
          {total === 0 &&
            (isFiltered ? (
              <EmptyState icon={Search} title={text.noMatchesTitle} body={text.noMatchesBody} />
            ) : (
              <EmptyState icon={History} title={text.emptyTitle} body={text.emptyBody} />
            ))}
        </div>

        <div ref={list}>
          {rows.before > 0 && <div style={{ height: rows.before }} />}
          {videos.slice(rows.first, rows.last + 1).map((video) => (
            <div key={video.id} data-row-key={video.id} ref={rows.measure} className="pb-3">
              <HistoryRow
                video={video}
                filesOpen={openFiles.has(video.id)}
                onToggleFiles={toggleFiles}
                downloadsOpen={openDownloads.has(video.id)}
                onToggleDownloads={toggleDownloads}
              />
            </div>
          ))}
          {rows.after > 0 && <div style={{ height: rows.after }} />}
        </div>
      </div>
    </Page>
  )
}

/** The videos of every page read, each once: a video can move from one page to the next while they're read again. */
function uniqueVideos(videos: readonly HistoryVideo[]): HistoryVideo[] {
  const seen = new Set<number>()
  return videos.filter((video) => {
    if (seen.has(video.id)) return false
    seen.add(video.id)
    return true
  })
}

/** A set of IDs, and a function that adds an ID or takes it out. */
function useToggledIds(): [ReadonlySet<number>, (id: number) => void] {
  const [ids, setIds] = useState<ReadonlySet<number>>(new Set())
  const toggle = useCallback((id: number) => {
    setIds((current) => {
      const next = new Set(current)
      if (!next.delete(id)) next.add(id)
      return next
    })
  }, [])
  return [ids, toggle]
}

interface HistoryRowProps {
  video: HistoryVideo
  /** Whether the list of a post's files is open. */
  filesOpen: boolean
  onToggleFiles: (videoId: number) => void
  /** Whether the list of the video's downloads is open. */
  downloadsOpen: boolean
  onToggleDownloads: (videoId: number) => void
}

/** A video in the history. Drawn again only when its own props change, not each time the list scrolls. */
const HistoryRow = memo(function HistoryRow({
  video,
  filesOpen,
  onToggleFiles,
  downloadsOpen,
  onToggleDownloads,
}: HistoryRowProps) {
  const { showLink } = useAppActions()
  const queryClient = useQueryClient()
  const [error, setError] = useState<ErrorDescription | null>(null)
  const { lastDownload } = video

  const run = (action: () => Promise<void>) => {
    setError(null)
    action().catch((caught: unknown) => setError(describeHistoryError(caught)))
  }
  const forget = () =>
    run(async () => {
      const confirmed = await confirmWithUser({
        title: text.forgetTitle,
        message: text.forgetMessage(video.title),
        confirmLabel: text.forget,
        cancelLabel: strings.dialogs.cancel,
      })
      if (!confirmed) return
      await forgetVideo(video.id)
      await queryClient.invalidateQueries({ queryKey: queryKeys.history })
      await queryClient.invalidateQueries({ queryKey: queryKeys.historyFilters })
    })

  return (
    <Card className="flex flex-col gap-2 p-3">
      <div className="flex items-start gap-3">
        <Thumbnail key={video.thumbnailUrl} url={video.thumbnailUrl} className="w-28" />
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <p className="truncate font-medium select-text" title={video.title}>
            {video.title}
          </p>
          <p className="truncate text-xs text-fg-secondary">
            {joinDetails([
              video.uploader,
              video.siteName,
              video.durationSeconds !== null && formatDuration(video.durationSeconds),
              formatContents(video),
            ])}
          </p>
          <p className="flex flex-wrap items-center gap-x-2 text-xs text-fg-secondary tabular-nums">
            <span>
              {joinDetails([
                lastDownload.preset,
                formatDay(lastDownload.downloadedAt),
                lastDownload.fileSize !== null && formatBytes(lastDownload.fileSize),
              ])}
            </span>
            {!lastDownload.fileExists && (
              <span className="rounded-full border border-warning/50 px-1.5 text-warning">{text.fileMissing}</span>
            )}
          </p>
          <div className="flex flex-wrap gap-x-4">
            {lastDownload.fileCount > 1 && (
              <ExpandToggle
                expanded={filesOpen}
                onToggle={() => onToggleFiles(video.id)}
                label={text.fileCount(lastDownload.fileCount)}
              />
            )}
            {video.downloadCount > 1 && (
              <ExpandToggle
                expanded={downloadsOpen}
                onToggle={() => onToggleDownloads(video.id)}
                label={text.downloadCount(video.downloadCount)}
              />
            )}
          </div>
        </div>
        <div className="flex shrink-0">
          <FileButtons download={lastDownload} onError={setError} />
          <IconButton
            label={text.openSourcePage}
            icon={ExternalLink}
            onClick={() => run(() => openWebPage(video.url))}
          />
        </div>
        <Menu
          label={text.moreActions}
          items={[
            { label: text.downloadAgain, onSelect: () => showLink(video.url) },
            { label: text.forget, onSelect: forget, danger: true },
          ]}
        />
      </div>
      {filesOpen && <FileList downloadId={lastDownload.id} onError={setError} />}
      {downloadsOpen && <DownloadList videoId={video.id} onError={setError} />}
      {error && <StatusMessage tone="danger" {...error} />}
    </Card>
  )
})

/** The link that shows or hides a list under a row. */
function ExpandToggle({ expanded, onToggle, label }: { expanded: boolean; onToggle: () => void; label: string }) {
  return (
    <button
      type="button"
      aria-expanded={expanded}
      onClick={onToggle}
      className="flex w-fit items-center gap-1 rounded text-xs font-medium text-accent hover:text-accent-hover focus-visible:outline-2 focus-visible:outline-accent"
    >
      {expanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
      {label}
    </button>
  )
}

/** Every file of a post's download, each with Open File and Show in Folder while it's still there. */
function FileList({ downloadId, onError }: { downloadId: number; onError: (error: ErrorDescription) => void }) {
  const { data: files } = useDownloadFiles(downloadId, true)
  const run = (action: (downloadId: number, position: number) => Promise<void>, position: number) =>
    action(downloadId, position).catch((error: unknown) => onError(describeHistoryError(error)))
  return (
    <div className="ml-31 flex flex-col divide-y divide-line border-t border-line">
      {files?.map((file) => (
        <div key={file.position} className="flex items-center gap-3 py-1.5">
          <div className="min-w-0 flex-1">
            <p className="truncate text-xs text-fg-secondary select-text" title={file.path}>
              {fileName(file.path)}
              {file.size !== null && <span className="ml-2 text-fg-muted tabular-nums">{formatBytes(file.size)}</span>}
              {!file.exists && <span className="ml-2 text-warning">{text.fileMissing}</span>}
            </p>
          </div>
          <div className="flex shrink-0">
            <IconButton
              label={text.openFile}
              icon={Play}
              disabled={!file.exists}
              onClick={() => void run(openDownloadFile, file.position)}
            />
            <IconButton
              label={text.showInFolder}
              icon={FolderOpen}
              disabled={!file.exists}
              onClick={() => void run(showDownloadFileInFolder, file.position)}
            />
          </div>
        </div>
      ))}
    </div>
  )
}

/** The last part of a Windows or other path. */
function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path
}

/** Every download of a video, for a row that's expanded. */
function DownloadList({ videoId, onError }: { videoId: number; onError: (error: ErrorDescription) => void }) {
  const { data: downloads } = useVideoDownloads(videoId, true)
  return (
    <div className="ml-31 flex flex-col divide-y divide-line border-t border-line">
      {downloads?.map((download) => (
        <div key={download.id} className="flex items-center gap-3 py-1.5">
          <div className="min-w-0 flex-1">
            <p className="text-xs text-fg-secondary tabular-nums">
              {joinDetails([
                download.preset,
                formatDay(download.downloadedAt),
                download.format,
                download.fileCount > 1 && text.fileCount(download.fileCount),
                download.fileSize !== null && formatBytes(download.fileSize),
              ])}
              {!download.fileExists && <span className="ml-2 text-warning">{text.fileMissing}</span>}
            </p>
            <p className="truncate text-xs text-fg-muted select-text" title={download.filePath}>
              {download.filePath}
            </p>
          </div>
          <FileButtons download={download} onError={onError} />
        </div>
      ))}
    </div>
  )
}

/** Open File and Show in Folder, as icon buttons, for a download whose file is still there. */
function FileButtons({ download, onError }: { download: PastDownload; onError: (error: ErrorDescription) => void }) {
  const run = (action: (downloadId: number) => Promise<void>) =>
    action(download.id).catch((error: unknown) => onError(describeHistoryError(error)))
  return (
    <div className="flex shrink-0">
      <IconButton
        label={text.openFile}
        icon={Play}
        disabled={!download.fileExists}
        onClick={() => void run(openDownload)}
      />
      <IconButton
        label={text.showInFolder}
        icon={FolderOpen}
        disabled={!download.fileExists}
        onClick={() => void run(showDownloadInFolder)}
      />
    </div>
  )
}

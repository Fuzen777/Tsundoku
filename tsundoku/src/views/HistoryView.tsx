import { useDeferredValue, useState } from "react"
import { useQueryClient } from "@tanstack/react-query"
import { ArrowDownWideNarrow, ArrowUpNarrowWide, ChevronDown, ChevronRight, FolderOpen, History, Play, Search } from "lucide-react"
import { confirmWithUser, forgetVideo, openDownload, openWebPage, queryKeys, showDownloadInFolder } from "../api"
import type { HistorySort } from "../bindings/HistorySort"
import type { HistoryVideo } from "../bindings/HistoryVideo"
import type { PastDownload } from "../bindings/PastDownload"
import { Button } from "../components/Button"
import { Card } from "../components/Card"
import { EmptyState } from "../components/EmptyState"
import { SelectInput, TextInput } from "../components/Field"
import { Menu } from "../components/Menu"
import { Page } from "../components/Page"
import { StatusMessage } from "../components/StatusMessage"
import { Thumbnail } from "../components/Thumbnail"
import { describeHistoryError, type ErrorDescription } from "../errors"
import { formatBytes, formatDay, formatDuration, joinDetails } from "../format"
import { useAppActions } from "../navigation"
import { useHistory, useHistoryFilters, useVideoDownloads } from "../queries"
import { strings } from "../strings"

const text = strings.history

const SORTS: readonly HistorySort[] = ["downloaded", "title", "uploader", "site", "duration", "size", "preset"]

/** Every video downloaded, newest first, with search, filters and sorting. */
export function HistoryView() {
  const [search, setSearch] = useState("")
  const [site, setSite] = useState<string | null>(null)
  const [preset, setPreset] = useState<string | null>(null)
  const [sort, setSort] = useState<HistorySort>("downloaded")
  const [descending, setDescending] = useState(true)
  // The search runs as typing pauses rather than on every key.
  const deferredSearch = useDeferredValue(search)
  const query = { search: deferredSearch.trim() || null, site, preset, sort, descending }
  const { data: videos } = useHistory(query)
  const { data: filters } = useHistoryFilters()
  const isFiltered = query.search !== null || site !== null || preset !== null

  return (
    <Page title={strings.nav.history}>
      <div className="flex flex-col gap-3">
        <div className="flex flex-wrap items-center gap-2">
          <div className="relative min-w-48 flex-1">
            <Search size={14} className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-fg-muted" />
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
          <Button
            size="icon"
            aria-label={descending ? text.descending : text.ascending}
            title={descending ? text.descending : text.ascending}
            onClick={() => setDescending((isDescending) => !isDescending)}
          >
            {descending ? <ArrowDownWideNarrow size={16} /> : <ArrowUpNarrowWide size={16} />}
          </Button>
        </div>

        {videos && videos.length > 0 && (
          <p className="text-xs text-fg-muted">{text.videoCount(videos.length)}</p>
        )}
        {videos?.length === 0 &&
          (isFiltered ? (
            <EmptyState icon={Search} title={text.noMatchesTitle} body={text.noMatchesBody} />
          ) : (
            <EmptyState icon={History} title={text.emptyTitle} body={text.emptyBody} />
          ))}
        {videos?.map((video) => (
          <HistoryRow key={video.id} video={video} />
        ))}
      </div>
    </Page>
  )
}

function HistoryRow({ video }: { video: HistoryVideo }) {
  const { showLink } = useAppActions()
  const queryClient = useQueryClient()
  const [expanded, setExpanded] = useState(false)
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
          {video.downloadCount > 1 && (
            <button
              type="button"
              aria-expanded={expanded}
              onClick={() => setExpanded((isExpanded) => !isExpanded)}
              className="flex w-fit items-center gap-1 rounded text-xs font-medium text-accent hover:text-accent-hover focus-visible:outline-2 focus-visible:outline-accent"
            >
              {expanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
              {text.downloadCount(video.downloadCount)}
            </button>
          )}
        </div>
        <FileButtons download={lastDownload} onError={setError} />
        <Menu
          label={text.moreActions}
          items={[
            { label: text.openSourcePage, onSelect: () => run(() => openWebPage(video.url)) },
            { label: text.downloadAgain, onSelect: () => showLink(video.url) },
            { label: text.forget, onSelect: forget, danger: true },
          ]}
        />
      </div>
      {expanded && <DownloadList videoId={video.id} onError={setError} />}
      {error && <StatusMessage tone="danger" {...error} />}
    </Card>
  )
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
      <Button
        size="icon"
        variant="ghost"
        aria-label={text.openFile}
        title={text.openFile}
        disabled={!download.fileExists}
        onClick={() => void run(openDownload)}
      >
        <Play size={16} />
      </Button>
      <Button
        size="icon"
        variant="ghost"
        aria-label={text.showInFolder}
        title={text.showInFolder}
        disabled={!download.fileExists}
        onClick={() => void run(showDownloadInFolder)}
      >
        <FolderOpen size={16} />
      </Button>
    </div>
  )
}

import { useState, type ReactNode } from "react"
import { ArrowDownWideNarrow, ArrowUpNarrowWide, ChevronDown, ChevronRight, Eye, History, Pencil, RefreshCw } from "lucide-react"
import {
  checkWatchedSource,
  checkWatchedSources,
  confirmWithUser,
  downloadNewWatchedItems,
  downloadWatchedItems,
  editWatchedSource,
  keptThumbnailUrl,
  openDownload,
  pickFolder,
  removeWatchedSource,
  skipEveryNewWatchedItem,
  skipWatchedItems,
} from "../api"
import type { HistoryVideo } from "../bindings/HistoryVideo"
import type { ItemKey } from "../bindings/ItemKey"
import type { WatchedGroup } from "../bindings/WatchedGroup"
import type { WatchedItem } from "../bindings/WatchedItem"
import type { WatchedShow } from "../bindings/WatchedShow"
import type { WatchedSort } from "../bindings/WatchedSort"
import type { WatchedSource } from "../bindings/WatchedSource"
import { GroupedList } from "../components/ArrangedList"
import { Button } from "../components/Button"
import { Card } from "../components/Card"
import { EmptyState } from "../components/EmptyState"
import { FailureFixes } from "../components/FailureFixes"
import { Field, SelectInput, TextInput } from "../components/Field"
import { IconButton } from "../components/IconButton"
import { ExpandToggle } from "../components/LinkButton"
import { ListedItems, ListingPicture } from "../components/ListingPicker"
import { Menu } from "../components/Menu"
import { Page } from "../components/Page"
import { PickerViewControls } from "../components/PickerViewControls"
import { SegmentedControl } from "../components/SegmentedControl"
import { StatusMessage } from "../components/StatusMessage"
import { classes } from "../components/styles"
import { Thumbnail } from "../components/Thumbnail"
import {
  describeHistoryError,
  describePreviewError,
  describeWatchedError,
  fixFor,
  type ErrorDescription,
} from "../errors"
import { useFoldedGroups } from "../foldedGroups"
import { formatDay, formatRoughAge, formatTimeAgo, joinDetails } from "../format"
import { isQueued, itemKey } from "../listingPicker"
import { useAppActions } from "../navigation"
import {
  useLatestWatchedDownloads,
  usePickerLayout,
  usePresets,
  useWatchedArrangement,
  useWatchedSources,
} from "../queries"
import { strings } from "../strings"
import {
  allNewItems,
  arrangeSources,
  isExpanded,
  newCount,
  stripShape,
  WATCHED_SORTS,
  type SourcedItem,
} from "../watchedSources"

const text = strings.watched

const SHOWS: readonly WatchedShow[] = ["sources", "allNew"]
const GROUP_OPTIONS = (["none", "site"] as const satisfies readonly WatchedGroup[]).map((group) => ({
  value: group,
  label: text.groups[group],
}))

/** The sources opened or closed by hand, kept while the app runs so leaving the view doesn't change them. */
const remembered: { expansion: ReadonlyMap<number, boolean> } = { expansion: new Map() }

/**
 * The watched channels, playlists and accounts: a card each, grouped by site and sorted as the toolbar says, the ones
 * whose last check failed first under Needs Attention. A card folds to its summary, and starts open while it has new
 * items. All New Items shows every source's new items in one picker instead.
 */
export function WatchedView() {
  const { data: sources = [] } = useWatchedSources()
  const { arrangement, ready, change, error: arrangeError } = useWatchedArrangement()
  const [expansion, setExpansion] = useState(remembered.expansion)
  const folds = useFoldedGroups("watched")
  const [error, setError] = useState<ErrorDescription | null>(null)
  const { groups, headings } = arrangeSources(sources, arrangement)
  const shown = groups.flatMap((group) => group.items)
  // An item two sources found counts once, as All New Items shows it.
  const waiting = allNewItems(sources).length
  const showingAll = arrangement.show === "allNew"

  const choose = (update: (current: ReadonlyMap<number, boolean>) => ReadonlyMap<number, boolean>) =>
    setExpansion((current) => {
      const next = update(current)
      remembered.expansion = next
      return next
    })
  const toggle = (source: WatchedSource) =>
    choose((current) => new Map(current).set(source.id, !isExpanded(source, current)))
  // Expand All also unfolds the folded headings, so every card it opens shows; Collapse All folds the cards only.
  const anyFolded =
    shown.some((source) => !isExpanded(source, expansion)) ||
    (headings && groups.some((group) => folds.isFolded(group.key)))
  const setAll = (open: boolean) => {
    choose(() => new Map(shown.map((source) => [source.id, open])))
    if (open) folds.unfoldAll()
  }

  const check = (ids: number[]) => {
    setError(null)
    checkWatchedSources(ids).catch((caught: unknown) => setError(describeWatchedError(caught)))
  }
  const allChecking = sources.every((source) => source.checking)

  const actions = sources.length > 0 && (
    <Button size="sm" variant="secondary" disabled={allChecking} title={text.checkAllHint} onClick={() => check(shown.map((source) => source.id))}>
      {text.checkAll}
    </Button>
  )

  return (
    <Page title={strings.nav.watched} actions={actions}>
      {sources.length === 0 ? (
        <EmptyState icon={Eye} title={text.emptyTitle} body={text.emptyBody} />
      ) : (
        <div className="flex max-w-4xl flex-col gap-3">
          <div className="flex flex-wrap items-center gap-2">
            <SegmentedControl
              label={text.show}
              options={SHOWS.map((show) => ({
                value: show,
                label: show === "allNew" ? text.shows.allNew(waiting) : text.shows.sources,
              }))}
              value={arrangement.show}
              disabled={!ready}
              onChange={(show) => void change({ show })}
            />
            {!showingAll && (
              <>
                <span className="ml-2 text-xs text-fg-secondary">{text.groupBy}</span>
                <SegmentedControl
                  label={text.groupBy}
                  options={GROUP_OPTIONS}
                  value={arrangement.group}
                  disabled={!ready}
                  onChange={(group) => void change({ group })}
                />
                <SelectInput
                  aria-label={text.sortBy}
                  title={text.sortBy}
                  value={arrangement.sort}
                  disabled={!ready}
                  onChange={(event) => void change({ sort: event.target.value as WatchedSort })}
                >
                  {WATCHED_SORTS.map((sort) => (
                    <option key={sort} value={sort}>
                      {text.sorts[sort]}
                    </option>
                  ))}
                </SelectInput>
                <IconButton
                  label={arrangement.descending ? text.descending : text.ascending}
                  icon={arrangement.descending ? ArrowDownWideNarrow : ArrowUpNarrowWide}
                  variant="secondary"
                  disabled={!ready}
                  onClick={() => void change({ descending: !arrangement.descending })}
                />
                <Button size="sm" variant="ghost" className="ml-auto" onClick={() => setAll(anyFolded)}>
                  {anyFolded ? text.expandAll : text.collapseAll}
                </Button>
              </>
            )}
          </div>
          {arrangeError && <StatusMessage tone="danger" {...arrangeError} />}
          {error && <StatusMessage tone="danger" {...error} />}
          {showingAll ? (
            <AllNewItems sources={sources} />
          ) : (
            <GroupedList
              view="watched"
              groups={groups}
              grouped={headings}
              folds={folds}
              keyOf={(source) => source.id}
              headingExtra={(group) => (
                <GroupHeadingExtra
                  label={group.label}
                  sources={group.items}
                  onCheck={() => check(group.items.map((source) => source.id))}
                />
              )}
              renderItem={(source) => (
                <SourceCard source={source} expanded={isExpanded(source, expansion)} onToggle={() => toggle(source)} />
              )}
            />
          )}
        </div>
      )}
    </Page>
  )
}

/** After a group's name and count: how many new items wait in it, and Check All Under it. */
function GroupHeadingExtra({
  label,
  sources,
  onCheck,
}: {
  label: string
  sources: readonly WatchedSource[]
  onCheck: () => void
}) {
  const waiting = newCount(sources)
  return (
    <>
      {waiting > 0 && <span className="shrink-0 text-xs font-medium text-accent tabular-nums">{text.groupNew(waiting)}</span>}
      <span className="ml-auto">
        <IconButton
          label={text.checkGroup(label)}
          icon={RefreshCw}
          disabled={sources.every((source) => source.checking)}
          onClick={onCheck}
        />
      </span>
    </>
  )
}

interface SourceCardProps {
  source: WatchedSource
  expanded: boolean
  onToggle: () => void
}

/**
 * A source: its chevron, picture, name, kind and site, and what checks found, with its actions on the right. Open, it
 * shows where it saves, its new items and what was skipped before. A failed check's message and the editor show either
 * way.
 */
function SourceCard({ source, expanded, onToggle }: SourceCardProps) {
  const { data: presets } = usePresets()
  const { showWatchedDownloads } = useAppActions()
  const [editing, setEditing] = useState(false)
  const [error, setError] = useState<ErrorDescription | null>(null)
  const presetName = presets?.presets.find((preset) => preset.id === source.presetId)?.name
  const Chevron = expanded ? ChevronDown : ChevronRight
  const hasItems = source.newItems.length > 0 || source.skippedItems.length > 0

  const act = (action: () => Promise<unknown>) => {
    setError(null)
    action().catch((caught: unknown) => setError(describeWatchedError(caught)))
  }

  async function remove() {
    const confirmed = await confirmWithUser({
      title: text.removeTitle,
      message: text.removeMessage(source.name),
      confirmLabel: text.removeConfirm,
      cancelLabel: strings.dialogs.cancel,
    })
    if (confirmed) await removeWatchedSource(source.id)
  }

  const check = () => act(() => checkWatchedSource(source.id))
  return (
    <Card className="flex flex-col gap-3 p-4">
      <div className="flex items-start gap-3">
        <button
          type="button"
          aria-expanded={expanded}
          aria-label={expanded ? text.collapse(source.name) : text.expand(source.name)}
          onClick={onToggle}
          className="flex min-w-0 flex-1 items-start gap-3 rounded text-left focus-visible:outline-2 focus-visible:outline-accent"
        >
          <Chevron size={16} className="mt-5 shrink-0 text-fg-secondary" aria-hidden />
          <ListingPicture listing={source} small />
          <span className="flex min-w-0 flex-1 flex-col gap-0.5">
            <span className="truncate font-semibold" title={source.name}>
              {source.name}
            </span>
            <span className="truncate text-xs text-fg-secondary">
              {joinDetails([strings.picker.kinds[source.kind], source.siteName])}
            </span>
            <SourceSummary source={source} />
            {expanded && (
              <span className="truncate text-xs text-fg-muted" title={source.folder ?? undefined}>
                {joinDetails([
                  source.folder ? text.savesIn(source.folder) : text.savesByRules,
                  presetName && text.startsAs(presetName),
                ])}
              </span>
            )}
          </span>
        </button>
        <div className="flex shrink-0 items-center gap-1">
          <IconButton
            label={source.checking ? text.checking : text.checkNow}
            icon={RefreshCw}
            variant="secondary"
            disabled={source.checking}
            onClick={check}
          />
          <IconButton
            label={text.showInHistory}
            icon={History}
            disabled={source.downloadedCount === 0}
            onClick={() => showWatchedDownloads(source.id)}
          />
          <IconButton label={text.edit} icon={Pencil} onClick={() => setEditing((shown) => !shown)} />
          <Menu label={text.moreActions} items={[{ label: text.remove, danger: true, onSelect: () => act(remove) }]} />
        </div>
      </div>
      {!expanded && source.downloadedCount > 0 && (
        // Under the name, past the chevron and the picture (a playlist's is wider).
        <LatestDownloads sourceId={source.id} className={source.kind === "playlist" ? "pl-34" : "pl-24"} />
      )}
      {source.lastError && !source.checking && (
        <StatusMessage tone="danger" {...describePreviewError(source.lastError)}>
          {fixFor(source.lastError) && <FailureFixes error={source.lastError} onRetry={check} />}
        </StatusMessage>
      )}
      {error && <StatusMessage tone="danger" {...error} />}
      {editing && <SourceEditor source={source} onDone={() => setEditing(false)} />}
      {expanded && hasItems && (
        <div className="flex flex-col gap-3 border-t border-line pt-3">
          {source.newItems.length > 0 && <NewItems source={source} />}
          {source.skippedItems.length > 0 && <SkippedItems source={source} />}
        </div>
      )}
    </Card>
  )
}

/**
 * A folded card's strip of the source's latest downloads, the latest first: as many pictures as fit on one row (the
 * row's height hides the ones that wrap), each opening its file. Its room is kept while they're read, so the card
 * doesn't grow under the pointer.
 */
function LatestDownloads({ sourceId, className }: { sourceId: number; className: string }) {
  const { data: videos = [] } = useLatestWatchedDownloads(sourceId)
  const [error, setError] = useState<ErrorDescription | null>(null)
  const open = (downloadId: number) => {
    setError(null)
    openDownload(downloadId).catch((caught: unknown) => setError(describeHistoryError(caught)))
  }
  return (
    <>
      <ul
        aria-label={text.latestDownloads}
        className={classes("flex h-12 flex-wrap gap-x-1.5 gap-y-4 overflow-hidden", className)}
      >
        {videos.map((video) => (
          <LatestDownload key={video.id} video={video} onOpen={open} />
        ))}
      </ul>
      {error && <StatusMessage tone="danger" {...error} />}
    </>
  )
}

/** A picture in the strip: the thumbnail kept in the data folder, else the site's; dimmed when its file is gone. */
function LatestDownload({ video, onOpen }: { video: HistoryVideo; onOpen: (downloadId: number) => void }) {
  const { lastDownload } = video
  const kept = video.thumbnailFile && keptThumbnailUrl(video.thumbnailFile)
  return (
    // The hover text sits on the item, since a disabled button shows none.
    <li
      className="shrink-0"
      title={[
        video.title,
        joinDetails([
          text.downloadedOn(formatDay(lastDownload.downloadedAt)),
          !lastDownload.fileExists && strings.history.fileMissing,
        ]),
      ].join("\n")}
    >
      <button
        type="button"
        aria-label={text.openDownload(video.title)}
        disabled={!lastDownload.fileExists}
        onClick={() => onOpen(lastDownload.id)}
        className="block rounded-md transition-opacity hover:opacity-80 focus-visible:outline-2 focus-visible:outline-accent disabled:opacity-40"
      >
        <Thumbnail
          key={`${video.thumbnailFile}|${video.thumbnailUrl}`}
          url={kept ?? video.thumbnailUrl}
          fallbackUrl={kept && video.thumbnailUrl}
          className={stripShape(video) === "square" ? "size-12" : "h-12 w-21"}
        />
      </button>
    </li>
  )
}

/** "Checked 2 hours ago · 3 new · 12 downloaded · 2 skipped · Last post 3 weeks ago", the new items in the accent. */
function SourceSummary({ source }: { source: WatchedSource }) {
  const waiting = source.newItems.length
  const counts = joinDetails([
    source.downloadedCount > 0 && text.downloadedCount(source.downloadedCount),
    source.skippedItems.length > 0 && text.skippedCount(source.skippedItems.length),
    source.latestPost && text.latestPost(formatRoughAge(source.latestPost)),
  ])
  return (
    <span className="text-xs text-fg-secondary tabular-nums">
      {source.checking ? (
        text.checking
      ) : (
        <>
          {source.lastCheckedAt === null ? text.neverChecked : text.checked(formatTimeAgo(source.lastCheckedAt))}
          {" · "}
          <span className={waiting > 0 ? "font-medium text-accent" : undefined}>{text.newCount(waiting)}</span>
        </>
      )}
      {counts && ` · ${counts}`}
    </span>
  )
}

/** Edit: Save In and Videos Start As, saved together. */
function SourceEditor({ source, onDone }: { source: WatchedSource; onDone: () => void }) {
  const { data: presets } = usePresets()
  const [folder, setFolder] = useState(source.folder ?? "")
  const [presetId, setPresetId] = useState(source.presetId ?? "")
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<ErrorDescription | null>(null)

  async function chooseFolder() {
    const chosen = await pickFolder(text.chooseFolderTitle)
    if (chosen) setFolder(chosen)
  }

  async function save() {
    setSaving(true)
    setError(null)
    try {
      await editWatchedSource(source.id, folder.trim() || null, presetId || null)
      onDone()
    } catch (caught) {
      setError(describeWatchedError(caught))
      setSaving(false)
    }
  }

  return (
    <div className="flex flex-col gap-3 border-t border-line pt-3">
      <Field label={text.saveIn} hint={text.saveInHint} htmlFor={`watched-folder-${source.id}`}>
        <div className="flex gap-2">
          <TextInput
            id={`watched-folder-${source.id}`}
            className="flex-1 font-mono"
            placeholder={text.rulesDecide}
            value={folder}
            disabled={saving}
            onChange={(event) => setFolder(event.target.value)}
          />
          <Button disabled={saving} onClick={() => void chooseFolder()}>
            {text.chooseFolder}
          </Button>
        </div>
      </Field>
      {source.kind !== "account" && (
        <Field label={text.videosStartAs} htmlFor={`watched-preset-${source.id}`}>
          <SelectInput
            id={`watched-preset-${source.id}`}
            value={presetId}
            disabled={saving || !presets}
            onChange={(event) => setPresetId(event.target.value)}
          >
            <option value="">{text.rulesPreset}</option>
            {presets?.presets.map((preset) => (
              <option key={preset.id} value={preset.id}>
                {preset.name}
              </option>
            ))}
          </SelectInput>
        </Field>
      )}
      {error && <StatusMessage tone="danger" {...error} />}
      <div className="flex justify-end gap-2">
        <Button variant="ghost" disabled={saving} onClick={onDone}>
          {text.discard}
        </Button>
        <Button variant="primary" loading={saving} onClick={() => void save()}>
          {text.save}
        </Button>
      </div>
    </div>
  )
}

/**
 * What checks found: each item ticked unless the history or the queue has it. Download Selected queues the ticked ones
 * and skips the rest.
 */
function NewItems({ source }: { source: WatchedSource }) {
  return (
    <ItemPicker
      items={source.newItems}
      startsTicked={(item) => !item.lastDownload}
      showUploader={source.kind === "playlist"}
      actions={(picked, run, busy) => (
        <>
          <Button variant="ghost" disabled={busy} onClick={() => run(() => skipWatchedItems(source.id))}>
            {text.skipAll}
          </Button>
          <Button
            variant="primary"
            loading={busy}
            disabled={picked.length === 0}
            title={text.downloadSelectedHint}
            onClick={() => run(() => downloadWatchedItems(source.id, keysOf(picked), true))}
          >
            {text.downloadSelected(picked.length)}
          </Button>
        </>
      )}
    />
  )
}

/** Skipped Before, folded to start with: the picker again, nothing ticked, and Download Selected for what's ticked. */
function SkippedItems({ source }: { source: WatchedSource }) {
  const [shown, setShown] = useState(false)
  return (
    <div className="flex flex-col gap-2">
      <ExpandToggle
        expanded={shown}
        onToggle={() => setShown((open) => !open)}
        label={text.skipped(source.skippedItems.length)}
      />
      {shown && (
        <ItemPicker
          items={source.skippedItems}
          startsTicked={() => false}
          showUploader={source.kind === "playlist"}
          actions={(picked, run, busy) => (
            <Button
              variant="secondary"
              loading={busy}
              disabled={picked.length === 0}
              onClick={() => run(() => downloadWatchedItems(source.id, keysOf(picked), false))}
            >
              {text.downloadSelected(picked.length)}
            </Button>
          )}
        />
      )}
    </div>
  )
}

/** All New Items: every source's new items in one picker, each named with its source. Download Selected queues the
 * ticked ones, each with its source's folder and preset, and skips the rest. */
function AllNewItems({ sources }: { sources: readonly WatchedSource[] }) {
  const items = allNewItems(sources)
  if (items.length === 0) return <p className="text-fg-secondary">{text.allNewNone}</p>
  return (
    <Card className="p-4">
      <ItemPicker
        items={items}
        startsTicked={(item) => !item.lastDownload}
        showUploader={false}
        sourceOf={(item: SourcedItem) => item.sourceName}
        actions={(picked, run, busy) => (
          <>
            <Button variant="ghost" disabled={busy} onClick={() => run(skipEveryNewWatchedItem)}>
              {text.skipAll}
            </Button>
            <Button
              variant="primary"
              loading={busy}
              disabled={picked.length === 0}
              title={text.downloadSelectedHint}
              onClick={() => run(() => downloadNewWatchedItems(keysOf(picked), true))}
            >
              {text.downloadSelected(picked.length)}
            </Button>
          </>
        )}
      />
    </Card>
  )
}

interface ItemPickerProps<Item extends WatchedItem> {
  items: readonly Item[]
  /** Whether an item is ticked until it's ticked or unticked by hand. One in the queue never is. */
  startsTicked: (item: Item) => boolean
  showUploader: boolean
  sourceOf?: (item: Item) => string
  /** The buttons under the items, given the ticked ones, a way to run an action (which shows its error, and starts the
   * ticks again once it's done) and whether one runs. */
  actions: (picked: Item[], run: (action: () => Promise<unknown>) => void, busy: boolean) => ReactNode
}

/** A list of a source's items in the picker's view (shared with the preview card's), with a box that ticks them all,
 * how many are ticked, and the actions under them. */
function ItemPicker<Item extends WatchedItem>({ items, startsTicked, showUploader, sourceOf, actions }: ItemPickerProps<Item>) {
  const [chosen, setChosen] = useState<ReadonlyMap<string, boolean>>(new Map())
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<ErrorDescription | null>(null)
  const { layout, change: changeLayout } = usePickerLayout("items")
  const isTicked = (item: Item) => !isQueued(item) && (chosen.get(itemKey(item)) ?? startsTicked(item))
  const picked = items.filter(isTicked)
  const selectable = items.filter((item) => !isQueued(item))
  const allTicked = selectable.length > 0 && picked.length === selectable.length

  const tick = (item: Item, on: boolean) => setChosen((current) => new Map(current).set(itemKey(item), on))

  const run = (action: () => Promise<unknown>) => {
    setBusy(true)
    setError(null)
    action()
      .then(() => setChosen(new Map()))
      .catch((caught: unknown) => setError(describeWatchedError(caught)))
      .finally(() => setBusy(false))
  }

  return (
    <div className="flex flex-col gap-2">
      <div className="flex flex-wrap items-center justify-between gap-x-3 gap-y-2">
        <label className="flex items-center gap-2 text-fg-secondary">
          <input
            type="checkbox"
            className="size-4 accent-accent"
            aria-label={text.selectAll}
            title={text.selectAll}
            checked={allTicked}
            ref={(box) => {
              if (box) box.indeterminate = picked.length > 0 && !allTicked
            }}
            disabled={busy || selectable.length === 0}
            onChange={(event) => setChosen(new Map(selectable.map((item) => [itemKey(item), event.target.checked])))}
          />
          <span className="whitespace-nowrap tabular-nums">{text.selected(picked.length, items.length)}</span>
        </label>
        <PickerViewControls layout={layout} onChange={changeLayout} />
      </div>
      <ListedItems
        items={items}
        layout={layout}
        showUploader={showUploader}
        sourceOf={sourceOf}
        isSelected={isTicked}
        disabled={busy}
        onSelect={tick}
      />
      {error && <StatusMessage tone="danger" {...error} />}
      <div className="flex justify-end gap-2">{actions(picked, run, busy)}</div>
    </div>
  )
}

function keysOf(items: readonly WatchedItem[]): ItemKey[] {
  return items.map(({ video }) => ({ site: video.site, videoId: video.videoId }))
}

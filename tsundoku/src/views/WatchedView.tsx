import { useState } from "react"
import { Download, Eye, Pencil, RefreshCw } from "lucide-react"
import {
  checkWatchedSource,
  confirmWithUser,
  downloadWatchedItems,
  editWatchedSource,
  pickFolder,
  removeWatchedSource,
  skipWatchedItems,
} from "../api"
import type { WatchedItem } from "../bindings/WatchedItem"
import type { WatchedSource } from "../bindings/WatchedSource"
import { Button } from "../components/Button"
import { Card } from "../components/Card"
import { EmptyState } from "../components/EmptyState"
import { FailureFixes } from "../components/FailureFixes"
import { Field, SelectInput, TextInput } from "../components/Field"
import { IconButton } from "../components/IconButton"
import { ExpandToggle } from "../components/LinkButton"
import { ListedItems, ListingPicture } from "../components/ListingPicker"
import { PickerViewControls } from "../components/PickerViewControls"
import { Menu } from "../components/Menu"
import { Page } from "../components/Page"
import { StatusMessage } from "../components/StatusMessage"
import { Thumbnail } from "../components/Thumbnail"
import { describePreviewError, describeWatchedError, fixFor, type ErrorDescription } from "../errors"
import {
  formatCalendarDate,
  formatContents,
  formatDuration,
  formatRoughAge,
  formatTimeAgo,
  joinDetails,
} from "../format"
import { isQueued, itemKey } from "../listingPicker"
import { usePickerLayout, usePresets, useWatchedSources } from "../queries"
import { strings } from "../strings"

const text = strings.watched

/**
 * The watched channels, playlists and accounts, in the order added: one card each with what checks found that's new,
 * Download Selected (which skips the rest), and what was skipped before, still downloadable.
 */
export function WatchedView() {
  const { data: sources = [] } = useWatchedSources()
  return (
    <Page title={strings.nav.watched}>
      {sources.length === 0 ? (
        <EmptyState icon={Eye} title={text.emptyTitle} body={text.emptyBody} />
      ) : (
        <div className="flex max-w-4xl flex-col gap-3">
          {sources.map((source) => (
            <SourceCard key={source.id} source={source} />
          ))}
        </div>
      )}
    </Page>
  )
}

function SourceCard({ source }: { source: WatchedSource }) {
  const { data: presets } = usePresets()
  const [editing, setEditing] = useState(false)
  const [error, setError] = useState<ErrorDescription | null>(null)
  const presetName = presets?.presets.find((preset) => preset.id === source.presetId)?.name

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
      <div className="flex items-start gap-4">
        <ListingPicture listing={source} small />
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <p className="truncate font-semibold" title={source.name}>
            {source.name}
          </p>
          <p className="truncate text-xs text-fg-secondary">
            {joinDetails([strings.picker.kinds[source.kind], source.siteName])}
          </p>
          <p className="text-xs text-fg-secondary tabular-nums">
            {source.checking
              ? text.checking
              : joinDetails([
                  source.lastCheckedAt === null ? text.neverChecked : text.checked(formatTimeAgo(source.lastCheckedAt)),
                  text.newCount(source.newItems.length),
                ])}
          </p>
          <p className="truncate text-xs text-fg-muted" title={source.folder ?? undefined}>
            {joinDetails([
              source.folder ? text.savesIn(source.folder) : text.savesByRules,
              presetName && text.startsAs(presetName),
            ])}
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-1">
          <IconButton
            label={source.checking ? text.checking : text.checkNow}
            icon={RefreshCw}
            variant="secondary"
            disabled={source.checking}
            onClick={check}
          />
          <IconButton label={text.edit} icon={Pencil} onClick={() => setEditing((shown) => !shown)} />
          <Menu label={text.moreActions} items={[{ label: text.remove, danger: true, onSelect: () => act(remove) }]} />
        </div>
      </div>
      {source.lastError && !source.checking && (
        <StatusMessage tone="danger" {...describePreviewError(source.lastError)}>
          {fixFor(source.lastError) && <FailureFixes error={source.lastError} onRetry={check} />}
        </StatusMessage>
      )}
      {error && <StatusMessage tone="danger" {...error} />}
      {editing && <SourceEditor source={source} onDone={() => setEditing(false)} />}
      {source.newItems.length > 0 && <NewItems source={source} />}
      {source.skippedItems.length > 0 && <SkippedItems source={source} />}
    </Card>
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
 * What checks found: each item ticked unless the history or the queue has it, kept as the items left unticked, so a
 * newer find comes in ticked. Download Selected queues the ticked ones and skips the rest.
 */
function NewItems({ source }: { source: WatchedSource }) {
  const items = source.newItems
  const [unticked, setUnticked] = useState<ReadonlySet<string>>(new Set())
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<ErrorDescription | null>(null)
  const { layout, change: changeLayout } = usePickerLayout("items")
  const isTicked = (item: WatchedItem) => !isQueued(item) && !item.lastDownload && !unticked.has(itemKey(item))
  const picked = items.filter(isTicked)
  const selectable = items.filter((item) => !isQueued(item))
  const allTicked = selectable.length > 0 && picked.length === selectable.length

  const tick = (item: WatchedItem, on: boolean) =>
    setUnticked((current) => {
      const next = new Set(current)
      if (on) next.delete(itemKey(item))
      else next.add(itemKey(item))
      return next
    })

  async function run(action: () => Promise<unknown>) {
    setBusy(true)
    setError(null)
    try {
      await action()
      setUnticked(new Set())
    } catch (caught) {
      setError(describeWatchedError(caught))
    } finally {
      setBusy(false)
    }
  }

  const keys = picked.map(({ video }) => ({
    site: video.site,
    videoId: video.videoId,
  }))
  return (
    <div className="flex flex-col gap-2 border-t border-line pt-3">
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
            onChange={(event) =>
              setUnticked(event.target.checked ? new Set() : new Set(items.map((item) => itemKey(item))))
            }
          />
          <span className="whitespace-nowrap tabular-nums">{text.selected(picked.length, items.length)}</span>
        </label>
        <PickerViewControls layout={layout} onChange={changeLayout} />
      </div>
      <ListedItems
        items={items}
        layout={layout}
        showUploader={source.kind === "playlist"}
        isSelected={isTicked}
        disabled={busy}
        onSelect={tick}
      />
      {error && <StatusMessage tone="danger" {...error} />}
      <div className="flex justify-end gap-2">
        <Button variant="ghost" disabled={busy} onClick={() => void run(() => skipWatchedItems(source.id))}>
          {text.skipAll}
        </Button>
        <Button
          variant="primary"
          loading={busy}
          disabled={picked.length === 0}
          title={text.downloadSelectedHint}
          onClick={() => void run(() => downloadWatchedItems(source.id, keys, true))}
        >
          {text.downloadSelected(picked.length)}
        </Button>
      </div>
    </div>
  )
}

/** Skipped Before, folded to start with: each item greyed, with Download. */
function SkippedItems({ source }: { source: WatchedSource }) {
  const [shown, setShown] = useState(false)
  const [error, setError] = useState<ErrorDescription | null>(null)

  function download(item: WatchedItem) {
    setError(null)
    downloadWatchedItems(source.id, [{ site: item.video.site, videoId: item.video.videoId }], false).catch(
      (caught: unknown) => setError(describeWatchedError(caught)),
    )
  }

  return (
    <div className="flex flex-col gap-2">
      <ExpandToggle
        expanded={shown}
        onToggle={() => setShown((open) => !open)}
        label={text.skipped(source.skippedItems.length)}
      />
      {error && <StatusMessage tone="danger" {...error} />}
      {shown && (
        <ul className="flex max-h-[min(20rem,40vh)] flex-col gap-1 overflow-y-auto">
          {source.skippedItems.map((item) => (
            <SkippedRow key={itemKey(item)} item={item} onDownload={() => download(item)} />
          ))}
        </ul>
      )}
    </div>
  )
}

function SkippedRow({ item, onDownload }: { item: WatchedItem; onDownload: () => void }) {
  const { video } = item
  const date = video.uploadDate
    ? formatCalendarDate(video.uploadDate)
    : item.approximateDate && formatRoughAge(item.approximateDate)
  return (
    <li className="flex items-center gap-3 py-1">
      <Thumbnail key={video.thumbnailUrl} url={video.thumbnailUrl} className="w-20 opacity-50" />
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <p className="truncate text-fg-muted" title={video.title}>
          {video.title}
        </p>
        <p className="truncate text-xs text-fg-secondary tabular-nums">
          {joinDetails([
            date,
            video.durationSeconds !== null && formatDuration(video.durationSeconds),
            formatContents(video),
          ])}
        </p>
      </div>
      <IconButton label={text.download} icon={Download} disabled={isQueued(item)} onClick={onDownload} />
    </li>
  )
}

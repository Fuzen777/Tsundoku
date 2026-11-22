import { useState, type ReactNode } from "react"
import { addWatchedSource, confirmWithUser, pickFolder } from "../api"
import type { FirstItems } from "../bindings/FirstItems"
import type { ListedItem } from "../bindings/ListedItem"
import type { Listing } from "../bindings/Listing"
import { describeWatchedError, type ErrorDescription } from "../errors"
import { isKnown, nounOf, takesPreset } from "../listingPicker"
import { useAppActions } from "../navigation"
import { usePresets, useRulePresets } from "../queries"
import { strings } from "../strings"
import { Button } from "./Button"
import { Field, SelectInput, TextInput } from "./Field"
import { StatusMessage } from "./StatusMessage"

const text = strings.watched

/** How many of a source's latest items Latest downloads, to start with: as many as a first read lists. */
const LATEST_TO_START = 50

type FirstChoice = FirstItems["kind"]

interface WatchFormProps {
  /** The listing, with every item the picker lists. */
  listing: Listing
  /** The items ticked in the picker, among those it shows: what Selected downloads. */
  selected: readonly ListedItem[]
  onCancel: () => void
  /** It's watched now: the card closes, and the Watched view opens. */
  onWatched: () => void
}

/**
 * Watch, under the picker's list: what Tsundoku downloads of it now (nothing, the latest N, the items ticked or
 * everything listed, asked each time), Save In, which beats every rule, and Videos Start As for a channel or a
 * playlist, which both apply to what's downloaded now as to what's picked later.
 */
export function WatchForm({ listing, selected, onCancel, onWatched }: WatchFormProps) {
  const { navigate } = useAppActions()
  const { data: presets } = usePresets()
  const rulePresets = useRulePresets(selected.map((item) => item.video))
  const [first, setFirst] = useState<FirstChoice>("fromNow")
  const [latest, setLatest] = useState(String(LATEST_TO_START))
  const [folder, setFolder] = useState("")
  const [presetId, setPresetId] = useState("")
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<ErrorDescription | null>(null)
  const kind = nounOf(listing)
  const noun = text.firstNouns[kind]
  const unknownCount = listing.items.filter((item) => !isKnown(item)).length
  const latestCount = Math.max(0, Number.parseInt(latest, 10) || 0)
  const takesPresets = listing.kind !== "account"
  const nothingToDownload = (first === "picked" && selected.length === 0) || (first === "all" && unknownCount === 0)

  /** The name of the preset a ticked video starts with, as the core picks it: Videos Start As, else a rule's, else
   * the default. */
  function startsWith(item: ListedItem, index: number): string | null {
    if (!presets || !takesPreset(item)) return null
    const existing = (id: string | null | undefined) => presets.presets.find((preset) => preset.id === id)
    const preset = existing(presetId) ?? existing(rulePresets[index]?.presetId) ?? existing(presets.defaultId)
    return preset?.name ?? null
  }

  async function chooseFolder() {
    const chosen = await pickFolder(text.chooseFolderTitle)
    if (chosen) setFolder(chosen)
  }

  /** What's downloaded now, once the person agreed to replace what a ticked item's download replaces; null if not. */
  async function firstItems(): Promise<FirstItems | null> {
    switch (first) {
      case "picked": {
        // As Download Selected asks before a file made with the same preset is replaced.
        const replacing = selected.filter(
          (item, index) => item.lastDownload !== null && item.lastDownload.preset === startsWith(item, index),
        ).length
        if (
          replacing > 0 &&
          !(await confirmWithUser({
            title: strings.pending.overwriteTitle,
            message: strings.pending.overwriteMessage(replacing),
            confirmLabel: strings.pending.overwriteConfirm,
            cancelLabel: strings.dialogs.cancel,
          }))
        ) {
          return null
        }
        return { kind: "picked", items: selected.map(({ video }) => ({ site: video.site, videoId: video.videoId })) }
      }
      case "latest":
        return { kind: "latest", count: latestCount }
      case "fromNow":
      case "all":
        return { kind: first }
    }
  }

  async function watch() {
    const downloaded = await firstItems()
    if (!downloaded) return
    setSaving(true)
    setError(null)
    try {
      await addWatchedSource({
        url: listing.url,
        kind: listing.kind,
        name: listing.title,
        siteName: listing.siteName,
        thumbnailUrl: listing.thumbnailUrl,
        items: listing.items.map(({ video, approximateDate }) => ({
          video,
          approximateDate,
        })),
        first: downloaded,
        folder: folder.trim() || null,
        presetId: presetId || null,
      })
      onWatched()
      navigate("watched")
    } catch (caught) {
      setError(describeWatchedError(caught))
      setSaving(false)
    }
  }

  const choice = (value: FirstChoice, label: ReactNode, disabled = false) => (
    <label className={`flex items-center gap-2 ${disabled ? "text-fg-muted" : ""}`}>
      <input
        type="radio"
        name="watch-first-items"
        className="size-4 accent-accent"
        checked={first === value}
        disabled={saving || disabled}
        onChange={() => setFirst(value)}
      />
      {label}
    </label>
  )

  return (
    <div className="flex flex-col gap-3 border-t border-line pt-3">
      <p className="font-medium">{text.watchTitle}</p>
      <fieldset className="flex flex-col gap-1.5">
        <legend className="mb-1.5 text-xs font-medium text-fg-secondary">{text.firstItems}</legend>
        {choice("fromNow", text.fromNow)}
        {choice(
          "latest",
          <span className="flex items-center gap-2">
            {text.latest}
            <TextInput
              aria-label={text.latest}
              inputMode="numeric"
              className="w-16 text-right tabular-nums"
              value={latest}
              disabled={saving}
              onFocus={() => setFirst("latest")}
              onChange={(event) => setLatest(event.target.value)}
            />
            {text.latestItems(noun)}
          </span>,
        )}
        {first === "latest" && latestCount > listing.items.length && (
          <p className="ml-6 text-xs text-fg-secondary">
            {text.latestPastListed(listing.items.length, strings.picker.nouns[kind][1])}
          </p>
        )}
        {choice("picked", text.picked(selected.length, noun), selected.length === 0)}
        {choice("all", text.all(unknownCount), unknownCount === 0)}
        <p className="mt-0.5 text-xs text-fg-secondary">{text.firstItemsHint}</p>
      </fieldset>
      <Field label={text.saveIn} hint={text.saveInHint} htmlFor="watch-folder">
        <div className="flex gap-2">
          <TextInput
            id="watch-folder"
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
      {takesPresets && (
        <Field label={text.videosStartAs} htmlFor="watch-preset">
          <SelectInput
            id="watch-preset"
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
        <Button variant="ghost" disabled={saving} onClick={onCancel}>
          {text.cancel}
        </Button>
        <Button variant="primary" loading={saving} disabled={nothingToDownload} onClick={() => void watch()}>
          {text.watch}
        </Button>
      </div>
    </div>
  )
}

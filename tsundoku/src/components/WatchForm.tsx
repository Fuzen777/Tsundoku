import { useState, type ReactNode } from "react"
import { addWatchedSource, pickFolder } from "../api"
import type { FirstItems } from "../bindings/FirstItems"
import type { Listing } from "../bindings/Listing"
import { describeWatchedError, type ErrorDescription } from "../errors"
import { isKnown, nounOf } from "../listingPicker"
import { useAppActions } from "../navigation"
import { usePresets } from "../queries"
import { strings } from "../strings"
import { Button } from "./Button"
import { Field, SelectInput, TextInput } from "./Field"
import { StatusMessage } from "./StatusMessage"

const text = strings.watched

/** How many of a source's latest items count as new when it's added, to start with. */
const LATEST_TO_START = 30

type FirstChoice = FirstItems["kind"]

interface WatchFormProps {
  listing: Listing
  onCancel: () => void
  /** It's watched now: the card closes, and the Watched view opens. */
  onWatched: () => void
}

/**
 * Watch, under the picker's list: what counts as new now (nothing, the latest N or everything listed, asked each
 * time), Save In, which beats every rule, and Videos Start As for a channel or a playlist.
 */
export function WatchForm({ listing, onCancel, onWatched }: WatchFormProps) {
  const { navigate } = useAppActions()
  const { data: presets } = usePresets()
  const [first, setFirst] = useState<FirstChoice>("latest")
  const [latest, setLatest] = useState(String(LATEST_TO_START))
  const [folder, setFolder] = useState("")
  const [presetId, setPresetId] = useState("")
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<ErrorDescription | null>(null)
  const noun = strings.picker.nouns[nounOf(listing)][1]
  const unknownCount = listing.items.filter((item) => !isKnown(item)).length
  const takesPresets = listing.kind !== "account"

  async function chooseFolder() {
    const chosen = await pickFolder(text.chooseFolderTitle)
    if (chosen) setFolder(chosen)
  }

  async function watch() {
    const count = Math.max(0, Number.parseInt(latest, 10) || 0)
    const firstItems: FirstItems =
      first === "latest" ? { kind: "latest", count } : first === "all" ? { kind: "all" } : { kind: "fromNow" }
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
        first: firstItems,
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

  const choice = (value: FirstChoice, label: ReactNode) => (
    <label className="flex items-center gap-2">
      <input
        type="radio"
        name="watch-first-items"
        className="size-4 accent-accent"
        checked={first === value}
        disabled={saving}
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
        {choice("all", text.all(unknownCount, noun))}
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
        <Button variant="primary" loading={saving} onClick={() => void watch()}>
          {text.watch}
        </Button>
      </div>
    </div>
  )
}

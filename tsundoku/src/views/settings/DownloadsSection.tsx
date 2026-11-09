import { useRef, useState } from "react"
import { useQueryClient } from "@tanstack/react-query"
import { pickFolder, queryKeys, saveDownloadOptions, setDefaultPreset } from "../../api"
import type { CookiesBrowser } from "../../bindings/CookiesBrowser"
import type { DownloadSettings } from "../../bindings/DownloadSettings"
import type { PauseRange } from "../../bindings/PauseRange"
import type { PostNamePlaceholder } from "../../bindings/PostNamePlaceholder"
import type { SitePauses } from "../../bindings/SitePauses"
import { Button } from "../../components/Button"
import { Card } from "../../components/Card"
import { Field, SelectInput, TextInput } from "../../components/Field"
import { PlaceholderButtons } from "../../components/PlaceholderButtons"
import { Section } from "../../components/Section"
import { StatusMessage } from "../../components/StatusMessage"
import { describeDownloadError, describePresetError, type ErrorDescription } from "../../errors"
import { fillPlaceholders } from "../../placeholders"
import { useDefaultDownloadFolder, useDownloadOptions, usePresets, useStorePresets } from "../../queries"
import { strings } from "../../strings"
import { useOpenDownloadFolder } from "../../useOpenDownloadFolder"

const text = strings.settings

/** The longest pause between a post's files, in seconds (`PauseRange::LONGEST` in the core). */
const LONGEST_PAUSE = 600

/** How posts' files are named unless Settings says otherwise (`post_names::DEFAULT_POST_FILE_NAME` in the core). */
const DEFAULT_POST_FILE_NAME = "{account} {date} {post} {number}"

/** What a post file's name can hold, in the order the Insert buttons offer them. */
const POST_NAME_PLACEHOLDERS: readonly PostNamePlaceholder[] = ["account", "date", "post", "number", "site"]

/** The most downloads that can run at once (`MAX_AT_ONCE` in the core). */
const AT_ONCE_CHOICES = [1, 2, 3, 4, 5] as const

const BROWSERS: readonly CookiesBrowser[] = [
  "automatic",
  "firefox",
  "zen",
  "libreWolf",
  "floorp",
  "waterfox",
  "chrome",
  "edge",
  "brave",
  "opera",
  "vivaldi",
  "chromium",
]

/** Browsers that keep Firefox's cookie database, which can be read while they run. */
const FIREFOX_FAMILY: readonly CookiesBrowser[] = ["automatic", "firefox", "zen", "libreWolf", "floorp", "waterfox"]

/** Saves download options as they change. Each section shows its own error. */
function useSaveOptions() {
  const queryClient = useQueryClient()
  const { data: options } = useDownloadOptions()
  const [error, setError] = useState<ErrorDescription | null>(null)
  const save = async (changes: Partial<DownloadSettings>) => {
    if (!options) return false
    setError(null)
    try {
      queryClient.setQueryData(queryKeys.downloadOptions, await saveDownloadOptions({ ...options, ...changes }))
      return true
    } catch (caught) {
      setError(describeDownloadError(caught))
      return false
    }
  }
  return { options, save, error }
}

/** Settings → Downloads: the download folder, how many at once, the speed limit, the default preset. */
export function DownloadsSection() {
  const queryClient = useQueryClient()
  const { options, save, error } = useSaveOptions()
  const { data: defaultFolder } = useDefaultDownloadFolder()
  const { data: presets } = usePresets()
  const storePresets = useStorePresets()
  const [presetError, setPresetError] = useState<ErrorDescription | null>(null)
  const openFolder = useOpenDownloadFolder()
  // The speed limit is saved when the field is left or Enter is pressed, not at every key.
  const [rateLimit, setRateLimit] = useState<string | null>(null)
  const [rateLimitSaved, setRateLimitSaved] = useState(false)
  const shownRateLimit = rateLimit ?? options?.rateLimit ?? ""
  const folder = options?.folder ?? null
  // As the core resolves it: a portable copy's folder beside it is saved without the copy's own folder.
  const shownFolder = presets?.defaultFolder ?? folder ?? defaultFolder ?? ""

  async function saveFolder(chosen: string | null) {
    if (!(await save({ folder: chosen }))) return
    // Presets without a folder of their own, rules and the preview card show the download folder.
    void queryClient.invalidateQueries({ queryKey: queryKeys.downloadDestination })
    await queryClient.invalidateQueries({ queryKey: queryKeys.presets })
  }

  async function chooseFolder() {
    const chosen = await pickFolder(text.folderTitle)
    if (chosen) await saveFolder(chosen)
  }

  async function saveRateLimit() {
    if (rateLimit === null) return
    if (await save({ rateLimit: rateLimit.trim() || null })) {
      setRateLimit(null)
      setRateLimitSaved(true)
    }
  }

  async function chooseDefaultPreset(id: string) {
    setPresetError(null)
    try {
      storePresets(await setDefaultPreset(id))
    } catch (caught) {
      setPresetError(describePresetError(caught))
    }
  }

  return (
    <Section title={text.downloads}>
      <Card className="flex flex-col gap-4 p-4">
        <Field
          label={text.folder}
          hint={folder || !defaultFolder ? text.folderHint : `${text.folderHint} ${text.defaultFolder(defaultFolder)}`}
        >
          <div className="flex gap-2">
            <TextInput readOnly className="flex-1" value={shownFolder} title={shownFolder} />
            <Button disabled={!options} onClick={() => void chooseFolder()}>
              {text.chooseFolder}
            </Button>
            <Button onClick={() => void openFolder.open()}>{text.openDownloadFolder}</Button>
            {folder && (
              <Button variant="ghost" onClick={() => void saveFolder(null)}>
                {text.useDefaultFolder}
              </Button>
            )}
          </div>
        </Field>
        {openFolder.error !== null && (
          <StatusMessage tone="danger" message={text.downloadFolderOpenFailed} detail={openFolder.error} />
        )}
        <div className="grid grid-cols-[minmax(0,12rem)_minmax(0,1fr)] gap-4">
          <Field label={text.atOnce} hint={text.atOnceHint} htmlFor="settings-at-once">
            <SelectInput
              id="settings-at-once"
              value={options?.atOnce ?? 2}
              disabled={!options}
              onChange={(event) => void save({ atOnce: Number(event.target.value) })}
            >
              {AT_ONCE_CHOICES.map((count) => (
                <option key={count} value={count}>
                  {count}
                </option>
              ))}
            </SelectInput>
          </Field>
          <Field label={text.defaultPreset} hint={text.defaultPresetHint} htmlFor="settings-default-preset">
            <SelectInput
              id="settings-default-preset"
              value={presets?.defaultId ?? ""}
              disabled={!presets}
              onChange={(event) => void chooseDefaultPreset(event.target.value)}
            >
              {presets?.presets.map((preset) => (
                <option key={preset.id} value={preset.id}>
                  {preset.name}
                </option>
              ))}
            </SelectInput>
          </Field>
        </div>
        <Field label={text.rateLimit} hint={text.rateLimitHint} htmlFor="settings-rate-limit">
          <div className="flex items-center gap-3">
            <TextInput
              id="settings-rate-limit"
              className="w-40 font-mono"
              placeholder={text.rateLimitPlaceholder}
              value={shownRateLimit}
              disabled={!options}
              onChange={(event) => {
                setRateLimitSaved(false)
                setRateLimit(event.target.value)
              }}
              onBlur={() => void saveRateLimit()}
              onKeyDown={(event) => {
                if (event.key === "Enter") void saveRateLimit()
              }}
            />
            {rateLimitSaved && <StatusMessage tone="success" message={text.saved} />}
          </div>
        </Field>
        {error && <StatusMessage tone="danger" {...error} />}
        {presetError && <StatusMessage tone="danger" {...presetError} />}
      </Card>
    </Section>
  )
}

/** Settings → Posts of Images: how the files of a post are named, with a live example under the field. */
export function PostsSection() {
  const { options, save, error } = useSaveOptions()
  const input = useRef<HTMLInputElement>(null)
  // The name is saved when the field is left or Enter is pressed, not at every key.
  const [name, setName] = useState<string | null>(null)
  const [saved, setSaved] = useState(false)
  const shownName = name ?? options?.postFileName ?? ""
  const example = `${fillPlaceholders(shownName.trim() || DEFAULT_POST_FILE_NAME, text.postNameSamples)}.${text.postNameSampleExtension}`

  async function saveName(changed: string | null) {
    if (changed === null) return
    if (await save({ postFileName: changed })) {
      setName(null)
      setSaved(true)
    }
  }

  function edit(changed: string) {
    setSaved(false)
    setName(changed)
  }

  return (
    <Section title={text.posts}>
      <Card className="flex flex-col gap-3 p-4">
        <Field
          label={text.postFileName}
          hint={text.postFileNameHint}
          error={error?.message}
          htmlFor="settings-post-file-name"
        >
          <div className="flex flex-col gap-2">
            <div className="flex items-center gap-2">
              <TextInput
                ref={input}
                id="settings-post-file-name"
                className="flex-1 font-mono"
                value={shownName}
                disabled={!options}
                onChange={(event) => edit(event.target.value)}
                onBlur={() => void saveName(name)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") void saveName(name)
                }}
              />
              {shownName.trim() !== DEFAULT_POST_FILE_NAME && (
                <Button
                  variant="ghost"
                  disabled={!options}
                  // Keeps the field from saving what's in it first, as leaving it would.
                  onMouseDown={(event) => event.preventDefault()}
                  onClick={() => void saveName(DEFAULT_POST_FILE_NAME)}
                >
                  {text.useDefaultName}
                </Button>
              )}
            </div>
            <PlaceholderButtons
              input={input}
              value={shownName}
              placeholders={POST_NAME_PLACEHOLDERS}
              hints={text.postNamePlaceholderHints}
              onChange={edit}
            />
            <div className="flex items-center gap-3">
              <p className="min-w-0 flex-1 truncate text-xs text-fg-muted" title={example}>
                {text.postFileNameExample} <span className="font-mono text-fg-secondary select-text">{example}</span>
              </p>
              {saved && <StatusMessage tone="success" message={text.saved} />}
            </div>
          </div>
        </Field>
      </Card>
    </Section>
  )
}

/** The sites besides Instagram that Settings → Pacing can give a pause, in the order shown. */
const OTHER_PACED_SITES: readonly { key: keyof SitePauses; name: string }[] = [
  { key: "x", name: "X" },
  { key: "pixiv", name: "Pixiv" },
  { key: "tumblr", name: "Tumblr" },
]

/** Settings → Pacing: Instagram's pause before each file and between its links in Pending, and its hourly limit; then
 * the same pause for X, Pixiv and Tumblr, none until one is set. */
export function PacingSection() {
  const { options, save, error } = useSaveOptions()
  const sitePauses = options?.sitePauses ?? { x: null, pixiv: null, tumblr: null }

  // The hourly limit is saved when the field is left or Enter is pressed, not at every key; empty for none.
  const [hourlyLimit, setHourlyLimit] = useState<string | null>(null)
  const savedLimit = options?.hourlyFileLimit ?? null
  const shownLimit = hourlyLimit ?? (savedLimit === null ? "" : String(savedLimit))

  async function saveHourlyLimit() {
    if (hourlyLimit === null) return
    const number = Number.parseInt(hourlyLimit, 10)
    const changed = Number.isNaN(number) || number <= 0 ? null : number
    if (await save({ hourlyFileLimit: changed })) setHourlyLimit(null)
  }

  return (
    <Section title={text.pacing}>
      <Card className="flex flex-col gap-3 p-4">
        <p className="text-fg-secondary">{text.pacingIntro(LONGEST_PAUSE)}</p>
        <p className="text-xs font-semibold text-fg-secondary">{text.instagram}</p>
        <Field label={text.postPause} hint={text.postPauseHint} htmlFor="settings-pause-instagram-min">
          <PauseFields
            id="settings-pause-instagram"
            saved={options?.postPause ?? { min: 5, max: 10 }}
            disabled={!options}
            onSave={(range) => (range ? save({ postPause: range }) : Promise.resolve(false))}
          />
        </Field>
        <Field label={text.hourlyLimit} hint={text.hourlyLimitHint} htmlFor="settings-hourly-limit">
          <div className="flex items-center gap-2 text-fg-secondary">
            <TextInput
              id="settings-hourly-limit"
              inputMode="numeric"
              className="w-24 text-right tabular-nums"
              placeholder={text.hourlyLimitPlaceholder}
              value={shownLimit}
              disabled={!options}
              onChange={(event) => setHourlyLimit(event.target.value)}
              onBlur={() => void saveHourlyLimit()}
              onKeyDown={(event) => {
                if (event.key === "Enter") void saveHourlyLimit()
              }}
            />
            <span>{text.files}</span>
          </div>
        </Field>
        <div className="flex flex-col gap-3 border-t border-line pt-3">
          <p className="text-xs text-fg-secondary">{text.otherSitesPause}</p>
          {OTHER_PACED_SITES.map(({ key, name }) => (
            <Field key={key} label={name} htmlFor={`settings-pause-${key}-min`}>
              <PauseFields
                id={`settings-pause-${key}`}
                saved={sitePauses[key]}
                disabled={!options}
                onSave={(range) => save({ sitePauses: { ...sitePauses, [key]: range } })}
              />
            </Field>
          ))}
        </div>
        {error && <StatusMessage tone="danger" {...error} />}
      </Card>
    </Section>
  )
}

/**
 * "From [5] to [10] seconds", saved when a field is left or Enter is pressed. `saved` null shows both fields empty
 * (No Pause), and emptying both saves null; one end left empty takes the other's value.
 */
function PauseFields({
  id,
  saved,
  disabled,
  onSave,
}: {
  id: string
  saved: PauseRange | null
  disabled: boolean
  onSave: (range: PauseRange | null) => Promise<boolean>
}) {
  const [draft, setDraft] = useState<{ min: string; max: string } | null>(null)
  const shown = draft ?? { min: saved ? String(saved.min) : "", max: saved ? String(saved.max) : "" }

  async function commit() {
    if (draft === null) return
    const read = (value: string) => {
      const number = Number.parseInt(value, 10)
      return Number.isNaN(number) ? null : Math.max(0, number)
    }
    const min = read(draft.min)
    const max = read(draft.max)
    const range = min === null && max === null ? null : { min: min ?? max ?? 0, max: max ?? min ?? 0 }
    if (await onSave(range)) setDraft(null)
  }

  const input = (end: "min" | "max", label: string) => (
    <TextInput
      id={`${id}-${end}`}
      aria-label={label}
      inputMode="numeric"
      className="w-20 text-right tabular-nums"
      placeholder={text.noPause}
      value={shown[end]}
      disabled={disabled}
      onChange={(event) => setDraft({ ...shown, [end]: event.target.value })}
      onBlur={() => void commit()}
      onKeyDown={(event) => {
        if (event.key === "Enter") void commit()
      }}
    />
  )

  return (
    <div className="flex items-center gap-2 text-fg-secondary">
      <span>{text.pauseFrom}</span>
      {input("min", text.pauseFrom)}
      <span>{text.pauseTo}</span>
      {input("max", text.pauseTo)}
      <span>{text.seconds}</span>
    </div>
  )
}

/** Settings → Cookies: where cookies are read from, for links that need a signed-in account. */
export function CookiesSection() {
  const { options, save, error } = useSaveOptions()
  const browser = options?.cookiesBrowser ?? null
  return (
    <Section title={text.cookies}>
      <Card className="flex flex-col gap-3 p-4">
        <Field label={text.cookiesBrowser} hint={text.cookiesHint} htmlFor="settings-cookies">
          <SelectInput
            id="settings-cookies"
            className="w-48"
            value={browser ?? ""}
            disabled={!options}
            onChange={(event) => void save({ cookiesBrowser: (event.target.value || null) as CookiesBrowser | null })}
          >
            <option value="">{text.cookiesNone}</option>
            {BROWSERS.map((option) => (
              <option key={option} value={option}>
                {text.browsers[option]}
              </option>
            ))}
          </SelectInput>
        </Field>
        {browser !== null && !FIREFOX_FAMILY.includes(browser) && <StatusMessage tone="warning" message={text.chromiumWarning} />}
        {error && <StatusMessage tone="danger" {...error} />}
      </Card>
    </Section>
  )
}

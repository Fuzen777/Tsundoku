import { useState } from "react"
import { useQueryClient } from "@tanstack/react-query"
import { pickFolder, queryKeys, saveDownloadOptions, setDefaultPreset } from "../../api"
import type { CookiesBrowser } from "../../bindings/CookiesBrowser"
import type { DownloadSettings } from "../../bindings/DownloadSettings"
import type { PauseRange } from "../../bindings/PauseRange"
import { Button } from "../../components/Button"
import { Card } from "../../components/Card"
import { Field, SelectInput, TextInput } from "../../components/Field"
import { Section } from "../../components/Section"
import { StatusMessage } from "../../components/StatusMessage"
import { describeDownloadError, describePresetError, type ErrorDescription } from "../../errors"
import { useDefaultDownloadFolder, useDownloadOptions, usePresets, useStorePresets } from "../../queries"
import { strings } from "../../strings"

const text = strings.settings

/** The longest pause between a post's files, in seconds (`PauseRange::LONGEST` in the core). */
const LONGEST_PAUSE = 600

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
  // The speed limit is saved when the field is left or Enter is pressed, not at every key.
  const [rateLimit, setRateLimit] = useState<string | null>(null)
  const [rateLimitSaved, setRateLimitSaved] = useState(false)
  const shownRateLimit = rateLimit ?? options?.rateLimit ?? ""
  const folder = options?.folder ?? null

  async function saveFolder(chosen: string | null) {
    // Presets without a folder of their own show the download folder.
    if (await save({ folder: chosen })) await queryClient.invalidateQueries({ queryKey: queryKeys.presets })
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
            <TextInput readOnly className="flex-1" value={folder ?? defaultFolder ?? ""} title={folder ?? defaultFolder ?? ""} />
            <Button disabled={!options} onClick={() => void chooseFolder()}>
              {text.chooseFolder}
            </Button>
            {folder && (
              <Button variant="ghost" onClick={() => void saveFolder(null)}>
                {text.useDefaultFolder}
              </Button>
            )}
          </div>
        </Field>
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

/** Settings → Posts of Images: where posts go, each site in its own folder, and the pause between Instagram's files. */
export function PostsSection() {
  const { options, save, error } = useSaveOptions()
  const { data: defaultDownloadFolder } = useDefaultDownloadFolder()
  // The pause is saved when a field is left or Enter is pressed, not at every key.
  const [pause, setPause] = useState<{ min: string; max: string } | null>(null)
  const savedPause = options?.postPause ?? { min: 10, max: 20 }
  const shownPause = pause ?? { min: String(savedPause.min), max: String(savedPause.max) }
  const folder = options?.postsFolder ?? null
  // Without a folder of their own, posts go to the download folder.
  const defaultFolder = options?.folder ?? defaultDownloadFolder ?? null

  async function chooseFolder() {
    const chosen = await pickFolder(text.postsFolderTitle)
    if (chosen) await save({ postsFolder: chosen })
  }

  async function savePause() {
    if (pause === null) return
    const read = (value: string, fallback: number) => {
      const number = Number.parseInt(value, 10)
      return Number.isNaN(number) ? fallback : Math.max(0, number)
    }
    const changed: PauseRange = { min: read(pause.min, savedPause.min), max: read(pause.max, savedPause.max) }
    if (await save({ postPause: changed })) setPause(null)
  }

  const pauseInput = (end: "min" | "max", label: string) => (
    <TextInput
      id={`settings-pause-${end}`}
      aria-label={label}
      inputMode="numeric"
      className="w-20 text-right tabular-nums"
      value={shownPause[end]}
      disabled={!options}
      onChange={(event) => setPause({ ...shownPause, [end]: event.target.value })}
      onBlur={() => void savePause()}
      onKeyDown={(event) => {
        if (event.key === "Enter") void savePause()
      }}
    />
  )

  return (
    <Section title={text.posts}>
      <Card className="flex flex-col gap-4 p-4">
        <Field
          label={text.postsFolder}
          hint={folder ? text.postsFolderHint : `${text.postsFolderHint} ${text.postsInDownloadFolder}`}
        >
          <div className="flex gap-2">
            <TextInput readOnly className="flex-1" value={folder ?? defaultFolder ?? ""} title={folder ?? defaultFolder ?? ""} />
            <Button disabled={!options} onClick={() => void chooseFolder()}>
              {text.chooseFolder}
            </Button>
            {folder && (
              <Button variant="ghost" onClick={() => void save({ postsFolder: null })}>
                {text.useDefaultFolder}
              </Button>
            )}
          </div>
        </Field>
        <Field label={text.postPause} hint={text.postPauseHint(LONGEST_PAUSE)} htmlFor="settings-pause-min">
          <div className="flex items-center gap-2 text-fg-secondary">
            <span>{text.pauseFrom}</span>
            {pauseInput("min", text.pauseFrom)}
            <span>{text.pauseTo}</span>
            {pauseInput("max", text.pauseTo)}
            <span>{text.seconds}</span>
          </div>
        </Field>
        {error && <StatusMessage tone="danger" {...error} />}
      </Card>
    </Section>
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

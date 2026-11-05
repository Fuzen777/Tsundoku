import { useState } from "react"
import { useQueryClient } from "@tanstack/react-query"
import { queryKeys, saveDownloadOptions, setDefaultPreset } from "../../api"
import type { CookiesBrowser } from "../../bindings/CookiesBrowser"
import type { DownloadSettings } from "../../bindings/DownloadSettings"
import { Card } from "../../components/Card"
import { Field, SelectInput, TextInput } from "../../components/Field"
import { Section } from "../../components/Section"
import { StatusMessage } from "../../components/StatusMessage"
import { describeDownloadError, describePresetError, type ErrorDescription } from "../../errors"
import { useDownloadOptions, usePresets, useStorePresets } from "../../queries"
import { strings } from "../../strings"

const text = strings.settings

/** The most downloads that can run at once (`MAX_AT_ONCE` in the core). */
const AT_ONCE_CHOICES = [1, 2, 3, 4, 5] as const

const BROWSERS: readonly CookiesBrowser[] = ["firefox", "chrome", "edge", "brave", "opera", "vivaldi", "chromium"]

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

/** Settings → Downloads: how many at once, the speed limit, the default preset. */
export function DownloadsSection() {
  const { options, save, error } = useSaveOptions()
  const { data: presets } = usePresets()
  const storePresets = useStorePresets()
  const [presetError, setPresetError] = useState<ErrorDescription | null>(null)
  // The speed limit is saved when the field is left or Enter is pressed, not at every key.
  const [rateLimit, setRateLimit] = useState<string | null>(null)
  const [rateLimitSaved, setRateLimitSaved] = useState(false)
  const shownRateLimit = rateLimit ?? options?.rateLimit ?? ""

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

/** Settings → Cookies: the browser yt-dlp reads cookies from, for videos that need a signed-in account. */
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
        {browser !== null && browser !== "firefox" && <StatusMessage tone="warning" message={text.chromiumWarning} />}
        {error && <StatusMessage tone="danger" {...error} />}
      </Card>
    </Section>
  )
}

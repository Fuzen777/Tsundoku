import { useState } from "react"
import { useQueryClient } from "@tanstack/react-query"
import { pickFolder, queryKeys, setDefaultPreset } from "../../api"
import type { CookiesBrowser } from "../../bindings/CookiesBrowser"
import type { SitePace } from "../../bindings/SitePace"
import type { SitePacing } from "../../bindings/SitePacing"
import { Button } from "../../components/Button"
import { Card } from "../../components/Card"
import { Field, SelectInput, TextInput } from "../../components/Field"
import { HelpMenu } from "../../components/HelpMenu"
import { DEFAULT_POST_FILE_NAME, PostNameInput } from "../../components/PostNameInput"
import { Section } from "../../components/Section"
import { StatusMessage } from "../../components/StatusMessage"
import { describePresetError, type ErrorDescription } from "../../errors"
import { useDefaultDownloadFolder, usePresets, useSaveDownloadOptions, useStorePresets } from "../../queries"
import { strings } from "../../strings"
import { useOpenDownloadFolder } from "../../useOpenDownloadFolder"

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

/** Settings → Downloads: the download folder, how many at once, the speed limit, the default preset. */
export function DownloadsSection() {
  const queryClient = useQueryClient()
  const { options, save, error } = useSaveDownloadOptions()
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
  const { options, save, error } = useSaveDownloadOptions()
  // The name is saved when the field is left or Enter is pressed, not at every key.
  const [name, setName] = useState<string | null>(null)
  const [saved, setSaved] = useState(false)
  const shownName = name ?? options?.postFileName ?? ""

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
          <PostNameInput
            id="settings-post-file-name"
            value={shownName}
            onChange={edit}
            emptyName={DEFAULT_POST_FILE_NAME}
            disabled={!options}
            onBlur={() => void saveName(name)}
            onKeyDown={(event) => {
              if (event.key === "Enter") void saveName(name)
            }}
            besideField={
              shownName.trim() !== DEFAULT_POST_FILE_NAME && (
                <Button
                  variant="ghost"
                  disabled={!options}
                  // Keeps the field from saving what's in it first, as leaving it would.
                  onMouseDown={(event) => event.preventDefault()}
                  onClick={() => void saveName(DEFAULT_POST_FILE_NAME)}
                >
                  {text.useDefaultName}
                </Button>
              )
            }
            besideExample={saved && <StatusMessage tone="success" message={text.saved} />}
          />
        </Field>
      </Card>
    </Section>
  )
}

/** The sites Settings → Pacing paces, in the order shown. */
const PACED_SITES: readonly (keyof SitePacing)[] = ["instagram", "x", "pixiv", "tumblr"]

/** As the core has it before Settings is read (`SitePacing::default`). */
const DEFAULT_PACING: SitePacing = {
  instagram: { atOnce: 1, pause: { min: 3, max: 5 } },
  x: { atOnce: 1, pause: { min: 0, max: 2 } },
  pixiv: { atOnce: 1, pause: { min: 0, max: 2 } },
  tumblr: { atOnce: 1, pause: { min: 0, max: 2 } },
}

/** Settings → Pacing: a line for each site that blocks accounts reading too fast, "X from [0] to [2] seconds, [1] at
 * once", then Instagram's hourly limit. What each does is in its (?), How Pacing Works. */
export function PacingSection() {
  const { options, save, error } = useSaveDownloadOptions()
  const pacing = options?.pacing ?? DEFAULT_PACING
  const help = text.pacingHelp

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
    <Section
      title={text.pacing}
      actions={
        <HelpMenu
          label={help.label}
          topics={[
            { title: help.pause, paragraphs: help.pauseParagraphs(LONGEST_PAUSE) },
            { title: help.atOnce, paragraphs: help.atOnceParagraphs },
            { title: text.hourlyLimit, paragraphs: help.hourlyLimitParagraphs },
          ]}
        />
      }
    >
      <Card className="flex flex-col gap-3 p-4">
        <p className="text-fg-secondary">{text.pacingIntro}</p>
        <div className="grid w-fit grid-cols-[repeat(8,auto)] items-center gap-x-2 gap-y-2 text-fg-secondary">
          {PACED_SITES.map((site) => (
            <PaceLine
              key={site}
              id={`settings-pace-${site}`}
              site={text.pacedSites[site]}
              saved={pacing[site]}
              disabled={!options}
              onSave={(pace) => save({ pacing: { ...pacing, [site]: pace } })}
            />
          ))}
        </div>
        <div className="flex items-center gap-2 border-t border-line pt-3 text-fg-secondary">
          <label htmlFor="settings-hourly-limit">{text.hourlyLimitLine.before}</label>
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
          <span>{text.hourlyLimitLine.after}</span>
        </div>
        {error && <StatusMessage tone="danger" {...error} />}
      </Card>
    </Section>
  )
}

/**
 * One site's line, as the cells of `PacingSection`'s grid: "X from [0] to [2] seconds, [1] at once". The pause is
 * saved when a field is left or Enter is pressed, one end left empty taking the other's value (both empty: no pause),
 * and the other end following the one typed in when they cross; how many at once as soon as it's picked.
 */
function PaceLine({
  id,
  site,
  saved,
  disabled,
  onSave,
}: {
  id: string
  site: string
  saved: SitePace
  disabled: boolean
  onSave: (pace: SitePace) => Promise<boolean>
}) {
  // `edited` is the end typed in last, which the other follows when they cross: "from 5" with "to 2" makes 5 to 5.
  const [draft, setDraft] = useState<{ min: string; max: string; edited: "min" | "max" } | null>(null)
  const shown = draft ?? { min: String(saved.pause.min), max: String(saved.pause.max) }
  const line = text.pacingLine

  async function commit() {
    if (draft === null) return
    const read = (value: string) => {
      const number = Number.parseInt(value, 10)
      return Number.isNaN(number) ? null : Math.max(0, number)
    }
    const min = read(draft.min) ?? read(draft.max) ?? 0
    const max = read(draft.max) ?? min
    const pause = min <= max ? { min, max } : draft.edited === "min" ? { min, max: min } : { min: max, max }
    if (await onSave({ ...saved, pause })) setDraft(null)
  }

  const input = (end: "min" | "max", label: string) => (
    <TextInput
      id={`${id}-${end}`}
      aria-label={label}
      inputMode="numeric"
      className="w-16 text-right tabular-nums"
      placeholder="0"
      value={shown[end]}
      disabled={disabled}
      onChange={(event) => setDraft({ ...shown, [end]: event.target.value, edited: end })}
      onBlur={() => void commit()}
      onKeyDown={(event) => {
        if (event.key === "Enter") void commit()
      }}
    />
  )

  return (
    <>
      <label htmlFor={`${id}-min`} className="pr-1 font-medium text-fg">
        {site}
      </label>
      <span>{line.from}</span>
      {input("min", text.pauseFrom(site))}
      <span>{line.to}</span>
      {input("max", text.pauseTo(site))}
      <span>{line.seconds}</span>
      <SelectInput
        id={`${id}-at-once`}
        aria-label={text.siteAtOnce(site)}
        className="w-20"
        value={saved.atOnce}
        disabled={disabled}
        onChange={(event) => void onSave({ ...saved, atOnce: Number(event.target.value) })}
      >
        {AT_ONCE_CHOICES.map((count) => (
          <option key={count} value={count}>
            {count}
          </option>
        ))}
      </SelectInput>
      <span>{line.atOnce}</span>
    </>
  )
}

/** Settings → Cookies: where cookies are read from, for links that need a signed-in account. */
export function CookiesSection() {
  const { options, save, error } = useSaveDownloadOptions()
  const browser = options?.cookiesBrowser ?? null
  return (
    <Section title={text.cookies}>
      <Card className="flex flex-col gap-3 p-4">
        <div className="flex flex-col gap-1.5">
          <div className="flex items-center gap-2">
            <label htmlFor="settings-cookies-browser" className="font-medium">
              {text.cookiesBrowser}
            </label>
            <SelectInput
              id="settings-cookies-browser"
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
          </div>
          <p className="text-xs text-fg-muted">{text.cookiesHint}</p>
        </div>
        {browser !== null && !FIREFOX_FAMILY.includes(browser) && <StatusMessage tone="warning" message={text.chromiumWarning} />}
        {error && <StatusMessage tone="danger" {...error} />}
      </Card>
    </Section>
  )
}

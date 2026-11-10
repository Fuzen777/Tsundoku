import { useState } from "react"
import { ArrowDown, ArrowUp, Copy, RotateCcw, Trash2 } from "lucide-react"
import {
  confirmWithUser,
  deletePreset,
  duplicatePreset,
  movePreset,
  pickFolder,
  resetPreset,
  setDefaultPreset,
} from "../../api"
import type { AudioFormat } from "../../bindings/AudioFormat"
import type { Preset } from "../../bindings/Preset"
import type { PresetsOverview } from "../../bindings/PresetsOverview"
import type { SponsorBlock } from "../../bindings/SponsorBlock"
import type { VideoContainer } from "../../bindings/VideoContainer"
import { Button } from "../../components/Button"
import { Card } from "../../components/Card"
import { Checkbox, Field, SelectInput, TextInput } from "../../components/Field"
import { IconButton } from "../../components/IconButton"
import { SegmentedControl } from "../../components/SegmentedControl"
import { StatusMessage } from "../../components/StatusMessage"
import { describePresetError, type ErrorDescription } from "../../errors"
import { usePresetArguments, useStorePresets } from "../../queries"
import { strings } from "../../strings"
import type { PresetSaveState } from "./usePresetDraft"
import { argumentsLine, AUDIO_FORMATS, heightLabel, VIDEO_CONTAINERS, VIDEO_HEIGHTS } from "./presetLabels"

const fields = strings.presets.fields
const hints = strings.presets.hints

const TYPE_OPTIONS = [
  { value: "video", label: fields.video },
  { value: "audio", label: fields.audio },
] as const
const CONTAINER_OPTIONS = VIDEO_CONTAINERS.map((value) => ({ value, label: fields.containers[value] }))
const AUDIO_FORMAT_OPTIONS = AUDIO_FORMATS.map((value) => ({ value, label: fields.audioFormats[value] }))
const SPONSOR_BLOCK_OPTIONS = (["off", "mark", "remove"] as const).map((value) => ({
  value,
  label: fields.sponsorBlockModes[value],
}))

interface PresetEditorProps {
  /** The preset as saved. */
  saved: Preset
  /** The preset as edited, saved on its own once edits pause. */
  draft: Preset
  overview: PresetsOverview
  saveState: PresetSaveState
  /** Why the last save failed, until one works. */
  saveError: ErrorDescription | null
  onChange: (draft: Preset) => void
  /** Saves the waiting edits at once, resolving to whether it worked. */
  onSaveNow: () => Promise<boolean>
  /** Drops the waiting edits. */
  onDiscard: () => void
  /** Another preset should be selected: a copy just made, or the first after a delete. */
  onSelect: (id: string) => void
}

export function PresetEditor({
  saved,
  draft,
  overview,
  saveState,
  saveError,
  onChange,
  onSaveNow,
  onDiscard,
  onSelect,
}: PresetEditorProps) {
  const storePresets = useStorePresets()
  const args = usePresetArguments(draft)
  const [actionError, setActionError] = useState<ErrorDescription | null>(null)
  const isBuiltIn = overview.builtInIds.includes(saved.id)
  const isDefault = overview.defaultId === saved.id
  const index = overview.presets.findIndex((preset) => preset.id === saved.id)
  // The default is always first, so nothing moves above it and it doesn't move.
  const lowestIndex = overview.presets[0]?.id === overview.defaultId ? 1 : 0
  const argumentsError = args.error ? describePresetError(args.error).message : undefined
  const { output } = draft
  const defaultFolder = overview.defaultFolder

  const update = (changes: Partial<Preset>) => onChange({ ...draft, ...changes })

  async function run(action: () => Promise<void>) {
    setActionError(null)
    try {
      await action()
    } catch (error) {
      setActionError(describePresetError(error))
    }
  }

  const setAsDefault = () =>
    run(async () => {
      storePresets(await setDefaultPreset(saved.id))
    })
  // A copy is of the preset as saved, so the edits are saved first.
  const duplicate = () =>
    run(async () => {
      if (await onSaveNow()) onSelect((await duplicatePreset(saved.id, strings.presets.copyName(draft.name))).id)
    })
  const moveTo = (to: number) =>
    run(async () => {
      storePresets(await movePreset(saved.id, to))
    })
  const reset = () =>
    run(async () => {
      onDiscard()
      storePresets(await resetPreset(saved.id))
    })
  const remove = () =>
    run(async () => {
      const confirmed = await confirmWithUser({
        title: strings.presets.deleteTitle,
        message: strings.presets.deleteMessage(saved.name),
        confirmLabel: strings.presets.delete,
        cancelLabel: strings.dialogs.cancel,
      })
      if (!confirmed) return
      onDiscard()
      const updated = await deletePreset(saved.id)
      storePresets(updated)
      onSelect(updated.presets[0].id)
    })
  const chooseFolder = async () => {
    const folder = await pickFolder(fields.chooseFolderTitle)
    if (folder) update({ folder })
  }

  return (
    <Card className="flex flex-col gap-5 p-5">
      <div className="flex flex-wrap items-center justify-end gap-1">
        {!isDefault && (
          <Button size="sm" onClick={() => void setAsDefault()}>
            {strings.presets.setAsDefault}
          </Button>
        )}
        {!isDefault && (
          <>
            <IconButton
              label={strings.presets.moveUp}
              icon={ArrowUp}
              disabled={index <= lowestIndex}
              onClick={() => void moveTo(index - 1)}
            />
            <IconButton
              label={strings.presets.moveDown}
              icon={ArrowDown}
              disabled={index >= overview.presets.length - 1}
              onClick={() => void moveTo(index + 1)}
            />
          </>
        )}
        <IconButton label={strings.presets.duplicate} icon={Copy} onClick={() => void duplicate()} />
        {isBuiltIn && (
          <IconButton label={strings.presets.reset} icon={RotateCcw} onClick={() => void reset()} />
        )}
        {!isDefault && (
          <IconButton label={strings.presets.delete} icon={Trash2} onClick={() => void remove()} />
        )}
      </div>

      <Field label={fields.name} htmlFor="preset-name">
        <TextInput id="preset-name" value={draft.name} onChange={(event) => update({ name: event.target.value })} />
      </Field>

      <Field label={fields.type}>
        <SegmentedControl
          label={fields.type}
          options={TYPE_OPTIONS}
          value={output.kind}
          onChange={(kind) =>
            update({
              output:
                kind === "video" ? { kind, maxHeight: null, container: "mkv" } : { kind, format: "original" },
            })
          }
        />
      </Field>

      {output.kind === "video" ? (
        <div className="grid grid-cols-[minmax(0,12rem)_minmax(0,1fr)] gap-4">
          <Field label={fields.quality} htmlFor="preset-quality">
            <SelectInput
              id="preset-quality"
              value={output.maxHeight ?? ""}
              onChange={(event) =>
                update({
                  output: { ...output, maxHeight: event.target.value === "" ? null : Number(event.target.value) },
                })
              }
            >
              {VIDEO_HEIGHTS.map((height) => (
                <option key={height ?? "best"} value={height ?? ""}>
                  {heightLabel(height)}
                </option>
              ))}
            </SelectInput>
          </Field>
          <Field label={fields.container} hint={hints.containers[output.container]}>
            <SegmentedControl<VideoContainer>
              label={fields.container}
              options={CONTAINER_OPTIONS}
              value={output.container}
              onChange={(container) => update({ output: { ...output, container } })}
            />
          </Field>
        </div>
      ) : (
        <Field label={fields.audioFormat} hint={hints.audioFormats[output.format]}>
          <SegmentedControl<AudioFormat>
            label={fields.audioFormat}
            options={AUDIO_FORMAT_OPTIONS}
            value={output.format}
            onChange={(format) => update({ output: { ...output, format } })}
          />
        </Field>
      )}

      <Field label={fields.fileName} hint={hints.fileName} htmlFor="preset-file-name">
        <TextInput
          id="preset-file-name"
          className="font-mono"
          value={draft.fileName}
          onChange={(event) => update({ fileName: event.target.value })}
        />
      </Field>

      <Field label={fields.folder} hint={draft.folder ? undefined : hints.defaultFolder(defaultFolder)}>
        <div className="flex gap-2">
          <TextInput readOnly className="flex-1" value={draft.folder ?? defaultFolder} title={draft.folder ?? defaultFolder} />
          <Button onClick={() => void chooseFolder()}>{fields.chooseFolder}</Button>
          {draft.folder && (
            <Button variant="ghost" onClick={() => update({ folder: null })}>
              {fields.useDefaultFolder}
            </Button>
          )}
        </div>
      </Field>

      <Field label={fields.embed}>
        <div className="flex flex-wrap gap-x-5 gap-y-2">
          <Checkbox
            label={fields.metadata}
            checked={draft.embedMetadata}
            onChange={(embedMetadata) => update({ embedMetadata })}
          />
          <Checkbox
            label={fields.thumbnail}
            checked={draft.embedThumbnail}
            onChange={(embedThumbnail) => update({ embedThumbnail })}
          />
          <Checkbox
            label={fields.chapters}
            checked={draft.embedChapters}
            onChange={(embedChapters) => update({ embedChapters })}
          />
          <Checkbox
            label={fields.subtitles}
            checked={draft.embedSubtitles && output.kind === "video"}
            disabled={output.kind === "audio"}
            onChange={(embedSubtitles) => update({ embedSubtitles })}
          />
        </div>
      </Field>

      {draft.embedSubtitles && output.kind === "video" && (
        <Field label={fields.subtitleLanguages} hint={hints.subtitleLanguages} htmlFor="preset-subtitle-languages">
          <TextInput
            id="preset-subtitle-languages"
            className="font-mono"
            value={draft.subtitleLanguages}
            onChange={(event) => update({ subtitleLanguages: event.target.value })}
          />
        </Field>
      )}

      <Field label={fields.sponsorBlock}>
        <SegmentedControl<SponsorBlock>
          label={fields.sponsorBlock}
          options={SPONSOR_BLOCK_OPTIONS}
          value={draft.sponsorBlock}
          onChange={(sponsorBlock) => update({ sponsorBlock })}
        />
      </Field>

      <Field
        label={fields.extraArguments}
        hint={hints.extraArguments}
        error={argumentsError}
        htmlFor="preset-extra-arguments"
      >
        <TextInput
          id="preset-extra-arguments"
          className="font-mono"
          value={draft.extraArguments}
          onChange={(event) => update({ extraArguments: event.target.value })}
        />
      </Field>

      <Field label={fields.arguments} hint={hints.arguments}>
        <pre className="rounded-md border border-line bg-input px-3 py-2 font-mono text-xs break-all whitespace-pre-wrap text-fg-secondary select-text">
          {argumentsLine(args.data ?? [])}
        </pre>
      </Field>

      {actionError && <StatusMessage tone="danger" {...actionError} />}
      {/* An argument the core refuses is already said under its field. */}
      {saveError && argumentsError === undefined && <StatusMessage tone="danger" {...saveError} />}
      {saveState !== "idle" && (
        <p className="text-right text-xs text-fg-muted" role="status">
          {saveState === "saving" ? strings.presets.saving : strings.presets.saved}
        </p>
      )}
    </Card>
  )
}

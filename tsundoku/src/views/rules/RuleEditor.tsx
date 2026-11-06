import { useRef } from "react"
import { Plus, X } from "lucide-react"
import { pickFolder } from "../../api"
import type { FolderRule } from "../../bindings/FolderRule"
import type { Preset } from "../../bindings/Preset"
import type { RuleCondition } from "../../bindings/RuleCondition"
import type { RuleField } from "../../bindings/RuleField"
import type { RuleOperator } from "../../bindings/RuleOperator"
import { Button } from "../../components/Button"
import { Card } from "../../components/Card"
import { Field, SelectInput, TextInput } from "../../components/Field"
import { IconButton } from "../../components/IconButton"
import { PlaceholderButtons } from "../../components/PlaceholderButtons"
import { StatusMessage } from "../../components/StatusMessage"
import { SegmentedControl } from "../../components/SegmentedControl"
import type { ErrorDescription } from "../../errors"
import { useSupportedSites } from "../../queries"
import { strings } from "../../strings"
import {
  CATCH_ALL_RULE_ID,
  DOWNLOAD_KINDS,
  FOLDER_PLACEHOLDERS,
  RULE_FIELDS,
  newCondition,
  canPickPreset,
  operatorsFor,
} from "./ruleLabels"

const text = strings.rules

/** A rule's Download Right Away: unset follows the rules below it, then Settings. */
type RightAwayChoice = "unset" | "on" | "off"

const RIGHT_AWAY_CHOICES: readonly { value: RightAwayChoice; label: string }[] = [
  { value: "unset", label: text.leaveAsIs },
  { value: "on", label: text.on },
  { value: "off", label: text.off },
]

/** The site field's suggestions: every supported site. */
const SITES_LIST_ID = "rule-sites"

interface RuleEditorProps {
  draft: FolderRule
  /** The rule's place in the list, from 1. */
  number: number
  presets: readonly Preset[]
  defaultPresetId: string
  downloadFolder: string
  saving: boolean
  error: ErrorDescription | null
  onChange: (draft: FolderRule) => void
  onSave: () => void
  onDiscard: () => void
}

/** One rule's conditions and what it does (its folder, the preset videos start with, Download Right Away), kept as a
 * draft until Save. The catch-all has no conditions, only a folder. */
export function RuleEditor({
  draft,
  number,
  presets,
  defaultPresetId,
  downloadFolder,
  saving,
  error,
  onChange,
  onSave,
  onDiscard,
}: RuleEditorProps) {
  const { data: supportedSites } = useSupportedSites()
  const folderInput = useRef<HTMLInputElement>(null)
  const isCatchAll = draft.id === CATCH_ALL_RULE_ID
  const update = (changes: Partial<FolderRule>) => onChange({ ...draft, ...changes })
  const setCondition = (index: number, condition: RuleCondition) =>
    update({ conditions: draft.conditions.map((existing, at) => (at === index ? condition : existing)) })

  async function chooseFolder() {
    const folder = await pickFolder(text.chooseFolderTitle)
    if (folder) update({ folder })
  }

  return (
    <Card className="flex flex-col gap-4 p-4">
      <p className="font-semibold">{text.number(number)}</p>
      {isCatchAll ? (
        <Field label={text.when}>
          <p className="text-fg-secondary">{text.catchAllWhen}</p>
        </Field>
      ) : (
        <Field label={text.when} hint={text.whenHint}>
          <div className="flex flex-col gap-2">
            {draft.conditions.map((condition, index) => (
              <ConditionRow
                // Conditions have no ID of their own, and their inputs are controlled, so their place will do.
                key={index}
                condition={condition}
                presets={presets}
                defaultPresetId={defaultPresetId}
                onChange={(changed) => setCondition(index, changed)}
                onRemove={() => update({ conditions: draft.conditions.filter((_, at) => at !== index) })}
              />
            ))}
            <div>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => update({ conditions: [...draft.conditions, newCondition("site", defaultPresetId)] })}
              >
                <Plus size={14} />
                {text.addCondition}
              </Button>
            </div>
          </div>
        </Field>
      )}
      <Field
        label={text.saveIn}
        hint={isCatchAll ? text.saveInHint(downloadFolder) : `${text.saveInHint(downloadFolder)} ${text.saveInEmptyHint}`}
        htmlFor="rule-folder"
      >
        <div className="flex flex-col gap-2">
          <div className="flex gap-2">
            <TextInput
              ref={folderInput}
              id="rule-folder"
              className="flex-1 font-mono"
              value={draft.folder}
              onChange={(event) => update({ folder: event.target.value })}
            />
            <Button onClick={() => void chooseFolder()}>{text.chooseFolder}</Button>
          </div>
          <PlaceholderButtons
            input={folderInput}
            value={draft.folder}
            placeholders={FOLDER_PLACEHOLDERS}
            hints={text.placeholderHints}
            onChange={(folder) => update({ folder })}
          />
        </div>
      </Field>
      {!isCatchAll && <RuleActions draft={draft} presets={presets} onChange={update} />}
      {error && <StatusMessage tone="danger" {...error} />}
      <div className="flex justify-end gap-2">
        <Button variant="ghost" disabled={saving} onClick={onDiscard}>
          {text.discard}
        </Button>
        <Button variant="primary" loading={saving} onClick={onSave}>
          {text.save}
        </Button>
      </div>
      <datalist id={SITES_LIST_ID}>
        {supportedSites?.sites.map((site) => (
          <option key={site.name} value={site.name} />
        ))}
      </datalist>
    </Card>
  )
}

interface RuleActionsProps {
  draft: FolderRule
  presets: readonly Preset[]
  onChange: (changes: Partial<FolderRule>) => void
}

/** What a rule does besides picking a folder: the preset matching videos start with, and Download Right Away. A rule
 * that checks the kind or the preset can't pick the preset, so Start As is off for it, or says why it can't stay. */
function RuleActions({ draft, presets, onChange }: RuleActionsProps) {
  const presetAllowed = canPickPreset(draft.conditions)
  const presetMissing = draft.presetId !== null && !presets.some((preset) => preset.id === draft.presetId)
  const rightAway: RightAwayChoice =
    draft.downloadRightAway === null ? "unset" : draft.downloadRightAway ? "on" : "off"
  return (
    <>
      <Field
        label={text.startAs}
        hint={presetAllowed ? text.startAsHint : text.startAsUnavailable}
        error={!presetAllowed && draft.presetId !== null ? text.startAsUnavailable : undefined}
        htmlFor="rule-preset"
      >
        <SelectInput
          id="rule-preset"
          className="self-start"
          value={draft.presetId ?? ""}
          disabled={!presetAllowed && draft.presetId === null}
          onChange={(event) => onChange({ presetId: event.target.value || null })}
        >
          <option value="">{text.leaveAsIs}</option>
          {presets.map((preset) => (
            <option key={preset.id} value={preset.id}>
              {preset.name}
            </option>
          ))}
          {presetMissing && <option value={draft.presetId ?? ""}>{text.deletedPreset}</option>}
        </SelectInput>
      </Field>
      <Field label={text.downloadRightAway} hint={text.downloadRightAwayHint}>
        <SegmentedControl
          label={text.downloadRightAway}
          options={RIGHT_AWAY_CHOICES}
          value={rightAway}
          onChange={(choice) => onChange({ downloadRightAway: choice === "unset" ? null : choice === "on" })}
        />
      </Field>
    </>
  )
}

interface ConditionRowProps {
  condition: RuleCondition
  presets: readonly Preset[]
  defaultPresetId: string
  onChange: (condition: RuleCondition) => void
  onRemove: () => void
}

/** What to check, how to compare it, and what to compare it to. Picking another field starts the condition over. */
function ConditionRow({ condition, presets, defaultPresetId, onChange, onRemove }: ConditionRowProps) {
  const { field, operator, value } = condition
  const setValue = (changed: string) => onChange({ ...condition, value: changed })

  return (
    <div className="grid grid-cols-[8rem_9.5rem_minmax(0,1fr)_auto] items-center gap-2">
      <SelectInput
        aria-label={text.field}
        title={text.field}
        value={field}
        onChange={(event) => onChange(newCondition(event.target.value as RuleField, defaultPresetId))}
      >
        {RULE_FIELDS.map((option) => (
          <option key={option} value={option}>
            {text.fields[option]}
          </option>
        ))}
      </SelectInput>
      <SelectInput
        aria-label={text.operator}
        title={text.operator}
        value={operator}
        onChange={(event) => onChange({ ...condition, operator: event.target.value as RuleOperator })}
      >
        {operatorsFor(field).map((option) => (
          <option key={option} value={option}>
            {text.operators[option]}
          </option>
        ))}
      </SelectInput>
      {field === "kind" ? (
        <SelectInput aria-label={text.value} value={value} onChange={(event) => setValue(event.target.value)}>
          {DOWNLOAD_KINDS.map((kind) => (
            <option key={kind} value={kind}>
              {text.kinds[kind]}
            </option>
          ))}
        </SelectInput>
      ) : field === "preset" ? (
        <SelectInput aria-label={text.value} value={value} onChange={(event) => setValue(event.target.value)}>
          {presets.map((preset) => (
            <option key={preset.id} value={preset.id}>
              {preset.name}
            </option>
          ))}
        </SelectInput>
      ) : (
        <TextInput
          aria-label={text.value}
          list={field === "site" ? SITES_LIST_ID : undefined}
          placeholder={text.placeholders[field]}
          value={value}
          onChange={(event) => setValue(event.target.value)}
        />
      )}
      <IconButton label={text.removeCondition} icon={X} onClick={onRemove} />
    </div>
  )
}

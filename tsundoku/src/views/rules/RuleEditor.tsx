import { useRef } from "react"
import { CaseSensitive, Plus, X } from "lucide-react"
import { pickFolder } from "../../api"
import { UNPACK_PLACES } from "../../archives"
import type { ConditionJoin } from "../../bindings/ConditionJoin"
import type { FolderRule } from "../../bindings/FolderRule"
import type { MissingValues } from "../../bindings/MissingValues"
import type { Preset } from "../../bindings/Preset"
import type { RuleCondition } from "../../bindings/RuleCondition"
import type { RuleField } from "../../bindings/RuleField"
import type { RuleOperator } from "../../bindings/RuleOperator"
import type { RuleUnpacking } from "../../bindings/RuleUnpacking"
import { Button } from "../../components/Button"
import { Card } from "../../components/Card"
import { Field, SelectInput, TextInput } from "../../components/Field"
import { IconButton } from "../../components/IconButton"
import { PlaceholderButtons } from "../../components/PlaceholderButtons"
import { DEFAULT_POST_FILE_NAME, PostNameInput } from "../../components/PostNameInput"
import { StatusMessage } from "../../components/StatusMessage"
import { SegmentedControl } from "../../components/SegmentedControl"
import { Tooltip } from "../../components/Tooltip"
import type { ErrorDescription } from "../../errors"
import { useDownloadOptions, useSupportedSites } from "../../queries"
import { strings } from "../../strings"
import {
  CATCH_ALL_RULE_ID,
  DOWNLOAD_KINDS,
  FOLDER_PLACEHOLDERS,
  RULE_FIELDS,
  newCondition,
  canPickPreset,
  capturedGroups,
  isPattern,
  mindsCase,
  operatorsFor,
} from "./ruleLabels"

const text = strings.rules

/** A rule's Download Right Away: unset follows the rules above it, then Settings. */
type RightAwayChoice = "unset" | "on" | "off"

const RIGHT_AWAY_CHOICES: readonly { value: RightAwayChoice; label: string }[] = [
  { value: "unset", label: text.leaveAsIs },
  { value: "on", label: text.on },
  { value: "off", label: text.off },
]

const MISSING_VALUE_CHOICES: readonly { value: MissingValues; label: string }[] = [
  { value: "standIn", label: text.missingValueChoices.standIn },
  { value: "leaveOut", label: text.missingValueChoices.leaveOut },
]

/** What a rule can say of matching downloads' archives, after Leave as Is: left packed, or unpacked to a place. */
const UNPACKING_CHOICES: readonly { value: RuleUnpacking; label: string }[] = [
  { value: "off", label: text.dontUnpack },
  ...UNPACK_PLACES.map((place) => ({ value: place, label: strings.settings.unpackPlaces[place] })),
]

/** The site field's suggestions: every supported site. */
const SITES_LIST_ID = "rule-sites"

interface RuleEditorProps {
  draft: FolderRule
  /** The rule's place in the list, from 1. */
  number: number
  presets: readonly Preset[]
  defaultPresetId: string
  saving: boolean
  error: ErrorDescription | null
  onChange: (draft: FolderRule) => void
  onSave: () => void
  onDiscard: () => void
}

/** One rule's conditions and what it does (its folder, the preset videos start with, Download Right Away, unpacking),
 * kept as a draft until Save. The catch-all has no conditions, only a folder. */
export function RuleEditor({
  draft,
  number,
  presets,
  defaultPresetId,
  saving,
  error,
  onChange,
  onSave,
  onDiscard,
}: RuleEditorProps) {
  const { data: supportedSites } = useSupportedSites()
  const folderInput = useRef<HTMLInputElement>(null)
  const { data: downloadOptions } = useDownloadOptions()
  const settingsPostName = downloadOptions?.postFileName.trim() || DEFAULT_POST_FILE_NAME
  const isCatchAll = draft.id === CATCH_ALL_RULE_ID
  const { placeholders, hints: placeholderHints } = folderPlaceholders(draft.conditions)
  const capturedPlaceholders = placeholders.filter((placeholder) => /^\d$/.test(placeholder))
  const update = (changes: Partial<FolderRule>) => onChange({ ...draft, ...changes })
  const setCondition = (index: number, condition: RuleCondition) =>
    update({ conditions: draft.conditions.map((existing, at) => (at === index ? condition : existing)) })

  async function chooseFolder() {
    const folder = await pickFolder(text.chooseFolderTitle)
    if (folder) update({ folder })
  }

  return (
    <Card className="flex flex-col gap-4 p-4">
      <p className="font-semibold">{draft.name.trim() || text.number(number)}</p>
      <Field label={text.name} htmlFor="rule-name">
        <TextInput
          id="rule-name"
          className="max-w-sm"
          placeholder={text.number(number)}
          value={draft.name}
          onChange={(event) => update({ name: event.target.value })}
        />
      </Field>
      {isCatchAll ? (
        <Field label={text.when}>
          <p className="text-fg-secondary">{text.catchAllWhen}</p>
        </Field>
      ) : (
        <Field label={text.when}>
          <div className="flex flex-col gap-2">
            {draft.conditions.map((condition, index) => (
              <ConditionRow
                // Conditions have no ID of their own, and their inputs are controlled, so their place will do.
                key={index}
                condition={condition}
                first={index === 0}
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
      <Field label={text.saveIn} htmlFor="rule-folder">
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
            placeholders={placeholders}
            hints={placeholderHints}
            onChange={(folder) => update({ folder })}
          />
        </div>
      </Field>
      {!isCatchAll && (
        <Field label={text.fileName} htmlFor="rule-file-name">
          <PostNameInput
            id="rule-file-name"
            value={draft.fileName}
            onChange={(fileName) => update({ fileName })}
            emptyName={settingsPostName}
            placeholder={text.usingDefault}
            extraPlaceholders={capturedPlaceholders}
            extraHints={placeholderHints}
          />
        </Field>
      )}
      <Field label={text.missingValues}>
        <SegmentedControl
          label={text.missingValues}
          options={MISSING_VALUE_CHOICES}
          value={draft.missingValues}
          onChange={(missingValues) => update({ missingValues })}
        />
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

/** At the start of a condition's row, after the first: AND or OR, how it joins the one above, which a click switches. */
function JoinButton({ join, onToggle }: { join: ConditionJoin; onToggle: () => void }) {
  return (
    <Tooltip label={text.joinHint[join]}>
      <Button size="sm" aria-label={text.joinHint[join]} className="w-full justify-center" onClick={onToggle}>
        {text.join[join]}
      </Button>
    </Tooltip>
  )
}

interface RuleActionsProps {
  draft: FolderRule
  presets: readonly Preset[]
  onChange: (changes: Partial<FolderRule>) => void
}

/** What a rule does besides picking a folder: the preset matching videos start with, Download Right Away, and whether
 * and where archives are unpacked. A rule that checks the kind or the preset can't pick the preset, so Start As is off
 * for it, or says why it can't stay. */
function RuleActions({ draft, presets, onChange }: RuleActionsProps) {
  const presetAllowed = canPickPreset(draft.conditions)
  const presetMissing = draft.presetId !== null && !presets.some((preset) => preset.id === draft.presetId)
  const rightAway: RightAwayChoice = draft.downloadRightAway === null ? "unset" : draft.downloadRightAway ? "on" : "off"
  return (
    <>
      <Field
        label={text.startAs}
        hint={presetAllowed ? undefined : text.startAsUnavailable}
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
      <Field label={text.downloadRightAway}>
        <SegmentedControl
          label={text.downloadRightAway}
          options={RIGHT_AWAY_CHOICES}
          value={rightAway}
          onChange={(choice) => onChange({ downloadRightAway: choice === "unset" ? null : choice === "on" })}
        />
      </Field>
      <Field
        label={text.unpackArchives}
        hint={draft.unpack && draft.unpack !== "off" ? strings.settings.unpackPlaceHints[draft.unpack] : undefined}
        htmlFor="rule-unpack"
      >
        <SelectInput
          id="rule-unpack"
          className="self-start"
          value={draft.unpack ?? ""}
          onChange={(event) => onChange({ unpack: (event.target.value || null) as RuleUnpacking | null })}
        >
          <option value="">{text.leaveAsIs}</option>
          {UNPACKING_CHOICES.map((choice) => (
            <option key={choice.value} value={choice.value}>
              {choice.label}
            </option>
          ))}
        </SelectInput>
      </Field>
    </>
  )
}

interface ConditionRowProps {
  condition: RuleCondition
  /** The first condition has no AND or OR before it. */
  first: boolean
  presets: readonly Preset[]
  defaultPresetId: string
  onChange: (condition: RuleCondition) => void
  onRemove: () => void
}

/** The folder's placeholders, then `{1}`, `{2}`… for the groups the rule's pattern captures, each with its hover text. */
function folderPlaceholders(conditions: readonly RuleCondition[]): {
  placeholders: string[]
  hints: Record<string, string>
} {
  const groups = Array.from({ length: capturedGroups(conditions) }, (_, index) => String(index + 1))
  return {
    placeholders: [...FOLDER_PLACEHOLDERS, ...groups],
    hints: {
      ...text.placeholderHints,
      ...Object.fromEntries(groups.map((group) => [group, text.capturedHint(group)])),
    },
  }
}

/** What to check, how to compare it, and what to compare it to. Picking another field starts the condition over. */
function ConditionRow({ condition, first, presets, defaultPresetId, onChange, onRemove }: ConditionRowProps) {
  const { field, operator, value } = condition
  const setValue = (changed: string) => onChange({ ...condition, value: changed })

  return (
    <div className="grid grid-cols-[3.5rem_8rem_9.5rem_minmax(0,1fr)_auto_auto] items-center gap-2">
      {first ? (
        // Keeps the first condition lined up with the ones after it.
        <span />
      ) : (
        <JoinButton
          join={condition.join}
          onToggle={() => onChange({ ...condition, join: condition.join === "all" ? "any" : "all" })}
        />
      )}
      <SelectInput
        aria-label={text.field}
        title={text.field}
        value={field}
        onChange={(event) =>
          onChange({ ...newCondition(event.target.value as RuleField, defaultPresetId), join: condition.join })
        }
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
          className={isPattern(operator) ? "font-mono" : undefined}
          list={field === "site" && !isPattern(operator) ? SITES_LIST_ID : undefined}
          placeholder={isPattern(operator) ? text.placeholders.pattern : text.placeholders[field]}
          value={value}
          onChange={(event) => setValue(event.target.value)}
        />
      )}
      {field === "kind" || field === "preset" ? (
        // Values picked from a list have no case to match.
        <span className="size-8" />
      ) : (
        <IconButton
          label={mindsCase(condition) ? text.ignoreCase : text.matchCase}
          icon={CaseSensitive}
          variant={mindsCase(condition) ? "secondary" : "ghost"}
          className={mindsCase(condition) ? "text-accent" : undefined}
          aria-pressed={mindsCase(condition)}
          onClick={() => onChange({ ...condition, caseSensitive: !condition.caseSensitive })}
        />
      )}
      <IconButton label={text.removeCondition} icon={X} onClick={onRemove} />
    </div>
  )
}

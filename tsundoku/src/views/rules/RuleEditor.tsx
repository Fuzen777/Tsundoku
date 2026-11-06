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
import { StatusMessage } from "../../components/StatusMessage"
import type { ErrorDescription } from "../../errors"
import { useSupportedSites } from "../../queries"
import { strings } from "../../strings"
import {
  CATCH_ALL_RULE_ID,
  DOWNLOAD_KINDS,
  FOLDER_PLACEHOLDERS,
  RULE_FIELDS,
  insertPlaceholder,
  newCondition,
  operatorsFor,
  type FolderPlaceholder,
} from "./ruleLabels"

const text = strings.rules

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

/** One rule's conditions and folder, kept as a draft until Save. The catch-all has no conditions, only a folder. */
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

  /** Puts `{site}` or `{uploader}` where the cursor is in the folder field, over any selected text. */
  function insert(placeholder: FolderPlaceholder) {
    const input = folderInput.current
    const start = input?.selectionStart ?? draft.folder.length
    const end = input?.selectionEnd ?? start
    const { folder, cursor } = insertPlaceholder(draft.folder, placeholder, start, end)
    update({ folder })
    // Once the field shows the new text, since setting it moves the cursor to its end.
    requestAnimationFrame(() => {
      input?.focus()
      input?.setSelectionRange(cursor, cursor)
    })
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
      <Field label={text.saveIn} hint={text.saveInHint(downloadFolder)} htmlFor="rule-folder">
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
          <div className="flex flex-wrap items-center gap-1.5">
            <span className="text-xs text-fg-muted">{text.insert}</span>
            {FOLDER_PLACEHOLDERS.map((placeholder) => (
              <Button
                key={placeholder}
                size="sm"
                className="font-mono"
                title={text.placeholderHints[placeholder]}
                // Keeps the cursor in the folder field, so the placeholder goes where it is.
                onMouseDown={(event) => event.preventDefault()}
                onClick={() => insert(placeholder)}
              >
                {`{${placeholder}}`}
              </Button>
            ))}
          </div>
        </div>
      </Field>
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

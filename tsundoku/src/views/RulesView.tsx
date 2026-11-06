import { useState } from "react"
import { useQueryClient } from "@tanstack/react-query"
import { ArrowDown, ArrowUp, FolderTree, Plus } from "lucide-react"
import { confirmWithUser, queryKeys, saveFolderRules } from "../api"
import type { FolderRule } from "../bindings/FolderRule"
import type { Preset } from "../bindings/Preset"
import { Button } from "../components/Button"
import { Card } from "../components/Card"
import { EmptyState } from "../components/EmptyState"
import { Menu } from "../components/Menu"
import { Page } from "../components/Page"
import { StatusMessage } from "../components/StatusMessage"
import { classes } from "../components/styles"
import { describeRuleError, type ErrorDescription } from "../errors"
import { useAppActions } from "../navigation"
import { useDefaultDownloadFolder, useDownloadOptions, useFolderRules, usePresets } from "../queries"
import { strings } from "../strings"
import { RuleEditor } from "./rules/RuleEditor"
import { describeConditions, newRule, shownFolder } from "./rules/ruleLabels"

const text = strings.rules

/** The rule being edited: its place in the list (the end for a new one), the draft, and the rule as it was. */
interface Editing {
  index: number
  draft: FolderRule
  original: FolderRule
}

/** The folder rules, in the order they're tried: each as a sentence with its folder. One rule is edited at a time,
 * as a draft until Save; moving, turning off and deleting save at once. */
export function RulesView() {
  const queryClient = useQueryClient()
  const { navigate } = useAppActions()
  const { data: rules } = useFolderRules()
  const { data: presetsOverview } = usePresets()
  const { data: options } = useDownloadOptions()
  const { data: defaultFolder } = useDefaultDownloadFolder()
  const [editing, setEditing] = useState<Editing | null>(null)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<ErrorDescription | null>(null)

  if (!rules) return <Page title={strings.nav.rules} />
  const presets = presetsOverview?.presets ?? []
  const downloadFolder = options?.folder ?? defaultFolder ?? ""
  const busy = saving || editing !== null

  async function store(list: FolderRule[]): Promise<boolean> {
    setError(null)
    setSaving(true)
    try {
      queryClient.setQueryData(queryKeys.folderRules, await saveFolderRules(list))
      // The preview card says which rule picks a download's folder.
      void queryClient.invalidateQueries({ queryKey: queryKeys.downloadDestination })
      return true
    } catch (caught) {
      setError(describeRuleError(caught))
      return false
    } finally {
      setSaving(false)
    }
  }

  function startEditing(index: number, rule: FolderRule) {
    setError(null)
    setEditing({ index, draft: rule, original: rule })
  }

  function move(index: number, by: -1 | 1) {
    if (!rules) return
    const list = [...rules]
    const [rule] = list.splice(index, 1)
    list.splice(index + by, 0, rule)
    void store(list)
  }

  function toggle(index: number) {
    if (!rules) return
    void store(rules.map((rule, at) => (at === index ? { ...rule, enabled: !rule.enabled } : rule)))
  }

  async function remove(index: number) {
    const confirmed = await confirmWithUser({
      title: text.deleteTitle,
      message: text.deleteMessage(index + 1),
      confirmLabel: text.delete,
      cancelLabel: strings.dialogs.cancel,
    })
    if (confirmed && rules) await store(rules.filter((_, at) => at !== index))
  }

  async function saveDraft() {
    if (!editing || !rules) return
    const list =
      editing.index < rules.length
        ? rules.map((rule, at) => (at === editing.index ? editing.draft : rule))
        : [...rules, editing.draft]
    if (await store(list)) setEditing(null)
  }

  const editor = (index: number) =>
    editing && (
      <RuleEditor
        key={editing.original.id || "new"}
        draft={editing.draft}
        number={index + 1}
        presets={presets}
        defaultPresetId={presetsOverview?.defaultId ?? ""}
        downloadFolder={downloadFolder}
        saving={saving}
        error={error}
        onChange={(draft) => setEditing({ ...editing, draft })}
        onSave={() => void saveDraft()}
        onDiscard={() => {
          setError(null)
          setEditing(null)
        }}
      />
    )

  return (
    <Page
      title={strings.nav.rules}
      actions={
        <Button size="sm" disabled={busy} onClick={() => startEditing(rules.length, newRule())}>
          <Plus size={14} />
          {text.addRule}
        </Button>
      }
    >
      <div className="flex max-w-3xl flex-col gap-3">
        <p className="text-fg-secondary">{text.intro}</p>
        {error && !editing && <StatusMessage tone="danger" {...error} />}
        {rules.map((rule, index) =>
          editing?.index === index ? (
            editor(index)
          ) : (
            <RuleRow
              key={rule.id}
              rule={rule}
              number={index + 1}
              presets={presets}
              folder={shownFolder(rule.folder, downloadFolder)}
              disabled={busy}
              isFirst={index === 0}
              isLast={index === rules.length - 1}
              onEdit={() => startEditing(index, rule)}
              onMove={(by) => move(index, by)}
              onToggle={() => toggle(index)}
              onDelete={() => void remove(index)}
            />
          ),
        )}
        {editing?.index === rules.length && editor(rules.length)}
        {rules.length === 0 && !editing && (
          <div className="py-8">
            <EmptyState icon={FolderTree} title={text.emptyTitle} body={text.emptyBody} />
          </div>
        )}
        {downloadFolder && (
          <div className="flex items-center gap-3">
            <p className="min-w-0 flex-1 truncate text-xs text-fg-muted" title={downloadFolder}>
              {text.otherwise(downloadFolder)}
            </p>
            <Button size="sm" variant="ghost" onClick={() => navigate("settings")}>
              {text.changeDownloadFolder}
            </Button>
          </div>
        )}
      </div>
    </Page>
  )
}

interface RuleRowProps {
  rule: FolderRule
  number: number
  presets: readonly Preset[]
  /** The rule's folder as it will be used. */
  folder: string
  /** While another rule is edited or the list is saving. */
  disabled: boolean
  isFirst: boolean
  isLast: boolean
  onEdit: () => void
  onMove: (by: -1 | 1) => void
  onToggle: () => void
  onDelete: () => void
}

/** A rule as a sentence and its folder, with its number, moving, Edit and a menu. */
function RuleRow({ rule, number, presets, folder, disabled, isFirst, isLast, onEdit, onMove, onToggle, onDelete }: RuleRowProps) {
  const sentence = describeConditions(rule.conditions, presets)
  return (
    <Card className="flex items-center gap-3 px-4 py-3">
      <span
        className={classes(
          "flex size-6 shrink-0 items-center justify-center rounded-full bg-input text-xs font-semibold tabular-nums",
          !rule.enabled && "text-fg-muted",
        )}
        title={text.number(number)}
      >
        {number}
      </span>
      <div className={classes("flex min-w-0 flex-1 flex-col gap-0.5", !rule.enabled && "opacity-60")}>
        <p className="break-words">{sentence}</p>
        <p className="truncate font-mono text-xs text-fg-secondary" title={folder}>
          {folder}
        </p>
      </div>
      {!rule.enabled && (
        <span className="shrink-0 rounded-full border border-line px-1.5 text-xs text-fg-secondary">{text.off}</span>
      )}
      <div className="flex shrink-0 items-center gap-1">
        <Button
          size="icon"
          variant="ghost"
          aria-label={text.moveUp}
          title={text.moveUp}
          disabled={disabled || isFirst}
          onClick={() => onMove(-1)}
        >
          <ArrowUp size={16} />
        </Button>
        <Button
          size="icon"
          variant="ghost"
          aria-label={text.moveDown}
          title={text.moveDown}
          disabled={disabled || isLast}
          onClick={() => onMove(1)}
        >
          <ArrowDown size={16} />
        </Button>
        <Button size="sm" disabled={disabled} onClick={onEdit}>
          {text.edit}
        </Button>
        {disabled ? (
          // Keeps the row's layout while the menu is hidden.
          <span className="size-8" />
        ) : (
          <Menu
            label={text.moreActions}
            items={[
              { label: rule.enabled ? text.turnOff : text.turnOn, onSelect: onToggle },
              { label: text.delete, onSelect: onDelete, danger: true },
            ]}
          />
        )}
      </div>
    </Card>
  )
}

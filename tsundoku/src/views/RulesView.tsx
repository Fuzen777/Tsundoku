import { useState, type ReactNode } from "react"
import { useQueryClient } from "@tanstack/react-query"
import { ArrowDown, ArrowUp, Copy, FolderTree, Pencil, Plus } from "lucide-react"
import { confirmWithUser, queryKeys, saveFolderRules } from "../api"
import type { FolderRule } from "../bindings/FolderRule"
import type { Preset } from "../bindings/Preset"
import { Button } from "../components/Button"
import { Card } from "../components/Card"
import { EmptyState } from "../components/EmptyState"
import { IconButton } from "../components/IconButton"
import { Menu, type MenuItem } from "../components/Menu"
import { Page } from "../components/Page"
import { StatusMessage } from "../components/StatusMessage"
import { classes } from "../components/styles"
import { describeRuleError, type ErrorDescription } from "../errors"
import { joinDetails } from "../format"
import { useAppActions } from "../navigation"
import { useDefaultDownloadFolder, useDownloadOptions, useFolderRules, usePresets } from "../queries"
import { strings } from "../strings"
import { useOpenDownloadFolder } from "../useOpenDownloadFolder"
import { RuleEditor } from "./rules/RuleEditor"
import { RuleTester } from "./rules/RuleTester"
import {
  CATCH_ALL_RULE_ID,
  copyOfRule,
  describeActions,
  describeConditions,
  newRule,
  shownFolder,
} from "./rules/ruleLabels"

const text = strings.rules

/** The rule being edited: its place in the list (for a new one, where it will go: just above the catch-all, or just
 * below the rule it duplicates), the draft, and whether it's new. */
interface Editing {
  index: number
  draft: FolderRule
  isNew: boolean
}

/** The folder rules, in the order they're tried: each as a sentence with its folder. One rule is edited at a time,
 * as a draft until Save; moving, turning off and deleting save at once. Duplicate opens a copy as a new rule just
 * below the original, saved like any new one. The catch-all stays last: it can't be moved, duplicated or deleted,
 * and new rules go above it. */
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
  const openFolder = useOpenDownloadFolder()

  if (!rules) return <Page title={strings.nav.rules} />
  const presets = presetsOverview?.presets ?? []
  const downloadFolder = options?.folder ?? defaultFolder ?? ""
  const busy = saving || editing !== null
  // The core keeps the catch-all last, so rules above it are the ones that move.
  const catchAllIndex = rules.findIndex((rule) => rule.id === CATCH_ALL_RULE_ID)
  const movableCount = catchAllIndex === -1 ? rules.length : catchAllIndex
  // A new rule shows where it will go, so the rules after it are numbered one further.
  const numberOf = (index: number) => index + 1 + (editing?.isNew && index >= editing.index ? 1 : 0)

  async function store(list: FolderRule[]): Promise<boolean> {
    setError(null)
    setSaving(true)
    try {
      queryClient.setQueryData(queryKeys.folderRules, await saveFolderRules(list))
      // The preview card and Pending say which rule picks a download's folder and preset.
      void queryClient.invalidateQueries({ queryKey: queryKeys.downloadDestination })
      void queryClient.invalidateQueries({ queryKey: queryKeys.rulePreset })
      void queryClient.invalidateQueries({ queryKey: queryKeys.ruleRightAway })
      return true
    } catch (caught) {
      setError(describeRuleError(caught))
      return false
    } finally {
      setSaving(false)
    }
  }

  function startEditing(index: number, rule: FolderRule, isNew: boolean) {
    setError(null)
    setEditing({ index, draft: rule, isNew })
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
    const list = editing.isNew
      ? [...rules.slice(0, editing.index), editing.draft, ...rules.slice(editing.index)]
      : rules.map((rule, at) => (at === editing.index ? editing.draft : rule))
    if (await store(list)) setEditing(null)
  }

  const editor = (current: Editing) => (
    <RuleEditor
      key={current.isNew ? "new" : current.draft.id}
      draft={current.draft}
      number={current.index + 1}
      presets={presets}
      defaultPresetId={presetsOverview?.defaultId ?? ""}
      downloadFolder={downloadFolder}
      saving={saving}
      error={error}
      onChange={(draft) => setEditing({ ...current, draft })}
      onSave={() => void saveDraft()}
      onDiscard={() => {
        setError(null)
        setEditing(null)
      }}
    />
  )

  const rows: ReactNode[] = rules.map((rule, index) =>
    editing && !editing.isNew && editing.index === index ? (
      editor(editing)
    ) : (
      <RuleRow
        key={rule.id}
        rule={rule}
        number={numberOf(index)}
        presets={presets}
        folder={rule.folder ? shownFolder(rule.folder, downloadFolder) : null}
        disabled={busy}
        canMoveUp={index > 0 && index < movableCount}
        canMoveDown={index < movableCount - 1}
        onEdit={() => startEditing(index, rule, false)}
        onDuplicate={() => startEditing(index + 1, copyOfRule(rule), true)}
        onMove={(by) => move(index, by)}
        onToggle={() => toggle(index)}
        onDelete={() => void remove(index)}
      />
    ),
  )
  const shownRows = editing?.isNew
    ? [...rows.slice(0, editing.index), editor(editing), ...rows.slice(editing.index)]
    : rows

  return (
    <Page
      title={strings.nav.rules}
      actions={
        <Button size="sm" disabled={busy} onClick={() => startEditing(movableCount, newRule(), true)}>
          <Plus size={14} />
          {text.addRule}
        </Button>
      }
    >
      <div className="flex max-w-3xl flex-col gap-3">
        <p className="text-fg-secondary">{text.intro}</p>
        <RuleTester />
        {error && !editing && <StatusMessage tone="danger" {...error} />}
        {shownRows}
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
            <Button size="sm" variant="ghost" onClick={() => void openFolder.open()}>
              {strings.settings.openDownloadFolder}
            </Button>
            <Button size="sm" variant="ghost" onClick={() => navigate("settings")}>
              {text.changeDownloadFolder}
            </Button>
          </div>
        )}
        {openFolder.error !== null && (
          <StatusMessage tone="danger" message={strings.settings.downloadFolderOpenFailed} detail={openFolder.error} />
        )}
      </div>
    </Page>
  )
}

interface RuleRowProps {
  rule: FolderRule
  number: number
  presets: readonly Preset[]
  /** The rule's folder as it will be used, if it picks one. */
  folder: string | null
  /** While another rule is edited or the list is saving. */
  disabled: boolean
  canMoveUp: boolean
  canMoveDown: boolean
  onEdit: () => void
  onDuplicate: () => void
  onMove: (by: -1 | 1) => void
  onToggle: () => void
  onDelete: () => void
}

/** A rule as a sentence, its folder and what else it does, with its number, moving, Edit, Duplicate and a menu. The
 * catch-all says it's always last, and can't be duplicated or deleted. */
function RuleRow({
  rule,
  number,
  presets,
  folder,
  disabled,
  canMoveUp,
  canMoveDown,
  onEdit,
  onDuplicate,
  onMove,
  onToggle,
  onDelete,
}: RuleRowProps) {
  const isCatchAll = rule.id === CATCH_ALL_RULE_ID
  const sentence = isCatchAll ? text.sentence.everythingElse : describeConditions(rule.conditions, presets)
  const actions = describeActions(rule, presets)
  const toggleItem: MenuItem = { label: rule.enabled ? text.turnOff : text.turnOn, onSelect: onToggle }
  const menuItems: MenuItem[] = isCatchAll
    ? [toggleItem]
    : [toggleItem, { label: text.delete, onSelect: onDelete, danger: true }]
  return (
    <Card id={`rule-${number}`} className="flex scroll-mt-4 items-center gap-3 px-4 py-3">
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
        {folder && (
          <p className="truncate font-mono text-xs text-fg-secondary" title={folder}>
            {folder}
          </p>
        )}
        {actions.length > 0 && <p className="text-xs text-fg-secondary">{joinDetails(actions)}</p>}
      </div>
      {isCatchAll && (
        <span
          className="shrink-0 rounded-full border border-line px-1.5 text-xs text-fg-secondary"
          title={text.alwaysLastHint}
        >
          {text.alwaysLast}
        </span>
      )}
      {!rule.enabled && (
        <span className="shrink-0 rounded-full border border-line px-1.5 text-xs text-fg-secondary">{text.off}</span>
      )}
      <div className="flex shrink-0 items-center gap-1">
        <IconButton label={text.moveUp} icon={ArrowUp} disabled={disabled || !canMoveUp} onClick={() => onMove(-1)} />
        <IconButton
          label={text.moveDown}
          icon={ArrowDown}
          disabled={disabled || !canMoveDown}
          onClick={() => onMove(1)}
        />
        <IconButton label={text.edit} icon={Pencil} disabled={disabled} onClick={onEdit} />
        {isCatchAll ? (
          // Keeps the buttons lined up with the other rows'.
          <span className="size-8" />
        ) : (
          <IconButton label={text.duplicate} icon={Copy} disabled={disabled} onClick={onDuplicate} />
        )}
        {disabled ? (
          // Keeps the row's layout while the menu is hidden.
          <span className="size-8" />
        ) : (
          <Menu label={text.moreActions} items={menuItems} />
        )}
      </div>
    </Card>
  )
}

import { useState, type DragEvent, type ReactNode } from "react"
import { useQueryClient } from "@tanstack/react-query"
import { ArrowDown, ArrowUp, Copy, FolderTree, GripVertical, Pencil, Plus } from "lucide-react"
import { confirmWithUser, queryKeys, saveFolderRules } from "../api"
import { useChapters, type Chapter } from "../chapters"
import type { FolderRule } from "../bindings/FolderRule"
import type { Preset } from "../bindings/Preset"
import { Button } from "../components/Button"
import { Card } from "../components/Card"
import { EmptyState } from "../components/EmptyState"
import { HelpMenu, type HelpTopic } from "../components/HelpMenu"
import { IconButton } from "../components/IconButton"
import { Menu, type MenuItem } from "../components/Menu"
import { Page } from "../components/Page"
import { Section } from "../components/Section"
import { StatusMessage } from "../components/StatusMessage"
import { Tooltip } from "../components/Tooltip"
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
  RULES_SECTION_IDS,
  copyOfRule,
  describeActions,
  describeConditions,
  describeManualConditions,
  movedRule,
  newRule,
  ruleLabel,
  shownFolder,
} from "./rules/ruleLabels"

const text = strings.rules

/** The type a dragged rule carries, so the window's drop of links ignores it (it takes text, links and files). */
const RULE_DRAG_TYPE = "application/x-tsundoku-rule"

/** The rule being edited: its place in the list (for a new one, where it will go: at the bottom, or just below the
 * rule it duplicates), the draft, and whether it's new. */
interface Editing {
  index: number
  draft: FolderRule
  isNew: boolean
}

/** The folder rules, in two lists: the automatic ones, where the last that matches decides, then the manual ones, only
 * ever applied by hand (Apply a Rule…). Each is a sentence with its folder. One rule is edited at a time, as a draft
 * until Save; moving (with its arrows or by dragging its grip, within its list), turning off, moving to the other list
 * and deleting save at once. Duplicate opens a copy as a new rule just below the original, saved like any new one. The
 * catch-all stays first, so every other rule overrides it: it can't be moved, duplicated or deleted, and new rules go
 * at the bottom of their list. The core keeps the manual rules after the others, so each list is a run of the rules. The
 * two lists are the view's chapters in the sidebar, with how many rules each holds. */
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
  // While a rule is dragged: its place, and where it would go (before the rule at `before`, or at the end).
  const [dragged, setDragged] = useState<number | null>(null)
  const [dropBefore, setDropBefore] = useState<number | null>(null)
  const openFolder = useOpenDownloadFolder()
  const { scrollRef, markShown } = useChapters({ view: "rules", chapters: rulesChapters(rules) })

  if (!rules) return <Page title={strings.nav.rules} scrollRef={scrollRef} onScroll={markShown} />
  const presets = presetsOverview?.presets ?? []
  const downloadFolder = options?.folder ?? defaultFolder ?? ""
  const busy = saving || editing !== null
  // The core keeps the catch-all first, so rules below it are the ones that move, and the manual rules last.
  const firstMovable = rules[0]?.id === CATCH_ALL_RULE_ID ? 1 : 0
  const firstManual = rules.findIndex((rule) => rule.manual)
  const manualStart = firstManual < 0 ? rules.length : firstManual
  // Where the list a rule is in starts (past the catch-all) and ends (past its last rule): a rule moves within its list.
  const listStart = (index: number) => (index < manualStart ? firstMovable : manualStart)
  const listEnd = (index: number) => (index < manualStart ? manualStart : rules.length)
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
      void queryClient.invalidateQueries({ queryKey: queryKeys.ruleUnpacking })
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

  function moveBefore(from: number, before: number) {
    const list = rules && movedRule(rules, from, before)
    if (list) void store(list)
  }

  function endDrag() {
    setDragged(null)
    setDropBefore(null)
  }

  /** Over a rule's row: the rule dragged would go before it, or after it past its middle, within the dragged rule's own
   * list, never above the catch-all. */
  function dragOver(event: DragEvent<HTMLDivElement>, index: number) {
    if (dragged === null) return
    event.preventDefault()
    event.dataTransfer.dropEffect = "move"
    const box = event.currentTarget.getBoundingClientRect()
    const before = event.clientY > box.top + box.height / 2 ? index + 1 : index
    setDropBefore(Math.min(Math.max(before, listStart(dragged)), listEnd(dragged)))
  }

  /** A drop anywhere in the list, rows and the gaps between them, puts the rule where the line shows. */
  function drop(event: DragEvent<HTMLDivElement>) {
    if (dragged === null) return
    event.preventDefault()
    if (dropBefore !== null) moveBefore(dragged, dropBefore)
    endDrag()
  }

  function toggle(index: number) {
    if (!rules) return
    void store(rules.map((rule, at) => (at === index ? { ...rule, enabled: !rule.enabled } : rule)))
  }

  /** Moves a rule to the other list: the core puts a rule made manual first among the manual ones, on, and one made
   * automatic last among the automatic ones. */
  function setManual(index: number, manual: boolean) {
    if (!rules) return
    void store(rules.map((rule, at) => (at === index ? { ...rule, manual, enabled: rule.enabled || manual } : rule)))
  }

  async function remove(index: number) {
    const confirmed = await confirmWithUser({
      title: text.deleteTitle,
      message: text.deleteMessage(ruleLabel(rules, index + 1)),
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

  // Where the list of the rule being dragged ends: the line shows after its last rule, never before the other list's.
  const draggedEnd = dragged === null ? 0 : listEnd(dragged)
  const rowOf = (rule: FolderRule, index: number): ReactNode =>
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
        canMoveUp={index > listStart(index)}
        canMoveDown={index >= firstMovable && index < listEnd(index) - 1}
        onEdit={() => startEditing(index, rule, false)}
        onDuplicate={() => startEditing(index + 1, copyOfRule(rule), true)}
        onMove={(by) => moveBefore(index, by < 0 ? index - 1 : index + 2)}
        draggable={!busy && index >= firstMovable}
        dropMark={
          dragged === null || dropBefore === null || movedRule(rules, dragged, dropBefore) === null
            ? null
            : dropBefore === index && index < draggedEnd
              ? "before"
              : dropBefore === index + 1 && index === draggedEnd - 1
                ? "after"
                : null
        }
        onDragStart={(event) => {
          event.dataTransfer.effectAllowed = "move"
          event.dataTransfer.setData(RULE_DRAG_TYPE, rule.id)
          setDragged(index)
        }}
        onDragEnd={endDrag}
        onDragOver={(event) => dragOver(event, index)}
        onToggle={() => toggle(index)}
        onSetManual={(manual) => setManual(index, manual)}
        onDelete={() => void remove(index)}
      />
    )
  const automaticRows = rules.slice(0, manualStart).map((rule, at) => rowOf(rule, at))
  const manualRows = rules.slice(manualStart).map((rule, at) => rowOf(rule, manualStart + at))
  // A new rule's editor goes where it will be saved, in its own list.
  if (editing?.isNew) {
    if (editing.draft.manual) manualRows.splice(editing.index - manualStart, 0, editor(editing))
    else automaticRows.splice(editing.index, 0, editor(editing))
  }

  return (
    <Page
      title={strings.nav.rules}
      scrollRef={scrollRef}
      onScroll={markShown}
      actions={
        <>
          <HelpMenu label={text.help.label} topics={helpTopics(downloadFolder)} />
          <Button size="sm" disabled={busy} onClick={() => startEditing(manualStart, newRule(), true)}>
            <Plus size={14} />
            {text.addRule}
          </Button>
        </>
      }
    >
      <div
        className="flex flex-col gap-3"
        onDragOver={(event) => {
          if (dragged !== null) event.preventDefault()
        }}
        onDrop={drop}
      >
        {/* The rules fill the width; the sentences explaining them keep to a width that reads well. */}
        <p className="max-w-3xl text-fg-secondary">{text.intro}</p>
        <RuleTester />
        {error && !editing && <StatusMessage tone="danger" {...error} />}
        <div id={RULES_SECTION_IDS.automatic} data-chapter={RULES_SECTION_IDS.automatic} className="mt-3 scroll-mt-4">
          <Section title={text.sections.automatic}>
            <div className="flex flex-col gap-3">
              {automaticRows}
              {rules.length === 0 && !editing && (
                <div className="py-8">
                  <EmptyState icon={FolderTree} title={text.emptyTitle} body={text.emptyBody} />
                </div>
              )}
            </div>
          </Section>
        </div>
        <div id={RULES_SECTION_IDS.manual} data-chapter={RULES_SECTION_IDS.manual} className="mt-3 scroll-mt-4">
          <Section
            title={text.sections.manual}
            actions={
              <Button
                size="sm"
                variant="ghost"
                disabled={busy}
                onClick={() => startEditing(rules.length, newRule(true), true)}
              >
                <Plus size={14} />
                {text.addManualRule}
              </Button>
            }
          >
            <div className="flex flex-col gap-3">
              <p className="max-w-3xl text-fg-secondary">{text.manualIntro}</p>
              {manualRows}
              {manualRows.length === 0 && <p className="text-xs text-fg-muted">{text.noManualRules}</p>}
            </div>
          </Section>
        </div>
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

/** The view's two lists as the sidebar lists them, each with how many rules it holds once they're read. */
function rulesChapters(rules: readonly FolderRule[] | undefined): Chapter[] {
  const manual = rules?.filter((rule) => rule.manual).length
  return [
    {
      id: RULES_SECTION_IDS.automatic,
      title: text.sections.automatic,
      count: rules && manual !== undefined ? rules.length - manual : undefined,
    },
    { id: RULES_SECTION_IDS.manual, title: text.sections.manual, count: manual },
  ]
}

/** What each of the rule editor's fields means, in the editor's order, under the field's own label. */
function helpTopics(downloadFolder: string): HelpTopic[] {
  const help = text.help
  return [
    { title: text.name, paragraphs: [help.name] },
    { title: text.when, paragraphs: help.when },
    { title: text.saveIn, paragraphs: help.saveIn(downloadFolder) },
    { title: text.fileName, paragraphs: help.fileName },
    { title: text.missingValues, paragraphs: help.missingValues },
    { title: text.startAs, paragraphs: help.startAs },
    { title: text.downloadRightAway, paragraphs: [help.downloadRightAway] },
    { title: text.unpackArchives, paragraphs: help.unpackArchives },
    { title: text.sections.manual, paragraphs: help.manual },
  ]
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
  /** Whether its grip can be dragged to move it. */
  draggable: boolean
  /** Where the rule being dragged would go, by this row. */
  dropMark: "before" | "after" | null
  onDragStart: (event: DragEvent<HTMLElement>) => void
  onDragEnd: () => void
  onDragOver: (event: DragEvent<HTMLDivElement>) => void
  onToggle: () => void
  /** Moves it to the manual rules, or back to the automatic ones. */
  onSetManual: (manual: boolean) => void
  onDelete: () => void
}

/** A rule as a sentence, its folder and what else it does, with a grip to drag it, its number, moving, Edit, Duplicate
 * and a menu. The catch-all says it's always first, and can't be moved, duplicated or deleted. A manual rule names its
 * conditions without "When", and is never off. A line above or below shows where a dragged rule would go. */
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
  draggable,
  dropMark,
  onDragStart,
  onDragEnd,
  onDragOver,
  onToggle,
  onSetManual,
  onDelete,
}: RuleRowProps) {
  const isCatchAll = rule.id === CATCH_ALL_RULE_ID
  const sentence = isCatchAll
    ? text.sentence.everythingElse
    : rule.manual
      ? describeManualConditions(rule.conditions, presets)
      : describeConditions(rule.conditions, presets)
  const actions = describeActions(rule, presets)
  const toggleItem: MenuItem = { label: rule.enabled ? text.turnOff : text.turnOn, onSelect: onToggle }
  const deleteItem: MenuItem = { label: text.delete, onSelect: onDelete, danger: true }
  const menuItems: MenuItem[] = isCatchAll
    ? [toggleItem]
    : rule.manual
      ? [{ label: text.moveToAutomatic, onSelect: () => onSetManual(false) }, deleteItem]
      : [toggleItem, { label: text.moveToManual, onSelect: () => onSetManual(true) }, deleteItem]
  return (
    <div data-rule-row className="relative" onDragOver={onDragOver}>
      {dropMark && (
        <span
          className={classes(
            "pointer-events-none absolute inset-x-0 h-0.5 rounded-full bg-accent",
            dropMark === "before" ? "-top-[7px]" : "-bottom-[7px]",
          )}
        />
      )}
      <Card id={`rule-${number}`} className="flex scroll-mt-4 items-center gap-3 py-3 pr-4 pl-2">
        {isCatchAll ? (
          <span className="w-4 shrink-0" />
        ) : (
          <Tooltip label={text.dragToMove}>
            <span
              role="img"
              aria-label={text.dragToMove}
              draggable={draggable}
              onDragStart={(event) => {
                // Drags the whole row, held where the grip was taken.
                const row = event.currentTarget.closest<HTMLElement>("[data-rule-row]")
                if (row) {
                  const box = row.getBoundingClientRect()
                  event.dataTransfer.setDragImage(row, event.clientX - box.left, event.clientY - box.top)
                }
                onDragStart(event)
              }}
              onDragEnd={onDragEnd}
              className={classes(
                "flex w-4 shrink-0 justify-center text-fg-muted",
                draggable ? "cursor-grab hover:text-fg-secondary active:cursor-grabbing" : "opacity-40",
              )}
            >
              <GripVertical size={16} />
            </span>
          </Tooltip>
        )}
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
          {rule.name.trim() && <p className="font-semibold break-words">{rule.name.trim()}</p>}
          {sentence && <p className="break-words">{sentence}</p>}
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
            title={text.alwaysFirstHint}
          >
            {text.alwaysFirst}
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
    </div>
  )
}

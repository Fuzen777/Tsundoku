import type { ConditionJoin } from "../../bindings/ConditionJoin"
import type { DownloadKind } from "../../bindings/DownloadKind"
import type { FolderRule } from "../../bindings/FolderRule"
import type { Preset } from "../../bindings/Preset"
import type { RuleCondition } from "../../bindings/RuleCondition"
import type { RuleField } from "../../bindings/RuleField"
import type { RuleOperator } from "../../bindings/RuleOperator"
import { strings } from "../../strings"

const text = strings.rules

/** The rule every install starts with, always first, which takes every download no other rule does
 * (`rules::CATCH_ALL_RULE_ID` in the core). */
export const CATCH_ALL_RULE_ID = "catch-all"

/** What a rule's folder can hold, filled in with the download's own (`rules::placeholder_value` in the core). Besides
 * these, `{1}` to `{9}` take what the rule's pattern captured (`capturedGroups`). */
export const FOLDER_PLACEHOLDERS = [
  "site",
  "uploader",
  "account",
  "playlist",
  "title",
  "id",
  "kind",
  "preset",
  "year",
  "month",
  "date",
  "added",
] as const

/** The most groups a rule's folder can take from its pattern, as `{1}` to `{9}`. */
const MOST_CAPTURED_GROUPS = 9

export const RULE_FIELDS: readonly RuleField[] = ["site", "uploader", "title", "link", "kind", "preset"]
export const DOWNLOAD_KINDS: readonly DownloadKind[] = ["video", "audio", "post", "file"]

/** The comparisons that fit a field: a kind or a preset is picked from a list, so it's either equal or not. Text can
 * also be matched with a pattern. */
export function operatorsFor(field: RuleField): readonly RuleOperator[] {
  return field === "kind" || field === "preset"
    ? ["is", "isNot"]
    : ["is", "isNot", "contains", "doesNotContain", "matches", "doesNotMatch"]
}

/** Whether a comparison takes a pattern (a regular expression) rather than text. */
export function isPattern(operator: RuleOperator): boolean {
  return operator === "matches" || operator === "doesNotMatch"
}

/** How many groups a rule's folder can take as `{1}`, `{2}`…: those of its first Matches pattern that captures any,
 * as the core picks them (`FolderRule::captured`), at most nine. The core reads patterns, so one JavaScript reads
 * differently, or can't read, counts as one group. */
export function capturedGroups(conditions: readonly RuleCondition[]): number {
  for (const { operator, value } of conditions) {
    const pattern = value.trim()
    if (operator !== "matches" || !pattern) continue
    const groups = countGroups(pattern)
    if (groups > 0) return Math.min(groups, MOST_CAPTURED_GROUPS)
  }
  return 0
}

/** The groups in a pattern: an empty alternative makes it match "", and the match has a slot for each group. */
function countGroups(pattern: string): number {
  try {
    return (new RegExp(`${pattern}|`).exec("")?.length ?? 1) - 1
  } catch {
    return 1
  }
}

/** A new condition on `field`, with the comparison most rules on it want. */
export function newCondition(field: RuleField, defaultPresetId: string): RuleCondition {
  switch (field) {
    case "kind":
      return { field, operator: "is", value: "video" }
    case "preset":
      return { field, operator: "is", value: defaultPresetId }
    case "title":
    case "link":
      return { field, operator: "contains", value: "" }
    case "site":
    case "uploader":
      return { field, operator: "is", value: "" }
  }
}

/** A rule not saved yet: one empty condition on the site, and nothing it does yet. Saving gives it its ID. */
export function newRule(): FolderRule {
  return {
    id: "",
    enabled: true,
    conditions: [newCondition("site", "")],
    join: "all",
    folder: "",
    presetId: null,
    downloadRightAway: null,
  }
}

/** A copy of a rule to be saved as a new one: the same conditions and actions, with no ID, so the core gives it its
 * own. */
export function copyOfRule(rule: FolderRule): FolderRule {
  return { ...rule, id: "", conditions: rule.conditions.map((condition) => ({ ...condition })) }
}

/** The rules with the one at `from` moved to just before the one at `before` (the list's length for the end), or
 * `null` when that leaves it where it is. */
export function movedRule(rules: readonly FolderRule[], from: number, before: number): FolderRule[] | null {
  if (before === from || before === from + 1) return null
  const list = [...rules]
  const [rule] = list.splice(from, 1)
  list.splice(before > from ? before - 1 : before, 0, rule)
  return list
}

/** Whether a rule with these conditions may pick the preset: not when it checks the kind or the preset, which both
 * depend on it (`FolderRule::can_pick_preset` in the core). */
export function canPickPreset(conditions: readonly RuleCondition[]): boolean {
  return !conditions.some((condition) => condition.field === "kind" || condition.field === "preset")
}

/** What a rule does besides picking a folder, as short phrases: the preset it starts videos with, and whether
 * matching links download right away. */
export function describeActions(rule: FolderRule, presets: readonly Preset[]): string[] {
  const actions = text.actions
  const phrases: string[] = []
  if (rule.presetId !== null) {
    const preset = presets.find((candidate) => candidate.id === rule.presetId)
    phrases.push(preset ? actions.startsAs(preset.name) : actions.presetDeleted)
  }
  if (rule.downloadRightAway !== null) phrases.push(rule.downloadRightAway ? actions.rightAway : actions.notRightAway)
  return phrases
}

/** A rule's conditions as a sentence: "When the site is Instagram and the uploader is “natgeo”". */
export function describeConditions(
  conditions: readonly RuleCondition[],
  join: ConditionJoin,
  presets: readonly Preset[],
): string {
  if (conditions.length === 0) return text.sentence.every
  return text.sentence.when(
    conditions.map((condition) => describeCondition(condition, presets)),
    join,
  )
}

function describeCondition({ field, operator, value }: RuleCondition, presets: readonly Preset[]): string {
  const sentence = text.sentence
  if (field === "kind") {
    const kind = isDownloadKind(value) ? sentence.kinds[value] : value
    return sentence.kind(operator === "isNot" || operator === "doesNotContain", kind)
  }
  const shown =
    field === "preset"
      ? (presets.find((preset) => preset.id === value)?.name ?? value)
      : field === "site" && !isPattern(operator)
        ? value
        : sentence.quoted(value)
  return `${sentence.fields[field]} ${sentence.operators[operator]} ${shown}`
}

function isDownloadKind(value: string): value is DownloadKind {
  return (DOWNLOAD_KINDS as readonly string[]).includes(value)
}

/** A rule's folder as it will be used: inside the download folder unless it's a full path ("D:\…" or "\\server\…"). */
export function shownFolder(folder: string, downloadFolder: string): string {
  if (!downloadFolder || /^([a-z]:[\\/]|[\\/]{2})/i.test(folder)) return folder
  return `${downloadFolder.replace(/[\\/]+$/, "")}\\${folder.replace(/^[\\/]+/, "")}`
}

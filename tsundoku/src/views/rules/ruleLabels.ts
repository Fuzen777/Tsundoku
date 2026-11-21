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

/** The elements of the Rules view's two lists of rules, which the sidebar can scroll to. */
export const RULES_SECTION_IDS = { automatic: "rules-automatic", manual: "rules-manual" } as const

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
      return { field, operator: "is", value: "video", caseSensitive: false, join: "all" }
    case "preset":
      return { field, operator: "is", value: defaultPresetId, caseSensitive: false, join: "all" }
    case "title":
    case "link":
      return { field, operator: "contains", value: "", caseSensitive: false, join: "all" }
    case "site":
    case "uploader":
      return { field, operator: "is", value: "", caseSensitive: false, join: "all" }
  }
}

/** A rule not saved yet: one empty condition on the site (a manual one's on the title, for a pattern to fill its
 * placeholders), and nothing it does yet. Saving gives it its ID. */
export function newRule(manual = false): FolderRule {
  return {
    id: "",
    name: "",
    enabled: true,
    manual,
    conditions: [manual ? { ...newCondition("title", ""), operator: "matches" } : newCondition("site", "")],
    folder: "",
    fileName: "",
    missingValues: "standIn",
    presetId: null,
    downloadRightAway: null,
    unpack: null,
  }
}

/** What a rule is called wherever it's named: its name, or "Rule 3" (its number, counting from 1) without one, or
 * before the rules are read. */
export function ruleLabel(rules: readonly FolderRule[] | undefined, number: number): string {
  return rules?.[number - 1]?.name.trim() || text.number(number)
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

/** What a rule does besides picking a folder, as short phrases: how it names files, whether it leaves out placeholders
 * without a value (when it has a folder or a file name for them), the preset it starts videos with, whether matching
 * links download right away, and whether and where their archives are unpacked. */
export function describeActions(rule: FolderRule, presets: readonly Preset[]): string[] {
  const actions = text.actions
  const phrases: string[] = []
  if (rule.fileName.trim()) phrases.push(actions.namesFiles(rule.fileName.trim()))
  if (rule.missingValues === "leaveOut" && (rule.folder.trim() || rule.fileName.trim())) {
    phrases.push(actions.leavesOut)
  }
  if (rule.presetId !== null) {
    const preset = presets.find((candidate) => candidate.id === rule.presetId)
    phrases.push(preset ? actions.startsAs(preset.name) : actions.presetDeleted)
  }
  if (rule.downloadRightAway !== null) phrases.push(rule.downloadRightAway ? actions.rightAway : actions.notRightAway)
  if (rule.unpack !== null) {
    phrases.push(
      rule.unpack === "off"
        ? actions.leavesArchivesPacked
        : actions.unpacksArchives(strings.settings.unpackPlaces[rule.unpack]),
    )
  }
  return phrases
}

/** A rule's conditions as a sentence, each after the AND or OR that joins it to the one before: "When the site is
 * Instagram AND the uploader is “natgeo”". */
export function describeConditions(conditions: readonly RuleCondition[], presets: readonly Preset[]): string {
  if (conditions.length === 0) return text.sentence.every
  return text.sentence.when(joinedConditions(conditions, presets))
}

/** A manual rule's conditions, which don't decide when it applies: "Conditions: the title matches “(RJ\d+)”", or
 * `null` without any. */
export function describeManualConditions(
  conditions: readonly RuleCondition[],
  presets: readonly Preset[],
): string | null {
  return conditions.length === 0 ? null : text.sentence.manual(joinedConditions(conditions, presets))
}

function joinedConditions(conditions: readonly RuleCondition[], presets: readonly Preset[]): string {
  return conditions
    .map((condition, index) =>
      index === 0
        ? describeCondition(condition, presets)
        : `${text.join[condition.join]} ${describeCondition(condition, presets)}`,
    )
    .join(" ")
}

function describeCondition(condition: RuleCondition, presets: readonly Preset[]): string {
  const described = describeComparison(condition, presets)
  return mindsCase(condition) ? `${described} ${text.sentence.matchingCase}` : described
}

/** Whether a condition compares letter case: when it asks to, on a field of text (`RuleCondition::minds_case` in the
 * core). */
export function mindsCase({ field, caseSensitive }: RuleCondition): boolean {
  return caseSensitive && field !== "kind" && field !== "preset"
}

function describeComparison({ field, operator, value }: RuleCondition, presets: readonly Preset[]): string {
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

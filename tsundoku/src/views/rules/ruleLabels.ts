import type { DownloadKind } from "../../bindings/DownloadKind"
import type { FolderRule } from "../../bindings/FolderRule"
import type { Preset } from "../../bindings/Preset"
import type { RuleCondition } from "../../bindings/RuleCondition"
import type { RuleField } from "../../bindings/RuleField"
import type { RuleOperator } from "../../bindings/RuleOperator"
import { strings } from "../../strings"

const text = strings.rules

/** The rule every install starts with, always last, which takes every download no other rule does
 * (`rules::CATCH_ALL_RULE_ID` in the core). */
export const CATCH_ALL_RULE_ID = "catch-all"

/** What a rule's folder can hold, filled in with the download's own. */
export const FOLDER_PLACEHOLDERS = ["site", "uploader"] as const

export const RULE_FIELDS: readonly RuleField[] = ["site", "uploader", "title", "link", "kind", "preset"]
export const DOWNLOAD_KINDS: readonly DownloadKind[] = ["video", "audio", "post", "file"]

/** The comparisons that fit a field: a kind or a preset is picked from a list, so it's either equal or not. */
export function operatorsFor(field: RuleField): readonly RuleOperator[] {
  return field === "kind" || field === "preset" ? ["is", "isNot"] : ["is", "isNot", "contains", "doesNotContain"]
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

/** A rule not saved yet: one empty condition on the site, and no folder. Saving gives it its ID. */
export function newRule(): FolderRule {
  return { id: "", enabled: true, conditions: [newCondition("site", "")], folder: "" }
}

/** A rule's conditions as a sentence: "When the site is Instagram and the uploader is “natgeo”". */
export function describeConditions(conditions: readonly RuleCondition[], presets: readonly Preset[]): string {
  if (conditions.length === 0) return text.sentence.every
  return text.sentence.when(conditions.map((condition) => describeCondition(condition, presets)))
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
      : field === "site"
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

import { Folder, FolderPen, FolderTree, Undo2 } from "lucide-react"
import { pickFolder } from "../api"
import type { Destination } from "../bindings/Destination"
import type { FolderRule } from "../bindings/FolderRule"
import { useAppActions } from "../navigation"
import { useFolderRules, useRuleLabel } from "../queries"
import { strings } from "../strings"
import { IconButton } from "./IconButton"
import { Menu, type MenuItem } from "./Menu"
import { classes } from "./styles"

interface DestinationLineProps {
  /** Where the rules, the preset or the download folder send it. */
  destination: Destination | undefined
  /** A folder picked for this one download, which wins over `destination`. */
  chosenFolder?: string | null
  /** Picks a folder for this one download, or `null` to go back to the usual one. Without it, the line only shows. */
  onChoose?: (folder: string | null) => void
  /** A rule picked for this one download, by its ID: what it sets (its folder too) wins over every other rule. */
  chosenRuleId?: string | null
  /** Picks a rule for this one download, or `null` to go back to the matching rules. Without it, none can be picked. */
  onChooseRule?: (ruleId: string | null) => void
  /** Off when a heading above already shows the folder (Pending grouped by folder): only what picked it is left. */
  folderShown?: boolean
  disabled?: boolean
  className?: string
}

/**
 * Where a download will go, and what picked that folder: a rule, the preset or the download folder, each opening
 * where it's set, or a folder picked for this one. With `onChoose`, Change Folder… picks another for it, and Use the
 * Usual Folder drops it. With `onChooseRule`, Apply a Rule… picks a rule whose folder, preset and file name win over
 * the others' for it, whatever its conditions, the manual rules listed first, and Use the Matching Rules drops it; the
 * line names the rule picked. The preview card, each pending link and Test a Link show it. Until it's known, an empty
 * box with `className` holds its place.
 */
export function DestinationLine({
  destination,
  chosenFolder = null,
  onChoose,
  chosenRuleId = null,
  onChooseRule,
  folderShown = true,
  disabled = false,
  className,
}: DestinationLineProps) {
  const { navigate } = useAppActions()
  const ruleLabel = useRuleLabel()
  const { data: rules } = useFolderRules()
  const shown: Destination | undefined = chosenFolder
    ? { folder: chosenFolder, source: { kind: "chosen" } }
    : destination
  if (!shown) return <div className={className} />
  const { source } = shown
  const sources = strings.preview.destinationSources
  // The rule picked for it, by its number, unless it's gone. When it gives the folder, the folder's source says so.
  const pickedNumber = (rules?.findIndex((rule) => rule.id === chosenRuleId) ?? -1) + 1
  const pickedRule = chosenRuleId && pickedNumber > 0 ? ruleLabel(pickedNumber) : null
  const folderFromPicked = pickedRule !== null && source.kind === "rule" && source.number === pickedNumber
  const from =
    source.kind === "rule"
      ? ([
          folderFromPicked ? sources.pickedRule(ruleLabel(source.number)) : sources.rule(ruleLabel(source.number)),
          "rules",
        ] as const)
      : source.kind === "preset"
        ? ([sources.preset, "presets"] as const)
        : source.kind === "downloadFolder"
          ? ([sources.downloadFolder, "settings"] as const)
          : null

  async function choose() {
    const folder = await pickFolder(strings.preview.chooseFolderTitle)
    if (folder) onChoose?.(folder)
  }

  return (
    <div className={classes("flex min-w-0 items-center gap-1.5 text-xs text-fg-secondary", className)}>
      {folderShown && (
        <>
          <Folder size={14} className="shrink-0" />
          <span className="truncate" title={strings.preview.savesIn(shown.folder)}>
            {shown.folder}
          </span>
        </>
      )}
      {from ? (
        <button
          type="button"
          className="shrink-0 rounded-sm text-fg-muted transition-colors hover:text-fg focus-visible:outline-2 focus-visible:outline-accent"
          title={
            folderFromPicked && pickedRule
              ? strings.preview.pickedRuleHint(pickedRule)
              : folderShown
                ? undefined
                : strings.preview.savesIn(shown.folder)
          }
          onClick={() => navigate(from[1])}
        >
          {folderShown ? strings.preview.destinationFrom(from[0]) : from[0]}
        </button>
      ) : (
        <span className="shrink-0 text-fg-muted">
          {folderShown ? strings.preview.destinationFrom(sources.chosen) : sources.chosen}
        </span>
      )}
      {pickedRule && !folderFromPicked && (
        <span className="shrink-0 text-fg-muted" title={strings.preview.pickedRuleHint(pickedRule)}>
          {strings.preview.destinationFrom(sources.pickedRule(pickedRule))}
        </span>
      )}
      {(onChoose || onChooseRule) && (
        <span className="-my-1.5 flex shrink-0 items-center">
          {onChoose && (
            <>
              <IconButton
                label={strings.preview.chooseFolder}
                icon={FolderPen}
                disabled={disabled}
                onClick={() => void choose()}
              />
              {chosenFolder && (
                <IconButton
                  label={strings.preview.usualFolder}
                  icon={Undo2}
                  disabled={disabled}
                  onClick={() => onChoose(null)}
                />
              )}
            </>
          )}
          {onChooseRule && rules && (
            <>
              <Menu
                label={strings.preview.applyRule}
                icon={FolderTree}
                items={ruleChoices(rules, ruleLabel, onChooseRule)}
              />
              {pickedRule && (
                <IconButton
                  label={strings.preview.matchingRules}
                  icon={Undo2}
                  disabled={disabled}
                  onClick={() => onChooseRule(null)}
                />
              )}
            </>
          )}
        </span>
      )}
    </div>
  )
}

/** Apply a Rule…'s list: every rule by its name or number, "(Off)" after one that's off. With manual rules, they come
 * first under their heading, since they're only ever applied this way, then the automatic ones under theirs. */
function ruleChoices(
  rules: readonly FolderRule[],
  ruleLabel: (number: number) => string,
  onChooseRule: (ruleId: string) => void,
): MenuItem[] {
  const hasManual = rules.some((rule) => rule.manual)
  const sections = strings.rules.sections
  const numbered = rules.map((rule, index) => ({ rule, number: index + 1 }))
  return [...numbered.filter(({ rule }) => rule.manual), ...numbered.filter(({ rule }) => !rule.manual)].map(
    ({ rule, number }) => ({
      label: rule.enabled ? ruleLabel(number) : strings.preview.ruleOff(ruleLabel(number)),
      onSelect: () => onChooseRule(rule.id),
      group: hasManual ? (rule.manual ? sections.manual : sections.automatic) : undefined,
    }),
  )
}

import { useAppActions } from "../navigation"
import { strings } from "../strings"

/** Beside a preset picker: the rule that picked its preset, opening the rules, as `DestinationLine` names the rule that
 * picked a folder. Nothing when no rule did. */
export function PresetRuleLink({ rule }: { rule: number | null }) {
  const { navigate } = useAppActions()
  if (rule === null) return null
  return (
    <button
      type="button"
      className="shrink-0 rounded-sm text-xs text-fg-muted transition-colors hover:text-fg focus-visible:outline-2 focus-visible:outline-accent"
      title={strings.preview.presetByRuleHint(rule)}
      onClick={() => navigate("rules")}
    >
      {strings.preview.presetByRule(rule)}
    </button>
  )
}

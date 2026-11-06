import { Folder } from "lucide-react"
import type { Destination } from "../bindings/Destination"
import { useAppActions } from "../navigation"
import { strings } from "../strings"
import { classes } from "./styles"

/** Where a download will go, and what picked that folder: a rule, the preset or the download folder, each opening
 * where it's set. The preview card and each pending link show it. Until it's known, an empty box with `className`
 * holds its place. */
export function DestinationLine({
  destination,
  className,
}: {
  destination: Destination | undefined
  className?: string
}) {
  const { navigate } = useAppActions()
  if (!destination) return <div className={className} />
  const { source } = destination
  const sources = strings.preview.destinationSources
  const [label, view] =
    source.kind === "rule"
      ? ([sources.rule(source.number), "rules"] as const)
      : source.kind === "preset"
        ? ([sources.preset, "presets"] as const)
        : ([sources.downloadFolder, "settings"] as const)
  return (
    <div className={classes("flex min-w-0 items-center gap-1.5 text-xs text-fg-secondary", className)}>
      <Folder size={14} className="shrink-0" />
      <span className="truncate" title={strings.preview.savesIn(destination.folder)}>
        {destination.folder}
      </span>
      <button
        type="button"
        className="shrink-0 rounded-sm text-fg-muted transition-colors hover:text-fg focus-visible:outline-2 focus-visible:outline-accent"
        onClick={() => navigate(view)}
      >
        {strings.preview.destinationFrom(label)}
      </button>
    </div>
  )
}

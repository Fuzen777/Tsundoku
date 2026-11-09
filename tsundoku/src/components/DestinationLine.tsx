import { Folder, FolderPen, Undo2 } from "lucide-react"
import { pickFolder } from "../api"
import type { Destination } from "../bindings/Destination"
import { useAppActions } from "../navigation"
import { strings } from "../strings"
import { IconButton } from "./IconButton"
import { classes } from "./styles"

interface DestinationLineProps {
  /** Where the rules, the preset or the download folder send it. */
  destination: Destination | undefined
  /** A folder picked for this one download, which wins over `destination`. */
  chosenFolder?: string | null
  /** Picks a folder for this one download, or `null` to go back to the usual one. Without it, the line only shows. */
  onChoose?: (folder: string | null) => void
  /** Off when a heading above already shows the folder (Pending grouped by folder): only what picked it is left. */
  folderShown?: boolean
  disabled?: boolean
  className?: string
}

/**
 * Where a download will go, and what picked that folder: a rule, the preset or the download folder, each opening
 * where it's set, or a folder picked for this one. With `onChoose`, Change Folder… picks another for it, and Use the
 * Usual Folder drops it. The preview card and each pending link show it. Until it's known, an empty box with
 * `className` holds its place.
 */
export function DestinationLine({
  destination,
  chosenFolder = null,
  onChoose,
  folderShown = true,
  disabled = false,
  className,
}: DestinationLineProps) {
  const { navigate } = useAppActions()
  const shown: Destination | undefined = chosenFolder ? { folder: chosenFolder, source: { kind: "chosen" } } : destination
  if (!shown) return <div className={className} />
  const { source } = shown
  const sources = strings.preview.destinationSources
  const from =
    source.kind === "rule"
      ? ([sources.rule(source.number), "rules"] as const)
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
          title={folderShown ? undefined : strings.preview.savesIn(shown.folder)}
          onClick={() => navigate(from[1])}
        >
          {folderShown ? strings.preview.destinationFrom(from[0]) : from[0]}
        </button>
      ) : (
        <span className="shrink-0 text-fg-muted">
          {folderShown ? strings.preview.destinationFrom(sources.chosen) : sources.chosen}
        </span>
      )}
      {onChoose && (
        <span className="-my-1.5 flex shrink-0 items-center">
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
        </span>
      )}
    </div>
  )
}

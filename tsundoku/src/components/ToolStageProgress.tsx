import type { ToolStage } from "../bindings/ToolStage"
import { formatBytes } from "../format"
import { strings } from "../strings"
import { ProgressBar } from "./ProgressBar"

/** How far a tool's install or update has got: a bar, and what it's doing. */
export function ToolStageProgress({ stage }: { stage: ToolStage }) {
  const { text, fraction } = describeStage(stage)
  return (
    <div className="flex flex-col gap-1.5">
      <ProgressBar value={fraction} label={text} />
      <p className="text-xs text-fg-secondary tabular-nums">{text}</p>
    </div>
  )
}

/** Only a download knows how far it has got; the other stages show a sliding bar. */
function describeStage(stage: ToolStage): { text: string; fraction?: number } {
  const text = strings.tools.stages
  switch (stage.kind) {
    case "connecting":
      return { text: text.connecting }
    case "downloading":
      return {
        text: text.downloading(formatBytes(stage.received), stage.total === null ? null : formatBytes(stage.total)),
        fraction: stage.total ? stage.received / stage.total : undefined,
      }
    case "verifying":
      return { text: text.verifying }
    case "installing":
      return { text: text.installing }
    case "checking":
      return { text: text.checking }
    case "updating":
      return { text: text.updating }
  }
}

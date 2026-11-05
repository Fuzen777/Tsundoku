import { Braces, Clapperboard, Download, type LucideIcon } from "lucide-react"
import type { ToolId } from "./bindings/ToolId"

/** The tools in the order they're listed, as the core returns them. Names and descriptions are in strings.ts. */
export const toolOrder: readonly ToolId[] = ["ytDlp", "ffmpeg", "deno"]

export const toolIcons: Record<ToolId, LucideIcon> = {
  ytDlp: Download,
  ffmpeg: Clapperboard,
  deno: Braces,
}

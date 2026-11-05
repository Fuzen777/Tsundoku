import { Braces, Clapperboard, Download, Images, type LucideIcon } from "lucide-react"
import type { ToolId } from "./bindings/ToolId"

/** The tools in the order they're listed, as the core returns them. Names and descriptions are in strings.ts. */
export const toolOrder: readonly ToolId[] = ["ytDlp", "ffmpeg", "deno", "galleryDl"]

/** The tools the first-run setup offers, as the core's `ToolId::SETUP`. gallery-dl is set up from Settings. */
export const setupToolOrder: readonly ToolId[] = ["ytDlp", "ffmpeg", "deno"]

export const toolIcons: Record<ToolId, LucideIcon> = {
  ytDlp: Download,
  ffmpeg: Clapperboard,
  deno: Braces,
  galleryDl: Images,
}

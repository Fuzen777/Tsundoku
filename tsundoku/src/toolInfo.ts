import { Braces, Clapperboard, Download, Images, type LucideIcon } from "lucide-react"
import type { ToolId } from "./bindings/ToolId"

/** The tools in the order they're listed, as the core returns them. Names and descriptions are in strings.ts. */
export const toolOrder: readonly ToolId[] = ["ytDlp", "ffmpeg", "deno", "galleryDl"]

/** The tools the first-run setup offers, as the core's `ToolId::SETUP`: all of them. */
export const setupToolOrder: readonly ToolId[] = ["ytDlp", "ffmpeg", "deno", "galleryDl"]

/** The tools the app works without, so the setup can be left when only they failed. */
export const optionalTools: readonly ToolId[] = ["deno", "galleryDl"]

export const toolIcons: Record<ToolId, LucideIcon> = {
  ytDlp: Download,
  ffmpeg: Clapperboard,
  deno: Braces,
  galleryDl: Images,
}

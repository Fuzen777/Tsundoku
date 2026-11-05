import { History, ListVideo, Settings, SlidersHorizontal, type LucideIcon } from "lucide-react"
import { strings } from "./strings"

export type ViewId = "queue" | "history" | "presets" | "settings"

export interface NavItem {
  id: ViewId
  label: string
  icon: LucideIcon
}

/** The views at the top of the sidebar. */
export const mainNavItems: readonly NavItem[] = [
  { id: "queue", label: strings.nav.queue, icon: ListVideo },
  { id: "history", label: strings.nav.history, icon: History },
  { id: "presets", label: strings.nav.presets, icon: SlidersHorizontal },
]

/** Settings sits apart, at the bottom of the sidebar. */
export const settingsNavItem: NavItem = { id: "settings", label: strings.nav.settings, icon: Settings }
